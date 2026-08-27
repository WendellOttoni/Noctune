use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{Author, SharedPlaylist, Visibility};

pub const DEFAULT_SHARE_API_URL: &str = "https://share.noctune.dev";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedPlaylistSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub author: Author,
    pub track_count: usize,
    pub duration_secs: Option<u64>,
    pub visibility: Visibility,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub likes: u64,
    pub updated_at: Option<String>,
}

#[derive(Clone)]
pub struct ShareClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl ShareClient {
    pub fn new(base_url: Option<&str>) -> Result<Self> {
        let url = base_url
            .unwrap_or(DEFAULT_SHARE_API_URL)
            .trim_end_matches('/')
            .to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()?;
        Ok(Self {
            base_url: url,
            client,
        })
    }

    pub fn publish(&self, playlist: &SharedPlaylist, token: Option<&str>) -> Result<String> {
        let url = format!("{}/api/playlists/publish", self.base_url);
        let mut req = self.client.post(&url).json(playlist);
        if let Some(t) = token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        let res = req.send().context("Falha ao publicar playlist")?;
        if !res.status().is_success() {
            return Err(anyhow!("Servidor retornou status HTTP {}", res.status()));
        }
        #[derive(Deserialize)]
        struct PublishResp {
            id: String,
        }
        let resp: PublishResp = res.json()?;
        Ok(resp.id)
    }

    pub fn search(
        &self,
        query: &str,
        tag: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SharedPlaylistSummary>> {
        let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        let mut url = format!(
            "{}/api/playlists/search?q={}&limit={}",
            self.base_url, q, limit
        );
        if let Some(t) = tag {
            let t_enc: String = url::form_urlencoded::byte_serialize(t.as_bytes()).collect();
            url.push_str(&format!("&tag={}", t_enc));
        }
        let res = self
            .client
            .get(&url)
            .send()
            .context("Falha ao buscar playlists públicas")?;
        if !res.status().is_success() {
            return Err(anyhow!("Servidor retornou status HTTP {}", res.status()));
        }
        let items: Vec<SharedPlaylistSummary> = res.json().unwrap_or_default();
        Ok(items)
    }

    pub fn get(&self, id: &str) -> Result<SharedPlaylist> {
        let url = format!("{}/api/playlists/{}", self.base_url, id);
        let res = self
            .client
            .get(&url)
            .send()
            .context("Falha ao carregar playlist compartilhada")?;
        if !res.status().is_success() {
            return Err(anyhow!(
                "Playlist não encontrada ou indisponível (HTTP {})",
                res.status()
            ));
        }
        let pl: SharedPlaylist = res.json()?;
        Ok(pl)
    }
}
