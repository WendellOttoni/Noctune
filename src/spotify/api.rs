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
    legacy_playlist_api: bool,
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
            legacy_playlist_api: false,
        })
    }

    pub fn with_legacy_playlists(mut self, enabled: bool) -> Self {
        self.legacy_playlist_api = enabled;
        self
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
        if let Err(e) = save_tokens(&new_tokens) {
            tracing::warn!(target: "spotify", "failed to save refreshed tokens: {e}");
        }
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
            return Err(response_error(resp));
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
            return Err(response_error(resp));
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
            return Err(response_error(resp));
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
            return Err(response_error(resp));
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
        let endpoint_kind = if self.legacy_playlist_api {
            "tracks"
        } else {
            "items"
        };
        let mut url = Some(format!(
            "{API_BASE}/playlists/{playlist_id}/{endpoint_kind}?limit=50"
        ));
        let mut visited = std::collections::HashSet::new();
        while let Some(endpoint) = url {
            validate_page_url(&endpoint)?;
            anyhow::ensure!(
                visited.insert(endpoint.clone()),
                "Spotify returned a pagination loop"
            );
            let resp = self
                .client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .with_context(|| format!("GET {endpoint}"))?;
            if !resp.status().is_success() {
                return Err(response_error(resp));
            }
            let page: PlaylistPage = resp
                .json()
                .context("Spotify playlist response is incompatible")?;
            url = page.next;
            for item in page.items {
                let Some(t) = item.track else { continue };
                if t.uri.is_empty() || !t.uri.starts_with("spotify:track:") {
                    continue;
                }
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
            validate_page_url(&endpoint)?;
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
                return Err(response_error(resp));
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
            return Err(response_error(resp));
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
        struct SpAlbum {
            name: String,
        }

        let resp = self
            .client
            .get(format!("{API_BASE}/search"))
            .header("Authorization", self.auth_header())
            .query(&[
                ("q", query),
                ("type", "track"),
                ("limit", &limit.clamp(1, 10).to_string()),
            ])
            .send()
            .context("GET /search")?;
        if !resp.status().is_success() {
            return Err(response_error(resp));
        }
        let result: SearchResult = resp.json()?;
        Ok(result
            .tracks
            .items
            .into_iter()
            .map(|t| crate::audio::Track {
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
            })
            .collect())
    }

    /// Get the current user's saved (liked) tracks.
    pub fn liked_tracks(&mut self, limit: u32) -> Result<Vec<crate::audio::Track>> {
        self.ensure_fresh()?;
        let mut tracks = Vec::new();
        let mut url = Some(format!("{API_BASE}/me/tracks?limit={limit}"));
        while let Some(endpoint) = url {
            validate_page_url(&endpoint)?;
            #[derive(Deserialize)]
            struct Page {
                next: Option<String>,
                items: Vec<SavedItem>,
            }
            #[derive(Deserialize)]
            struct SavedItem {
                track: SpTrack,
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
            struct SpAlbum {
                name: String,
            }

            let resp = self
                .client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .context("GET /me/tracks")?;
            if !resp.status().is_success() {
                return Err(response_error(resp));
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

    /// Get the current user's playlists as (id, name, track_count) tuples.
    pub fn my_playlists(&mut self) -> Result<Vec<(String, String, u32)>> {
        self.ensure_fresh()?;
        let mut playlists = Vec::new();
        let mut url = Some(format!("{API_BASE}/me/playlists?limit=50"));
        while let Some(endpoint) = url {
            validate_page_url(&endpoint)?;
            #[derive(Deserialize)]
            struct Page {
                next: Option<String>,
                items: Vec<SpPlaylist>,
            }
            #[derive(Deserialize)]
            struct SpPlaylist {
                id: String,
                name: String,
                #[serde(rename = "items", alias = "tracks", default)]
                tracks: TrackCount,
            }
            #[derive(Deserialize, Default)]
            struct TrackCount {
                total: u32,
            }

            let resp = self
                .client
                .get(&endpoint)
                .header("Authorization", self.auth_header())
                .send()
                .context("GET /me/playlists")?;
            if !resp.status().is_success() {
                return Err(response_error(resp));
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
        let resp = self
            .client
            .put(format!("{API_BASE}/me/player/volume"))
            .header("Authorization", self.auth_header())
            .query(&[("volume_percent", pct.to_string())])
            .header("Content-Length", "0")
            .send()
            .context("PUT /me/player/volume")?;
        if !resp.status().is_success() && resp.status().as_u16() != 204 {
            return Err(response_error(resp));
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
            return Some((
                match kind {
                    "track" => "track",
                    "album" => "album",
                    "playlist" => "playlist",
                    "artist" => "artist",
                    _ => return None,
                },
                id,
            ));
        }
        // URL form: https://open.spotify.com/TYPE/ID
        if input.contains("open.spotify.com") {
            let path = input.split("open.spotify.com").nth(1)?;
            let path = path.trim_start_matches('/');
            let mut parts = path.splitn(2, '/');
            let kind = parts.next()?;
            let id = parts.next()?.split('?').next()?.to_string();
            return Some((
                match kind {
                    "track" => "track",
                    "album" => "album",
                    "playlist" => "playlist",
                    "artist" => "artist",
                    _ => return None,
                },
                id,
            ));
        }
        None
    }
}

#[derive(Deserialize)]
struct PlaylistPage {
    next: Option<String>,
    items: Vec<PlaylistItem>,
}
#[derive(Deserialize)]
struct PlaylistItem {
    #[serde(rename = "item", alias = "track", default)]
    track: Option<PlaylistTrack>,
}
#[derive(Deserialize)]
struct PlaylistTrack {
    uri: String,
    name: String,
    duration_ms: u64,
    #[serde(default)]
    artists: Vec<ArtistRef>,
    #[serde(default)]
    album: PlaylistAlbum,
}
#[derive(Default, Deserialize)]
struct PlaylistAlbum {
    #[serde(default)]
    name: String,
}

fn validate_page_url(endpoint: &str) -> Result<()> {
    let parsed = url::Url::parse(endpoint)?;
    anyhow::ensure!(
        parsed.scheme() == "https"
            && parsed.host_str() == Some("api.spotify.com")
            && parsed.port_or_known_default() == Some(443)
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.path().starts_with("/v1/"),
        "Spotify returned an untrusted pagination URL"
    );
    Ok(())
}

fn response_error(response: reqwest::blocking::Response) -> anyhow::Error {
    let code = response.status().as_u16();
    let wait = response
        .headers()
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let hint = match code {
        401 => "Session expired; sign in again.",
        403 => "Permission denied; check scopes, app allowlist and Development Mode Premium requirement.",
        404 => "Resource or active Connect device unavailable; open Spotify and select a device.",
        429 => "Spotify quota reached; wait before retrying.",
        _ => "Spotify request failed; try again later.",
    };
    if let Some(seconds) = wait {
        anyhow!("Spotify HTTP {code}: {hint} Retry after {seconds}s.")
    } else {
        anyhow!("Spotify HTTP {code}: {hint}")
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    #[test]
    fn playlist_accepts_new_and_legacy_items_and_unavailable_tracks() {
        for field in ["item", "track"] {
            let json = format!(
                r#"{{"next":null,"items":[{{"{field}":{{"uri":"spotify:track:a","name":"A","duration_ms":1000,"artists":[],"album":{{"name":"B"}}}}}},{{"{field}":null}}]}}"#
            );
            let page: PlaylistPage = serde_json::from_str(&json).unwrap();
            assert_eq!(page.items[0].track.as_ref().unwrap().name, "A");
            assert!(page.items[1].track.is_none());
        }
        assert!(serde_json::from_str::<PlaylistPage>(r#"{"error":"denied"}"#).is_err());
    }
    #[test]
    fn pagination_never_sends_token_to_other_hosts() {
        assert!(validate_page_url("https://api.spotify.com/v1/me/playlists?offset=50").is_ok());
        for url in [
            "https://example.com/v1/me",
            "http://api.spotify.com/v1/me",
            "https://api.spotify.com.evil.test/v1/me",
            "https://user@api.spotify.com/v1/me",
        ] {
            assert!(validate_page_url(url).is_err());
        }
    }
}
