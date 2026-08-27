use anyhow::{anyhow, Context, Result};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

use crate::{audio::Track, config::SubsonicConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsonicSong {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<u64>,
    #[serde(rename = "coverArt")]
    pub cover_art: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    #[serde(rename = "bitRate")]
    pub bit_rate: Option<u32>,
    pub track: Option<u32>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsonicAlbum {
    pub id: String,
    pub title: Option<String>,
    pub name: Option<String>,
    pub artist: Option<String>,
    #[serde(rename = "coverArt")]
    pub cover_art: Option<String>,
    #[serde(rename = "songCount")]
    pub song_count: Option<u32>,
    pub duration: Option<u64>,
    pub year: Option<i32>,
}

impl SubsonicAlbum {
    pub fn display_title(&self) -> &str {
        self.title.as_deref().or(self.name.as_deref()).unwrap_or("Unknown Album")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsonicPlaylist {
    pub id: String,
    pub name: String,
    #[serde(rename = "songCount")]
    pub song_count: Option<u32>,
    pub duration: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum SubsonicFetchResult {
    Songs(Vec<Track>),
    Albums(Vec<SubsonicAlbum>),
    Playlists(Vec<SubsonicPlaylist>),
}

#[derive(Clone)]
pub struct SubsonicClient {
    server_url: String,
    username: String,
    password: String,
    client: reqwest::blocking::Client,
}

fn url_encode(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}

impl SubsonicClient {
    pub fn new(config: &SubsonicConfig) -> Result<Self> {
        let server_url = config.server_url.trim().trim_end_matches('/').to_string();
        if server_url.is_empty() || config.username.is_empty() {
            return Err(anyhow!("Subsonic / Navidrome não configurado (URL e usuário necessários)"));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()?;
        Ok(Self {
            server_url,
            username: config.username.clone(),
            password: config.password.clone(),
            client,
        })
    }

    fn auth_query(&self) -> String {
        let salt: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(6)
            .map(char::from)
            .collect();
        let digest = md5::compute(format!("{}{}", self.password, salt));
        let token = format!("{:x}", digest);
        format!(
            "u={}&t={}&s={}&v=1.16.1&c=noctune&f=json",
            url_encode(&self.username),
            token,
            salt
        )
    }

    pub fn ping(&self) -> Result<()> {
        let url = format!("{}/rest/ping.view?{}", self.server_url, self.auth_query());
        let res = self.client.get(&url).send().context("Falha ao conectar com servidor Subsonic")?;
        if !res.status().is_success() {
            return Err(anyhow!("Servidor retornou status HTTP {}", res.status()));
        }
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<Track>> {
        let q = url_encode(query);
        let url = format!(
            "{}/rest/search3.view?query={}&songCount=50&{}",
            self.server_url,
            q,
            self.auth_query()
        );
        let res = self.client.get(&url).send().context("Erro na busca do Subsonic")?;
        let json: serde_json::Value = res.json()?;
        let songs_json = json
            .pointer("/subsonic-response/searchResult3/song")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks = Vec::new();
        for item in songs_json {
            if let Ok(song) = serde_json::from_value::<SubsonicSong>(item) {
                tracks.push(self.song_to_track(&song));
            }
        }
        Ok(tracks)
    }

    pub fn get_album_list(&self, list_type: &str, size: usize) -> Result<Vec<SubsonicAlbum>> {
        let url = format!(
            "{}/rest/getAlbumList2.view?type={}&size={}&{}",
            self.server_url,
            list_type,
            size,
            self.auth_query()
        );
        let res = self
            .client
            .get(&url)
            .send()
            .context("Erro ao obter álbuns")?;
        let json: serde_json::Value = res.json()?;
        let albums_json = json
            .pointer("/subsonic-response/albumList2/album")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut albums = Vec::new();
        for item in albums_json {
            if let Ok(album) = serde_json::from_value::<SubsonicAlbum>(item) {
                albums.push(album);
            }
        }
        Ok(albums)
    }

    pub fn get_album_tracks(&self, album_id: &str) -> Result<Vec<Track>> {
        let url = format!(
            "{}/rest/getAlbum.view?id={}&{}",
            self.server_url,
            album_id,
            self.auth_query()
        );
        let res = self
            .client
            .get(&url)
            .send()
            .context("Erro ao obter faixas do álbum")?;
        let json: serde_json::Value = res.json()?;
        let songs_json = json
            .pointer("/subsonic-response/album/song")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks = Vec::new();
        for item in songs_json {
            if let Ok(song) = serde_json::from_value::<SubsonicSong>(item) {
                tracks.push(self.song_to_track(&song));
            }
        }
        Ok(tracks)
    }

    pub fn get_playlists(&self) -> Result<Vec<SubsonicPlaylist>> {
        let url = format!(
            "{}/rest/getPlaylists.view?{}",
            self.server_url,
            self.auth_query()
        );
        let res = self
            .client
            .get(&url)
            .send()
            .context("Erro ao obter playlists")?;
        let json: serde_json::Value = res.json()?;
        let playlists_json = json
            .pointer("/subsonic-response/playlists/playlist")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut playlists = Vec::new();
        for item in playlists_json {
            if let Ok(pl) = serde_json::from_value::<SubsonicPlaylist>(item) {
                playlists.push(pl);
            }
        }
        Ok(playlists)
    }

    pub fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>> {
        let url = format!(
            "{}/rest/getPlaylist.view?id={}&{}",
            self.server_url,
            playlist_id,
            self.auth_query()
        );
        let res = self
            .client
            .get(&url)
            .send()
            .context("Erro ao obter faixas da playlist")?;
        let json: serde_json::Value = res.json()?;
        let songs_json = json
            .pointer("/subsonic-response/playlist/entry")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks = Vec::new();
        for item in songs_json {
            if let Ok(song) = serde_json::from_value::<SubsonicSong>(item) {
                tracks.push(self.song_to_track(&song));
            }
        }
        Ok(tracks)
    }

    pub fn get_random_songs(&self, size: usize) -> Result<Vec<Track>> {
        let url = format!(
            "{}/rest/getRandomSongs.view?size={}&{}",
            self.server_url,
            size,
            self.auth_query()
        );
        let res = self
            .client
            .get(&url)
            .send()
            .context("Erro ao obter músicas aleatórias")?;
        let json: serde_json::Value = res.json()?;
        let songs_json = json
            .pointer("/subsonic-response/randomSongs/song")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks = Vec::new();
        for item in songs_json {
            if let Ok(song) = serde_json::from_value::<SubsonicSong>(item) {
                tracks.push(self.song_to_track(&song));
            }
        }
        Ok(tracks)
    }

    pub fn stream_url(&self, song_id: &str) -> String {
        format!(
            "{}/rest/stream.view?id={}&{}",
            self.server_url,
            song_id,
            self.auth_query()
        )
    }

    pub fn song_to_track(&self, song: &SubsonicSong) -> Track {
        let stream_link = self.stream_url(&song.id);
        Track {
            path: PathBuf::from(stream_link),
            title: song.title.clone(),
            artist: song.artist.clone(),
            album: song.album.clone(),
            genre: None,
            year: song.year.map(|y| y.to_string()),
            duration: song.duration.map(Duration::from_secs),
            replaygain_track_db: None,
            replaygain_album_db: None,
            cover_url: None,
            added_at: None,
        }
    }
}
