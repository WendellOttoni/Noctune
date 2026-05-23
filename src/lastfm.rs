use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API_BASE: &str = "https://ws.audioscrobbler.com/2.0/";

#[derive(Debug, Clone)]
pub struct RecentTrack {
    pub artist: String,
    pub title: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct TopArtist {
    pub name: String,
    pub playcount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastfmSession {
    pub session_key: String,
    pub username: String,
}

#[derive(Debug, Clone)]
pub struct LastfmClient {
    api_key: String,
    api_secret: String,
    session: LastfmSession,
    client: reqwest::blocking::Client,
}

impl LastfmClient {
    pub fn new(api_key: String, api_secret: String, session: LastfmSession) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            api_key,
            api_secret,
            session,
            client,
        })
    }

    fn sign(&self, params: &BTreeMap<&str, String>) -> String {
        let mut s = String::new();
        for (k, v) in params {
            s.push_str(k);
            s.push_str(v);
        }
        s.push_str(&self.api_secret);
        md5_hex(s.as_bytes())
    }

    fn post(&self, mut params: BTreeMap<&str, String>) -> Result<serde_json::Value> {
        params.insert("api_key", self.api_key.clone());
        params.insert("format", "json".into());
        let sig = self.sign(&params);
        params.insert("api_sig", sig);
        let form: Vec<(&&str, &String)> = params.iter().collect();
        let resp = self
            .client
            .post(API_BASE)
            .form(&form)
            .send()
            .context("Last.fm POST")?;
        let v: serde_json::Value = resp.json().context("Last.fm JSON")?;
        if let Some(e) = v.get("error") {
            return Err(anyhow!(
                "Last.fm error {}: {}",
                e,
                v.get("message").unwrap_or(&serde_json::Value::Null)
            ));
        }
        Ok(v)
    }

    pub fn update_now_playing(&self, artist: &str, title: &str) -> Result<()> {
        let mut p = BTreeMap::new();
        p.insert("method", "track.updateNowPlaying".into());
        p.insert("artist", artist.to_string());
        p.insert("track", title.to_string());
        p.insert("sk", self.session.session_key.clone());
        self.post(p)?;
        Ok(())
    }

    /// Last `limit` scrobbled tracks. Issue #63.
    pub fn recent_tracks(&self, limit: u32) -> Result<Vec<RecentTrack>> {
        let url = format!(
            "{API_BASE}?method=user.getrecenttracks&user={}&api_key={}&format=json&limit={}",
            self.session.username, self.api_key, limit
        );
        let v: serde_json::Value = self.client.get(&url).send()?.json()?;
        let entries = v
            .get("recenttracks")
            .and_then(|r| r.get("track"))
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for e in entries {
            let artist = e
                .get("artist")
                .and_then(|a| a.get("#text").or_else(|| a.get("name")))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let name = e
                .get("name")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let ts = e
                .get("date")
                .and_then(|d| d.get("uts"))
                .and_then(|u| u.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            out.push(RecentTrack {
                artist,
                title: name,
                timestamp: ts,
            });
        }
        Ok(out)
    }

    /// Top artists over the given period (overall / 7day / 1month / 12month). Issue #63.
    pub fn top_artists(&self, period: &str, limit: u32) -> Result<Vec<TopArtist>> {
        let url = format!(
            "{API_BASE}?method=user.gettopartists&user={}&api_key={}&format=json&period={}&limit={}",
            self.session.username, self.api_key, period, limit
        );
        let v: serde_json::Value = self.client.get(&url).send()?.json()?;
        let entries = v
            .get("topartists")
            .and_then(|r| r.get("artist"))
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(entries
            .into_iter()
            .filter_map(|e| {
                let name = e.get("name").and_then(|s| s.as_str())?.to_string();
                let plays = e
                    .get("playcount")
                    .and_then(|s| s.as_str())
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                Some(TopArtist {
                    name,
                    playcount: plays,
                })
            })
            .collect())
    }

    pub fn scrobble(&self, artist: &str, title: &str, timestamp: u64) -> Result<()> {
        let mut p = BTreeMap::new();
        p.insert("method", "track.scrobble".into());
        p.insert("artist[0]", artist.to_string());
        p.insert("track[0]", title.to_string());
        p.insert("timestamp[0]", timestamp.to_string());
        p.insert("sk", self.session.session_key.clone());
        self.post(p)?;
        Ok(())
    }
}

