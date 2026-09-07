//! Persistent native credentials, with a locked, atomic file fallback.
use anyhow::{Context, Result};
use std::{collections::HashMap, fs::OpenOptions, io::Write, path::Path};

const APP_NAME: &str = "noctune";

#[derive(Debug, PartialEq, Eq)]
pub enum Backend {
    Native,
    File,
}

pub fn store(service: &str, key: &str, value: &str) -> Result<Backend> {
    let id = format!("{service}:{key}");
    let path = crate::config::project_dirs()?
        .config_dir()
        .join("secrets.json");
    if let Ok(entry) = keyring::Entry::new(APP_NAME, &id) {
        if entry.set_password(value).is_ok()
            && keyring::Entry::new(APP_NAME, &id)
                .ok()
                .and_then(|entry| entry.get_password().ok())
                .as_deref()
                == Some(value)
        {
            if path.exists() {
                update_file(&path, &id, None)?;
            }
            return Ok(Backend::Native);
        }
    }
    update_file(&path, &id, Some(value))?;
    tracing::warn!(target: "secrets", "Native credential store unavailable; using protected local file");
    Ok(Backend::File)
}

pub fn load(service: &str, key: &str) -> Option<String> {
    let id = format!("{service}:{key}");
    // A failed native write may leave an older entry behind. The fallback is the
    // latest value until a successful native write explicitly removes it.
    if let Ok(dirs) = crate::config::project_dirs() {
        if let Some(value) = read_file(&dirs.config_dir().join("secrets.json"))
            .ok()
            .and_then(|mut values| values.remove(&id))
        {
            return Some(value);
        }
    }
    if let Ok(entry) = keyring::Entry::new(APP_NAME, &id) {
        if let Ok(value) = entry.get_password() {
            return Some(value);
        }
    }
    let path = crate::config::project_dirs()
        .ok()?
        .config_dir()
        .join("secrets.json");
    read_file(&path).ok()?.remove(&id)
}

#[allow(dead_code)]
pub fn delete(service: &str, key: &str) -> Result<()> {
    let id = format!("{service}:{key}");
    if let Ok(entry) = keyring::Entry::new(APP_NAME, &id) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.into()),
        }
    }
    let path = crate::config::project_dirs()?
        .config_dir()
        .join("secrets.json");
    update_file(&path, &id, None)
}

fn read_file(path: &Path) -> Result<HashMap<String, String>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).context("Invalid credential file; preserving it")
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(error.into()),
    }
}

fn update_file(path: &Path, id: &str, value: Option<&str>) -> Result<()> {
    let parent = path.parent().context("Credential path has no parent")?;
    std::fs::create_dir_all(parent)?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(path.with_extension("lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let mut values = read_file(path)?;
    match value {
        Some(value) => {
            values.insert(id.to_owned(), value.to_owned());
        }
        None => {
            values.remove(id);
        }
    }
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(&serde_json::to_vec(&values)?)?;
    staged.as_file().sync_all()?;
    staged
        .persist(path)
        .context("Could not atomically persist credentials")?;
    Ok(())
}

/// Preserve the legacy copy until the destination has been independently read.
pub fn migrate_from_file(service: &str, key: &str, legacy_path: &Path) {
    let Ok(value) = std::fs::read_to_string(legacy_path) else {
        return;
    };
    if let Err(error) = migrate_verified(legacy_path, || {
        store(service, key, &value)?;
        anyhow::ensure!(
            load(service, key).as_deref() == Some(value.as_str()),
            "Credential verification failed"
        );
        Ok(())
    }) {
        tracing::warn!(target: "secrets", "Credential migration incomplete; legacy copy preserved: {error}");
    }
}

fn migrate_verified(path: &Path, persist: impl FnOnce() -> Result<()>) -> Result<()> {
    persist()?;
    std::fs::remove_file(path).context("Could not remove verified legacy credentials")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_persists_across_processes() {
        const KEY: &str = "NOCTUNE_TEST_SECRET_PATH";
        if let Some(path) = std::env::var_os(KEY) {
            update_file(Path::new(&path), "test:session", Some("fixture")).unwrap();
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.json");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "secrets::tests::fallback_persists_across_processes",
            ])
            .env(KEY, &path)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            read_file(&path).unwrap().get("test:session").unwrap(),
            "fixture"
        );
    }
    #[test]
    fn failed_migration_preserves_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy");
        std::fs::write(&path, "original").unwrap();
        assert!(migrate_verified(&path, || anyhow::bail!("disk full")).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
    }
    #[test]
    fn corrupt_store_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.json");
        std::fs::write(&path, "corrupt").unwrap();
        assert!(update_file(&path, "a", Some("b")).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "corrupt");
    }
    #[test]
    fn file_roundtrip_preserves_other_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.json");
        update_file(&path, "a", Some("first")).unwrap();
        update_file(&path, "b", Some("second")).unwrap();
        update_file(&path, "a", None).unwrap();
        assert_eq!(
            read_file(&path).unwrap(),
            HashMap::from([("b".into(), "second".into())])
        );
    }
}
