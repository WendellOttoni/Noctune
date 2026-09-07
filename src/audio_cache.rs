//! Budgeted disk audio cache. Open decoders hold leases, including prefetched audio.
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs::{File, FileTimes},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::{Duration, SystemTime},
};

static ACTIVE: LazyLock<Mutex<HashMap<PathBuf, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CONFIG: LazyLock<Mutex<crate::config::CacheConfig>> =
    LazyLock::new(|| Mutex::new(Default::default()));
pub fn configure(config: crate::config::CacheConfig) {
    *CONFIG.lock().unwrap() = config;
}
pub struct Lease(PathBuf);
impl Drop for Lease {
    fn drop(&mut self) {
        let mut active = ACTIVE.lock().unwrap();
        if let Some(count) = active.get_mut(&self.0) {
            *count -= 1;
            if *count == 0 {
                active.remove(&self.0);
            }
        }
    }
}
pub fn open(path: &Path) -> Result<crate::audio::SymphoniaSource> {
    let mut active = ACTIVE.lock().unwrap();
    let file = File::open(path)?;
    let _ = file.set_times(FileTimes::new().set_modified(SystemTime::now()));
    *active.entry(path.to_path_buf()).or_default() += 1;
    let lease = Lease(path.to_path_buf());
    drop(active);
    decode(file, lease)
}

fn decode(file: File, lease: Lease) -> Result<crate::audio::SymphoniaSource> {
    let mut hint = symphonia::core::probe::Hint::new();
    if let Some(ext) = lease.0.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut source = crate::audio::SymphoniaSource::from_file(file, hint)?;
    source.cache_lease = Some(lease);
    Ok(source)
}

pub fn publish(candidate: &Path, destination: &Path) -> Result<crate::audio::SymphoniaSource> {
    // Validate before replacing an existing (possibly corrupt) cache entry.
    drop(crate::audio::SymphoniaSource::from_file(
        File::open(candidate)?,
        symphonia::core::probe::Hint::new(),
    )?);
    let mut active = ACTIVE.lock().unwrap();
    if !active.contains_key(destination) {
        let mut staged = tempfile::NamedTempFile::new_in(
            destination.parent().context("Cache path has no parent")?,
        )?;
        std::io::copy(&mut File::open(candidate)?, staged.as_file_mut())?;
        staged.as_file().sync_all()?;
        staged
            .persist(destination)
            .context("Could not publish completed audio download")?;
    }
    // Retain the open file before releasing the cache lock, then establish its lease.
    let file = File::open(destination)?;
    *active.entry(destination.to_path_buf()).or_default() += 1;
    let lease = Lease(destination.to_path_buf());
    drop(active);
    decode(file, lease)
}

#[derive(Debug, Default)]
pub struct Usage {
    pub bytes: u64,
    pub files: usize,
    pub removed: usize,
}
pub fn maintain(clear: bool) -> Result<Usage> {
    let dir = crate::config::audio_cache_dir()?;
    std::fs::create_dir_all(&dir)?;
    prune(
        &dir,
        &CONFIG.lock().unwrap(),
        clear,
        &ACTIVE.lock().unwrap(),
    )
}
fn prune(
    dir: &Path,
    cfg: &crate::config::CacheConfig,
    clear: bool,
    active: &HashMap<PathBuf, usize>,
) -> Result<Usage> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if !entry.file_name().to_string_lossy().starts_with("noctune_") {
            continue;
        }
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("mp4" | "m4a" | "webm" | "opus" | "ogg" | "mp3" | "aac" | "wav")
        ) {
            continue;
        }
        let meta = entry.metadata()?;
        files.push((
            path,
            meta.len(),
            meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        ));
    }
    files.sort_by_key(|entry| entry.2);
    let mut usage = Usage {
        bytes: files.iter().map(|f| f.1).sum(),
        files: files.len(),
        removed: 0,
    };
    let budget = cfg.audio_max_size_mb.saturating_mul(1024 * 1024);
    for (path, size, modified) in files {
        let expired = cfg.audio_expire_days > 0
            && modified.elapsed().unwrap_or_default()
                > Duration::from_secs(cfg.audio_expire_days.saturating_mul(86400));
        if !active.contains_key(&path) && (clear || expired || (budget > 0 && usage.bytes > budget))
        {
            std::fs::remove_file(&path).context("Could not remove unused audio cache file")?;
            usage.bytes = usage.bytes.saturating_sub(size);
            usage.files -= 1;
            usage.removed += 1;
        }
    }
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_keeps_active_and_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        let protected = dir.path().join("noctune_playing.m4a");
        std::fs::write(&protected, vec![0; 800_000]).unwrap();
        std::fs::write(dir.path().join("noctune_old.m4a"), vec![0; 800_000]).unwrap();
        std::fs::write(dir.path().join("personal.mp3"), b"keep").unwrap();
        let config = crate::config::CacheConfig {
            audio_max_size_mb: 1,
            ..Default::default()
        };
        let usage = prune(
            dir.path(),
            &config,
            false,
            &HashMap::from([(protected.clone(), 1)]),
        )
        .unwrap();
        assert_eq!(usage.removed, 1);
        assert!(protected.exists());
        assert!(dir.path().join("personal.mp3").exists());
    }
}
