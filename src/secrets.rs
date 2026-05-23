//! Token storage helpers (#98).
//!
//! Stores opaque secret strings under (service, key) tuples. The OS keyring
//! (Secret Service on Linux, Keychain on macOS, Credential Manager on Windows)
//! is the preferred backend. If keyring is unavailable (containers, headless
//! servers, no D-Bus) we fall back to a JSON file under the user's config dir
//! with 0600 permissions on Unix.
//!
//! Callers serialize their token struct to a string (typically JSON) and hand
//! the result to [`store`] / [`load`] — this module makes no assumptions about
//! the payload.

use std::path::PathBuf;

const APP_NAME: &str = "noctune";

/// Persist `value` under `(service, key)`. Returns `true` if the value reached
/// the keyring, `false` if we fell back to the file store.
pub fn store(service: &str, key: &str, value: &str) -> bool {
    let id = make_id(service, key);
    match keyring::Entry::new(APP_NAME, &id) {
        Ok(entry) => match entry.set_password(value) {
            Ok(()) => {
                tracing::debug!(target: "secrets", "stored {service}/{key} in keyring");
                return true;
            }
            Err(e) => {
                tracing::warn!(target: "secrets", "keyring set failed for {service}/{key}: {e} — using file fallback")
            }
        },
        Err(e) => {
            tracing::warn!(target: "secrets", "keyring entry failed for {service}/{key}: {e} — using file fallback")
        }
    }
    file_store(service, key, value);
    false
}

/// Read a previously-stored value. Tries keyring first, then file fallback.
pub fn load(service: &str, key: &str) -> Option<String> {
    let id = make_id(service, key);
    if let Ok(entry) = keyring::Entry::new(APP_NAME, &id) {
        match entry.get_password() {
            Ok(v) => return Some(v),
            Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                tracing::warn!(target: "secrets", "keyring read failed for {service}/{key}: {e}")
            }
        }
    }
    file_load(service, key)
}

/// Remove a stored value from every backend that has it.
#[allow(dead_code)]
pub fn delete(service: &str, key: &str) {
    let id = make_id(service, key);
    if let Ok(entry) = keyring::Entry::new(APP_NAME, &id) {
        let _ = entry.delete_credential();
    }
    file_delete(service, key);
}

fn make_id(service: &str, key: &str) -> String {
    format!("{service}:{key}")
}

fn fallback_path() -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|p| p.config_dir().join("secrets.json"))
}

fn file_store(service: &str, key: &str, value: &str) {
    let Some(path) = fallback_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut map = file_read_all().unwrap_or_default();
    map.insert(make_id(service, key), value.to_string());
    match serde_json::to_string(&map) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&path, s) {
                tracing::warn!(target: "secrets", "file fallback write failed {}: {e}", path.display());
                return;
            }
            restrict_permissions(&path);
        }
        Err(e) => tracing::warn!(target: "secrets", "file fallback serialize failed: {e}"),
    }
}

fn file_load(service: &str, key: &str) -> Option<String> {
    let map = file_read_all()?;
    map.get(&make_id(service, key)).cloned()
}

#[allow(dead_code)]
fn file_delete(service: &str, key: &str) {
    let Some(path) = fallback_path() else { return };
    let Some(mut map) = file_read_all() else {
        return;
    };
    if map.remove(&make_id(service, key)).is_some() {
        match serde_json::to_string(&map) {
            Ok(s) => {
                if let Err(e) = std::fs::write(&path, s) {
                    tracing::warn!(target: "secrets", "file fallback rewrite failed {}: {e}", path.display());
                }
            }
            Err(e) => tracing::warn!(target: "secrets", "file fallback serialize failed: {e}"),
        }
    }
}

fn file_read_all() -> Option<std::collections::HashMap<String, String>> {
    let path = fallback_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {
    // Windows ACLs on per-user config dir are sufficient — no extra work needed.
}

/// One-time migration helper: if `legacy_path` exists, read it into the keyring
/// and delete the file. Safe to call on every startup.
pub fn migrate_from_file(service: &str, key: &str, legacy_path: &std::path::Path) {
    if !legacy_path.exists() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(legacy_path) else {
        return;
    };
    let stored_in_keyring = store(service, key, &text);
    if stored_in_keyring {
        match std::fs::remove_file(legacy_path) {
            Ok(()) => {
                tracing::info!(target: "secrets", "migrated {service}/{key} from {} to keyring", legacy_path.display())
            }
            Err(e) => {
                tracing::warn!(target: "secrets", "migrated {service}/{key} to keyring but could not delete {}: {e}", legacy_path.display())
            }
        }
    }
}
