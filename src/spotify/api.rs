use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::time::Duration;

use super::{refresh_token, save_tokens, Tokens};

const API_BASE: &str = "https://api.spotify.com/v1";

pub struct SpotifyApi {
    client_id: String,
    client: reqwest::blocking::Client,
    tokens: Tokens,
}

#[derive(Debug, Deserialize)]
pub struct CurrentlyPlaying {
    pub is_playing: bool,
    #[allow(dead_code)]
    pub progress_ms: Option<u64>,
    #[allow(dead_code)]
    pub item: Option<TrackObject>,
}

#[derive(Debug, Deserialize)]
pub struct TrackObject {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub duration_ms: u64,
    #[allow(dead_code)]
    pub artists: Vec<ArtistRef>,
}

#[derive(Debug, Deserialize)]
pub struct ArtistRef {
    #[allow(dead_code)]
    pub name: String,
}

impl SpotifyApi {
    pub fn new(client_id: String, tokens: Tokens) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            client_id,
            client,
            tokens,
        })
    }

    fn ensure_fresh(&mut self) -> Result<()> {
        if !self.tokens.is_expired() {
            return Ok(());
        }
        let refresh = self
            .tokens
            .refresh_token
            .clone()
            .ok_or_else(|| anyhow!("no refresh token; re-authenticate"))?;
        let new_tokens = refresh_token(&self.client_id, &refresh)?;
        save_tokens(&new_tokens).ok();
        self.tokens = new_tokens;
        Ok(())
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.tokens.access_token)
    }

    pub fn currently_playing(&mut self) -> Result<Option<CurrentlyPlaying>> {
        self.ensure_fresh()?;
        let resp = self
            .client
            .get(format!("{API_BASE}/me/player/currently-playing"))
            .header("Authorization", self.auth_header())
            .send()
            .context("GET /me/player/currently-playing")?;

        if resp.status().as_u16() == 204 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let s = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("currently-playing failed ({s}): {body}"));
        }
        let parsed: CurrentlyPlaying = resp.json()?;
        Ok(Some(parsed))
    }

    pub fn play(&mut self) -> Result<()> {
        self.empty_put("/me/player/play")
    }

    pub fn pause(&mut self) -> Result<()> {
        self.empty_put("/me/player/pause")
    }

    #[allow(dead_code)]
    pub fn next(&mut self) -> Result<()> {
        self.empty_post("/me/player/next")
    }

    #[allow(dead_code)]
    pub fn previous(&mut self) -> Result<()> {
        self.empty_post("/me/player/previous")
    }

    fn empty_put(&mut self, path: &str) -> Result<()> {
        self.ensure_fresh()?;
        let resp = self
            .client
            .put(format!("{API_BASE}{path}"))
            .header("Authorization", self.auth_header())
            .header("Content-Length", "0")
            .send()
            .with_context(|| format!("PUT {path}"))?;
        if !resp.status().is_success() && resp.status().as_u16() != 204 {
            return Err(anyhow!("{path} returned {}", resp.status()));
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn empty_post(&mut self, path: &str) -> Result<()> {
        self.ensure_fresh()?;
        let resp = self
            .client
            .post(format!("{API_BASE}{path}"))
            .header("Authorization", self.auth_header())
            .header("Content-Length", "0")
            .send()
            .with_context(|| format!("POST {path}"))?;
        if !resp.status().is_success() && resp.status().as_u16() != 204 {
            return Err(anyhow!("{path} returned {}", resp.status()));
        }
        Ok(())
    }
}
