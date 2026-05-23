use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

use super::{refresh_token, save_tokens, Tokens};

const API_BASE: &str = "https://api.spotify.com/v1";

#[derive(Clone)]
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

    /// Play a specific Spotify URI on the active Connect device.
    pub fn play_uri(&mut self, uri: &str) -> Result<()> {
        self.ensure_fresh()?;
        #[derive(Serialize)]
        struct Body<'a> {
            uris: &'a [&'a str],
        }
        let body = Body { uris: &[uri] };
        let resp = self
            .client
            .put(format!("{API_BASE}/me/player/play"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .context("PUT /me/player/play")?;
        if !resp.status().is_success() && resp.status().as_u16() != 204 {
            let s = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(anyhow!("play_uri failed ({s}): {text}"));
        }
        Ok(())
    }

    pub fn track_by_id(&mut self, id: &str) -> Result<crate::audio::Track> {
        self.ensure_fresh()?;
        #[derive(Deserialize)]
        struct SpTrack {
            name: String,
            duration_ms: u64,
            uri: String,
            artists: Vec<ArtistRef>,
            album: SpAlbum,
        }
        #[derive(Deserialize)]
        struct SpAlbum {
            name: String,
        }
        let resp = self
            .client
            .get(format!("{API_BASE}/tracks/{id}"))
            .header("Authorization", self.auth_header())
            .send()
            .with_context(|| format!("GET /tracks/{id}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("tracks/{id} returned {}", resp.status()));
        }
        let t: SpTrack = resp.json()?;
        let artist = t.artists.first().map(|a| a.name.clone());
        Ok(crate::audio::Track {
            path: PathBuf::from(&t.uri),
            title: t.name,
            artist,
            album: Some(t.album.name),
            genre: None,
            year: None,
            duration: Some(Duration::from_millis(t.duration_ms)),
            replaygain_track_db: None,
            replaygain_album_db: None,
            cover_url: None,
            added_at: None,
        })
    }

    pub fn playlist_tracks(&mut self, playlist_id: &str) -> Result<Vec<crate::audio::Track>> {
        self.ensure_fresh()?;
        let mut tracks = Vec::new();
        let mut url = Some(format!("{API_BASE}/playlists/{playlist_id}/tracks?limit=50&fields=next,items(track(uri,name,duration_ms,artists,album))"));
        while let Some(endpoint) = url {
            #[derive(Deserialize)]
            struct Page {
                next: Option<String>,
                items: Vec<Item>,
            }
            #[derive(Deserialize)]
            struct Item {
                track: Option<SpSimpleTrack>,
            }
            #[derive(Deserialize)]
            struct SpSimpleTrack {
                uri: String,
                name: String,
                duration_ms: u64,
                artists: Vec<ArtistRef>,
                album: SpAlbum,
            }
            #[derive(Deserialize)]
            struct SpAlbum { name: String }
            let resp = self
                .client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .with_context(|| format!("GET {endpoint}"))?;
            if !resp.status().is_success() {
                return Err(anyhow!("playlist tracks returned {}", resp.status()));
            }
            let page: Page = resp.json()?;
            url = page.next;
            for item in page.items {
                let Some(t) = item.track else { continue };
                if t.uri.is_empty() || !t.uri.starts_with("spotify:track:") { continue }
                tracks.push(crate::audio::Track {
                    path: PathBuf::from(&t.uri),
                    title: t.name,
                    artist: t.artists.first().map(|a| a.name.clone()),
                    album: Some(t.album.name),
                    genre: None,
                    year: None,
                    duration: Some(Duration::from_millis(t.duration_ms)),
                    replaygain_track_db: None,
                    replaygain_album_db: None,
                    cover_url: None,
                    added_at: None,
                });
            }
        }
        Ok(tracks)
    }

    pub fn album_tracks(&mut self, album_id: &str) -> Result<Vec<crate::audio::Track>> {
        self.ensure_fresh()?;
        let mut tracks = Vec::new();
        let mut url = Some(format!("{API_BASE}/albums/{album_id}/tracks?limit=50"));
        while let Some(endpoint) = url {
            #[derive(Deserialize)]
            struct Page {
                next: Option<String>,
                items: Vec<SpSimpleTrack>,
            }
            #[derive(Deserialize)]
            struct SpSimpleTrack {
                uri: String,
                name: String,
                duration_ms: u64,
                artists: Vec<ArtistRef>,
            }
            let resp = self
                .client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .with_context(|| format!("GET {endpoint}"))?;
            if !resp.status().is_success() {
                return Err(anyhow!("album tracks returned {}", resp.status()));
            }
            let page: Page = resp.json()?;
            url = page.next;
            for t in page.items {
                tracks.push(crate::audio::Track {
                    path: PathBuf::from(&t.uri),
                    title: t.name,
                    artist: t.artists.first().map(|a| a.name.clone()),
                    album: None,
                    genre: None,
                    year: None,
                    duration: Some(Duration::from_millis(t.duration_ms)),
                    replaygain_track_db: None,
                    replaygain_album_db: None,
                    cover_url: None,
                    added_at: None,
                });
            }
        }
        Ok(tracks)
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

    /// Search tracks, albums, and playlists. Returns up to `limit` tracks.
    pub fn search(&mut self, query: &str, limit: u32) -> Result<Vec<crate::audio::Track>> {
        self.ensure_fresh()?;
        #[derive(Deserialize)]
        struct SearchResult {
            tracks: TrackPage,
        }
        #[derive(Deserialize)]
        struct TrackPage {
            items: Vec<SpTrack>,
        }
        #[derive(Deserialize)]
        struct SpTrack {
            uri: String,
            name: String,
            duration_ms: u64,
            artists: Vec<ArtistRef>,
            album: SpAlbum,
        }
        #[derive(Deserialize)]
        struct SpAlbum { name: String }

        let resp = self.client
            .get(format!("{API_BASE}/search"))
            .header("Authorization", self.auth_header())
            .query(&[("q", query), ("type", "track"), ("limit", &limit.to_string())])
            .send()
            .context("GET /search")?;
        if !resp.status().is_success() {
            return Err(anyhow!("search failed: {}", resp.status()));
        }
        let result: SearchResult = resp.json()?;
        Ok(result.tracks.items.into_iter().map(|t| crate::audio::Track {
            path: PathBuf::from(&t.uri),
            title: t.name,
            artist: t.artists.first().map(|a| a.name.clone()),
            album: Some(t.album.name),
            genre: None,
            year: None,
            duration: Some(Duration::from_millis(t.duration_ms)),
            replaygain_track_db: None,
            replaygain_album_db: None,
            cover_url: None,
            added_at: None,
        }).collect())
    }

    /// Get the current user's saved (liked) tracks.
    pub fn liked_tracks(&mut self, limit: u32) -> Result<Vec<crate::audio::Track>> {
        self.ensure_fresh()?;
        let mut tracks = Vec::new();
        let mut url = Some(format!("{API_BASE}/me/tracks?limit={limit}"));
        while let Some(endpoint) = url {
            #[derive(Deserialize)]
            struct Page { next: Option<String>, items: Vec<SavedItem> }
            #[derive(Deserialize)]
            struct SavedItem { track: SpTrack }
            #[derive(Deserialize)]
            struct SpTrack {
                uri: String, name: String, duration_ms: u64,
                artists: Vec<ArtistRef>, album: SpAlbum,
            }
            #[derive(Deserialize)]
            struct SpAlbum { name: String }

            let resp = self.client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .context("GET /me/tracks")?;
            if !resp.status().is_success() {
                return Err(anyhow!("liked tracks failed: {}", resp.status()));
            }
            let page: Page = resp.json()?;
            url = page.next;
            for item in page.items {
                let t = item.track;
                tracks.push(crate::audio::Track {
                    path: PathBuf::from(&t.uri),
                    title: t.name,
                    artist: t.artists.first().map(|a| a.name.clone()),
                    album: Some(t.album.name),
                    genre: None, year: None,
                    duration: Some(Duration::from_millis(t.duration_ms)),
                    replaygain_track_db: None, replaygain_album_db: None,
                    cover_url: None,
                    added_at: None,
                });
            }
        }
        Ok(tracks)
    }

    /// Get the current user's playlists as (id, name, track_count) tuples.
    pub fn my_playlists(&mut self) -> Result<Vec<(String, String, u32)>> {
        self.ensure_fresh()?;
        let mut playlists = Vec::new();
        let mut url = Some(format!("{API_BASE}/me/playlists?limit=50"));
        while let Some(endpoint) = url {
            #[derive(Deserialize)]
            struct Page { next: Option<String>, items: Vec<SpPlaylist> }
            #[derive(Deserialize)]
            struct SpPlaylist { id: String, name: String, tracks: TrackCount }
            #[derive(Deserialize)]
            struct TrackCount { total: u32 }

            let resp = self.client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .context("GET /me/playlists")?;
            if !resp.status().is_success() {
                return Err(anyhow!("my_playlists failed: {}", resp.status()));
            }
            let page: Page = resp.json()?;
            url = page.next;
            for p in page.items {
                playlists.push((p.id, p.name, p.tracks.total));
            }
        }
        Ok(playlists)
    }

    /// Set playback volume on the active Connect device (0–100).
    #[allow(dead_code)]
    pub fn set_volume(&mut self, pct: u8) -> Result<()> {
        self.ensure_fresh()?;
        let resp = self.client
            .put(format!("{API_BASE}/me/player/volume"))
            .header("Authorization", self.auth_header())
            .query(&[("volume_percent", pct.to_string())])
            .header("Content-Length", "0")
            .send()
            .context("PUT /me/player/volume")?;
        if !resp.status().is_success() && resp.status().as_u16() != 204 {
            return Err(anyhow!("set_volume failed: {}", resp.status()));
        }
        Ok(())
    }

    /// Parse any Spotify URL or URI and return the (type, id) pair.
    /// Handles: spotify:track:ID, open.spotify.com/track/ID, etc.
    #[allow(dead_code)]
    pub fn parse_spotify_input(input: &str) -> Option<(&'static str, String)> {
        let input = input.trim();
        // URI form: spotify:TYPE:ID
        if let Some(rest) = input.strip_prefix("spotify:") {
            let mut parts = rest.splitn(2, ':');
            let kind = parts.next()?;
            let id = parts.next()?.split('?').next()?.to_string();
            return Some((match kind {
                "track" => "track", "album" => "album",
                "playlist" => "playlist", "artist" => "artist",
                _ => return None,
            }, id));
        }
        // URL form: https://open.spotify.com/TYPE/ID
        if input.contains("open.spotify.com") {
            let path = input.split("open.spotify.com").nth(1)?;
            let path = path.trim_start_matches('/');
            let mut parts = path.splitn(2, '/');
            let kind = parts.next()?;
            let id = parts.next()?.split('?').next()?.to_string();
            return Some((match kind {
                "track" => "track", "album" => "album",
                "playlist" => "playlist", "artist" => "artist",
                _ => return None,
            }, id));
        }
        None
    }
}