// --- auth flow ---

pub fn get_token(api_key: &str, api_secret: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("method", "auth.getToken".into());
    params.insert("api_key", api_key.to_string());
    let sig = {
        let mut s = String::new();
        for (k, v) in &params {
            s.push_str(k);
            s.push_str(v);
        }
        s.push_str(api_secret);
        md5_hex(s.as_bytes())
    };
    params.insert("api_sig", sig);
    params.insert("format", "json".into());
    let form: Vec<(&&str, &String)> = params.iter().collect();
    let resp = client.post(API_BASE).form(&form).send()?;
    let v: serde_json::Value = resp.json()?;
    v.get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("no token in response: {v}"))
}

pub fn get_session(api_key: &str, api_secret: &str, token: &str) -> Result<LastfmSession> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("method", "auth.getSession".into());
    params.insert("api_key", api_key.to_string());
    params.insert("token", token.to_string());
    let sig = {
        let mut s = String::new();
        for (k, v) in &params {
            s.push_str(k);
            s.push_str(v);
        }
        s.push_str(api_secret);
        md5_hex(s.as_bytes())
    };
    params.insert("api_sig", sig);
    params.insert("format", "json".into());
    let form: Vec<(&&str, &String)> = params.iter().collect();
    let resp = client.post(API_BASE).form(&form).send()?;
    let v: serde_json::Value = resp.json()?;
    if let Some(e) = v.get("error") {
        return Err(anyhow!(
            "Last.fm auth error {}: {}",
            e,
            v.get("message").unwrap_or(&serde_json::Value::Null)
        ));
    }
    let sess = v
        .get("session")
        .ok_or_else(|| anyhow!("no session field"))?;
    let key = sess
        .get("key")
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .to_string();
    let name = sess
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    Ok(LastfmSession {
        session_key: key,
        username: name,
    })
}

pub fn session_path() -> Option<PathBuf> {
    crate::config::project_dirs()
        .ok()
        .map(|p| p.config_dir().join("lastfm.json"))
}

pub fn load_session() -> Option<LastfmSession> {
    let p = session_path()?;
    let s = fs::read_to_string(p).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn save_session(s: &LastfmSession) {
    if let Some(p) = session_path() {
        if let Some(parent) = p.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!(target: "lastfm", "failed to create dir {}: {e}", parent.display());
            }
        }
        match serde_json::to_string(s) {
            Ok(j) => {
                if let Err(e) = fs::write(&p, j) {
                    tracing::warn!(target: "lastfm", "failed to save session {}: {e}", p.display());
                }
            }
            Err(e) => tracing::warn!(target: "lastfm", "failed to serialize session: {e}"),
        }
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Simple MD5 implementation (no extra dep required since we have sha2 for Spotify).
/// We compute it via the standard algorithm rather than pulling in md5 crate.
fn md5_hex(data: &[u8]) -> String {
    // Use sha2 isn't md5 — Last.fm requires md5. Use a tiny pure-Rust impl.
    let digest = md5_compute(data);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn md5_compute(data: &[u8]) -> [u8; 16] {
    // Standard MD5 constants
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    let msg_len = data.len();
    let bit_len = (msg_len as u64).wrapping_mul(8);

    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    for chunk in padded.chunks(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            let b = &chunk[i * 4..i * 4 + 4];
            *w = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0u32..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                (a.wrapping_add(f)
                    .wrapping_add(K[i as usize])
                    .wrapping_add(m[g as usize]))
                .rotate_left(S[i as usize]),
            );
            a = temp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}
