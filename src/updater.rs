//! Self-Updater module for Noctune.
//! Checks GitHub Releases for new builds and performs in-place binary replacements.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::Duration;
use std::{
    io::{Read, Write},
    path::Path,
};

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Checks the GitHub API for the latest Noctune release.
pub fn check_for_updates() -> Result<Option<UpdateInfo>> {
    let current_version = env!("CARGO_PKG_VERSION").trim_start_matches('v');
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(format!("Noctune/{current_version}"))
        .build()?;

    let url = "https://api.github.com/repos/WendellOttoni/Noctune/releases/latest";
    let resp = client
        .get(url)
        .send()
        .context("Failed to query GitHub Releases API")?;

    if !resp.status().is_success() {
        return Err(anyhow!("GitHub API returned HTTP {}", resp.status()));
    }

    let release: GitHubRelease = resp.json().context("Failed to parse release response")?;
    let latest_version = release.tag_name.trim_start_matches('v');

    if is_newer_version(latest_version, current_version) {
        let target_artifact = target_artifact(std::env::consts::OS, std::env::consts::ARCH)
            .context("No release artifact for this OS/architecture")?;
        let download_url = release
            .assets
            .into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(target_artifact))
            .map(|a| a.browser_download_url);

        Ok(Some(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest_version.to_string(),
            release_notes: release.body,
            download_url,
        }))
    } else {
        Ok(None)
    }
}

/// Downloads and replaces the current running binary in-place.
pub fn apply_update(download_url: &str) -> Result<()> {
    let expected_name = target_artifact(std::env::consts::OS, std::env::consts::ARCH)
        .context("Unsupported operating system/architecture")?;
    let url = url::Url::parse(download_url)?;
    anyhow::ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url
                .path()
                .starts_with("/WendellOttoni/Noctune/releases/download/")
            && url.path_segments().and_then(|mut s| s.next_back()) == Some(expected_name)
            && url.query().is_none()
            && url.fragment().is_none(),
        "Unexpected release artifact URL"
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent("Noctune-Updater")
        .build()?;
    let mut digest_bytes = Vec::new();
    client
        .get(format!("{download_url}.sha256"))
        .send()?
        .error_for_status()?
        .take(4097)
        .read_to_end(&mut digest_bytes)?;
    anyhow::ensure!(digest_bytes.len() <= 4096, "Invalid checksum file");
    let checksum = String::from_utf8(digest_bytes)?;
    let expected = parse_checksum(&checksum, expected_name)?;
    let mut bytes = Vec::new();
    client
        .get(download_url)
        .send()?
        .error_for_status()?
        .take(200 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 200 * 1024 * 1024,
        "Release exceeds size limit"
    );
    install_verified(&std::env::current_exe()?, &bytes, expected)
}

fn install_verified(path: &Path, bytes: &[u8], expected: &str) -> Result<()> {
    verify_download(bytes, expected)?;
    validate_binary(bytes, std::env::consts::OS, std::env::consts::ARCH)?;
    replace_binary(path, bytes)
}

pub fn target_artifact(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("noctune-windows-x64.exe"),
        ("linux", "x86_64") => Some("noctune-linux-x64"),
        ("macos", "aarch64") => Some("noctune-macos-arm64"),
        ("macos", "x86_64") => Some("noctune-macos-x64"),
        _ => None,
    }
}

fn parse_checksum<'a>(text: &'a str, name: &str) -> Result<&'a str> {
    let mut parts = text.split_whitespace();
    let digest = parts.next().context("Missing digest")?;
    anyhow::ensure!(
        digest.len() == 64
            && digest.bytes().all(|b| b.is_ascii_hexdigit())
            && parts.next().map(|s| s.trim_start_matches('*')) == Some(name)
            && parts.next().is_none(),
        "Invalid release checksum manifest"
    );
    Ok(digest)
}
fn verify_download(bytes: &[u8], expected: &str) -> Result<()> {
    anyhow::ensure!(!bytes.is_empty(), "Empty release");
    anyhow::ensure!(
        format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(expected),
        "Release checksum mismatch; installed version preserved"
    );
    Ok(())
}

