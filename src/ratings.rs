use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// 0 = unrated, 1-5 = stars, 6 = favorite
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Ratings {
    entries: HashMap<String, u8>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Ratings {
    pub fn load() -> Self {
        let path = ratings_path();
        let mut r: Ratings = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        r.path = path;
        r
    }

    pub fn save(&self) {
        let Some(p) = &self.path else { return };
        if let Some(parent) = p.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!(target: "ratings", "failed to create dir {}: {e}", parent.display());
            }
        }
        match serde_json::to_string(self) {
            Ok(s) => {
                if let Err(e) = fs::write(p, s) {
                    tracing::warn!(target: "ratings", "failed to save {}: {e}", p.display());
                }
            }
            Err(e) => tracing::warn!(target: "ratings", "failed to serialize: {e}"),
        }
    }

    pub fn get(&self, path: &Path) -> u8 {
        self.entries
            .get(&path.display().to_string())
            .copied()
            .unwrap_or(0)
    }

    pub fn set(&mut self, path: &Path, rating: u8) {
        let key = path.display().to_string();
        if rating == 0 {
            self.entries.remove(&key);
        } else {
            self.entries.insert(key, rating.min(6));
        }
        self.save();
    }

    pub fn toggle_favorite(&mut self, path: &Path) -> bool {
        let current = self.get(path);
        if current == 6 {
            self.set(path, 0);
            false
        } else {
            self.set(path, 6);
            true
        }
    }

    pub fn is_favorite(&self, path: &Path) -> bool {
        self.get(path) == 6
    }
}

fn ratings_path() -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|p| p.config_dir().join("ratings.json"))
}
