//! Self-Updater module for Noctune.
//! Checks GitHub Releases for new builds and performs in-place binary replacements.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

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
        let target_artifact = current_target_artifact();
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
    let current_exe =
        std::env::current_exe().context("Could not determine current executable path")?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(format!("Noctune/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client
        .get(download_url)
        .send()
        .context("Failed to download release binary")?;
    if !resp.status().is_success() {
        return Err(anyhow!("Download failed with HTTP {}", resp.status()));
    }

    let bytes = resp.bytes().context("Failed to read binary data")?;
    if bytes.is_empty() {
        return Err(anyhow!("Downloaded binary is empty"));
    }

    replace_binary(&current_exe, &bytes)
}

fn current_target_artifact() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "noctune-windows-x64.exe"
    }
    #[cfg(target_os = "linux")]
    {
        "noctune-linux-x64"
    }
    #[cfg(target_os = "macos")]
    {
        "noctune-macos-arm64"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        "noctune"
    }
}

fn replace_binary(exe_path: &PathBuf, new_bytes: &[u8]) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let old_exe = exe_path.with_extension("exe.old");
        // Remove any preexisting .old file if left from previous update
        let _ = std::fs::remove_file(&old_exe);

        // Rename running exe to .old (Windows allows renaming open files)
        std::fs::rename(exe_path, &old_exe)
            .context("Failed to rename running executable to .old")?;

        // Write new binary in place
        if let Err(e) = std::fs::write(exe_path, new_bytes) {
            // Restore original executable if write fails
            let _ = std::fs::rename(&old_exe, exe_path);
            return Err(e).context("Failed to write new executable");
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let temp_path = exe_path.with_extension("tmp");
        std::fs::write(&temp_path, new_bytes).context("Failed to write temp executable")?;
        let mut perms = std::fs::metadata(&temp_path)?.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&temp_path, perms);

        std::fs::rename(&temp_path, exe_path).context("Failed to swap executable in place")?;
    }

    #[cfg(not(any(target_os = "windows", unix)))]
    {
        let _ = (exe_path, new_bytes);
        return Err(anyhow!(
            "Unsupported operating system for in-place self-update"
        ));
    }

    Ok(())
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