fn validate_binary(bytes: &[u8], os: &str, arch: &str) -> Result<()> {
    let valid = match (os, arch) {
        ("windows", "x86_64") if bytes.len() >= 64 && &bytes[..2] == b"MZ" => {
            let offset = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
            bytes.get(offset..offset.saturating_add(6)) == Some(&b"PE\0\0\x64\x86"[..])
        }
        ("linux", "x86_64") => {
            bytes.len() >= 20
                && &bytes[..4] == b"\x7fELF"
                && bytes[4] == 2
                && bytes[5] == 1
                && bytes[18..20] == [62, 0]
        }
        ("macos", "aarch64" | "x86_64") => {
            bytes.len() >= 8
                && bytes[..4] == [0xcf, 0xfa, 0xed, 0xfe]
                && u32::from_le_bytes(bytes[4..8].try_into().unwrap())
                    == if arch == "aarch64" {
                        0x0100000c
                    } else {
                        0x01000007
                    }
        }
        _ => false,
    };
    anyhow::ensure!(valid, "Release binary does not match this OS/architecture");
    Ok(())
}

fn replace_binary(exe_path: &Path, new_bytes: &[u8]) -> Result<()> {
    let parent = exe_path
        .parent()
        .context("Executable has no parent directory")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(new_bytes)?;
    staged.as_file().sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    // A unique backup avoids overwriting an older recoverable installation.
    let backup = tempfile::Builder::new()
        .prefix("noctune-backup-")
        .suffix(".old")
        .tempfile_in(parent)?;
    let backup_path = backup.path().to_path_buf();
    backup.close()?;
    std::fs::rename(exe_path, &backup_path).context("Could not preserve current executable")?;
    match staged.persist(exe_path) {
        Ok(_) => {
            tracing::info!(backup = %backup_path.display(), "Update installed; backup retained");
            Ok(())
        }
        Err(error) => {
            std::fs::rename(&backup_path, exe_path).with_context(|| {
                format!(
                    "Update failed; recover original from {}",
                    backup_path.display()
                )
            })?;
            Err(error.error).context("Update failed; original restored")
        }
    }
}

fn is_newer_version(remote: &str, current: &str) -> bool {
    let parse_ver = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|part| part.trim().parse::<u32>().ok())
            .collect()
    };

    let r_parts = parse_ver(remote);
    let c_parts = parse_ver(current);

    for (r, c) in r_parts.iter().zip(c_parts.iter()) {
        if r > c {
            return true;
        }
        if r < c {
            return false;
        }
    }

    r_parts.len() > c_parts.len()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_download_leaves_installation_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("noctune");
        std::fs::write(&exe, b"original").unwrap();
        assert!(install_verified(&exe, b"truncated", &"0".repeat(64)).is_err());
        assert_eq!(std::fs::read(exe).unwrap(), b"original");
    }
    #[test]
    fn checksum_names_and_architectures_must_match() {
        assert!(parse_checksum(&format!("{}  wrong", "a".repeat(64)), "noctune").is_err());
        assert_eq!(
            target_artifact("macos", "x86_64"),
            Some("noctune-macos-x64")
        );
        assert_eq!(target_artifact("linux", "aarch64"), None);
        assert!(validate_binary(b"<html>error</html>", "windows", "x86_64").is_err());
    }
    #[test]
    fn successful_replacement_retains_backup() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("noctune");
        std::fs::write(&exe, b"old").unwrap();
        replace_binary(&exe, b"new").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        let backup = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .find(|entry| entry.path().extension().is_some_and(|e| e == "old"))
            .unwrap();
        assert_eq!(std::fs::read(backup.path()).unwrap(), b"old");
    }
}
