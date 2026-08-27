use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

use crate::{audio::Track, config::VaultConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultTrack {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<u64>,
    pub cover_url: Option<String>,
    pub play_count: Option<u64>,
    pub direct_stream_url: Option<String>,
}

impl VaultTrack {
    pub fn to_track(&self, server_url: &str) -> Track {
        let stream_url = self
            .direct_stream_url
            .clone()
            .unwrap_or_else(|| format!("{}/api/vault/stream/{}", server_url.trim_end_matches('/'), self.id));
        Track {
            path: PathBuf::from(stream_url),
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone().or_else(|| Some("Cloud Vault".into())),
            duration: self.duration_secs.map(Duration::from_secs),
            genre: None,
            year: None,
            track_number: None,
        }
    }
}

#[derive(Clone)]
pub struct VaultClient {
    server_url: String,
    client: reqwest::blocking::Client,
}

impl VaultClient {
    pub fn new(cfg: &VaultConfig) -> Result<Self> {
        let server_url = cfg.server_url.trim().trim_end_matches('/').to_string();
        if server_url.is_empty() {
            return Err(anyhow!("Cloud Vault URL não configurada"));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            server_url,
            client,
        })
    }

    pub fn search(&self, query: &str) -> Result<Vec<Track>> {
        let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        let url = format!("{}/api/vault/tracks?q={}&limit=50", self.server_url, q);
        let res = self.client.get(&url).send().context("Erro ao consultar catálogo do Vault")?;
        if !res.status().is_success() {
            return Err(anyhow!("Vault retornou status {}", res.status()));
        }
        let items: Vec<VaultTrack> = res.json().unwrap_or_default();
        Ok(items.iter().map(|v| v.to_track(&self.server_url)).collect())
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<Track>> {
        let url = format!("{}/api/vault/tracks/recent?limit={limit}", self.server_url);
        let res = self.client.get(&url).send().context("Erro ao obter faixas recentes do Vault")?;
        if !res.status().is_success() {
            return Err(anyhow!("Vault retornou status {}", res.status()));
        }
        let items: Vec<VaultTrack> = res.json().unwrap_or_default();
        Ok(items.iter().map(|v| v.to_track(&self.server_url)).collect())
    }
}
