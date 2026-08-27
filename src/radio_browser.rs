//! Online Radio Directory & Curated Station Hub.
//! Connects to the community-driven Radio-Browser API (+45,000 global stations)
//! and provides built-in curated plug-and-play streaming stations.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioStation {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub votes: Option<u32>,
    #[serde(default)]
    pub stationuuid: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioCategory {
    TopVoted,
    Lofi,
    Rock,
    Jazz,
    Synthwave,
    Brazil,
    Classical,
    Curated,
    Custom,
    Favorites,
    Search,
}

impl RadioCategory {
    pub const ALL: [RadioCategory; 11] = [
        RadioCategory::TopVoted,
        RadioCategory::Lofi,
        RadioCategory::Rock,
        RadioCategory::Jazz,
        RadioCategory::Synthwave,
        RadioCategory::Brazil,
        RadioCategory::Classical,
        RadioCategory::Curated,
        RadioCategory::Custom,
        RadioCategory::Favorites,
        RadioCategory::Search,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            RadioCategory::TopVoted => "🔥 Top 100 Globais",
            RadioCategory::Lofi => "☕ Lo-Fi & Beats",
            RadioCategory::Rock => "🎸 Rock & Metal",
            RadioCategory::Jazz => "🎷 Jazz & Blues",
            RadioCategory::Synthwave => "⚡ Synthwave & Retrowave",
            RadioCategory::Brazil => "🇧🇷 Brasil & MPB",
            RadioCategory::Classical => "🎻 Clássica & Piano",
            RadioCategory::Curated => "★ Seleção Manual",
            RadioCategory::Custom => "✨ Minhas Rádios",
            RadioCategory::Favorites => "♥ Favoritas",
            RadioCategory::Search => "🔍 Busca Livre (+45k)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioTab {
    Curated,
    Search,
}

impl RadioTab {
    pub fn cycle(self) -> Self {
        match self {
            RadioTab::Curated => RadioTab::Search,
            RadioTab::Search => RadioTab::Curated,
        }
    }
}

/// Returns a curated selection of reliable, high-quality online radio streams.
pub fn curated_stations() -> Vec<RadioStation> {
    vec![
        // Lo-Fi & Chill
        RadioStation {
            name: "Lofi Girl (Chill Beats)".into(),
            url: "https://play.streamafrica.net/lofiradio".into(),
            homepage: Some("https://lofigirl.com".into()),
            tags: "lofi, chill, instrumental, study".into(),
            country: Some("France".into()),
            bitrate: Some(192),
            codec: Some("MP3".into()),
            votes: Some(15000),
            stationuuid: None,
        },
        RadioStation {
            name: "Nightwave Plaza (Vaporwave)".into(),
            url: "https://radio.plaza.one/mp3".into(),
            homepage: Some("https://plaza.one".into()),
            tags: "vaporwave, future funk, chill".into(),
            country: Some("Worldwide".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(9500),
            stationuuid: None,
        },
        RadioStation {
            name: "SomaFM: Groove Salad".into(),
            url: "https://ice6.somafm.com/groovesalad-256-mp3".into(),
            homepage: Some("https://somafm.com/groovesalad/".into()),
            tags: "ambient, downtempo, chill".into(),
            country: Some("United States".into()),
            bitrate: Some(256),
            codec: Some("MP3".into()),
            votes: Some(18000),
            stationuuid: None,
        },
        RadioStation {
            name: "Chillhop Radio".into(),
            url: "http://stream.zeno.fm/f3wvbbqmdg8uv".into(),
            homepage: Some("https://chillhop.com".into()),
            tags: "chillhop, lofi, beats".into(),
            country: Some("Netherlands".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(8200),
            stationuuid: None,
        },
        // Synthwave & Cyberpunk
        RadioStation {
            name: "Nightride FM (Synthwave / Retrowave)".into(),
            url: "https://stream.nightride.fm/nightride.mp3".into(),
            homepage: Some("https://nightride.fm".into()),
            tags: "synthwave, retrowave, cyberpunk, 80s".into(),
            country: Some("United States".into()),
            bitrate: Some(256),
            codec: Some("MP3".into()),
            votes: Some(12000),
            stationuuid: None,
        },
        RadioStation {
            name: "SomaFM: DEF CON Radio".into(),
            url: "https://ice6.somafm.com/defcon-256-mp3".into(),
            homepage: Some("https://somafm.com/defcon/".into()),
            tags: "hacker, electronic, ambient, darkwave".into(),
            country: Some("United States".into()),
            bitrate: Some(256),
            codec: Some("MP3".into()),
            votes: Some(11000),
            stationuuid: None,
        },
        // Jazz & Classical
        RadioStation {
            name: "SomaFM: Secret Agent".into(),
            url: "https://ice6.somafm.com/secretagent-128-mp3".into(),
            homepage: Some("https://somafm.com/secretagent/".into()),
            tags: "spy jazz, lounge, surf, 60s".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(7800),
            stationuuid: None,
        },
        RadioStation {
            name: "Radio Swiss Jazz".into(),
            url: "http://stream.srg-ssr.ch/m/rsj/mp3_128".into(),
            homepage: Some("https://www.radioswissjazz.ch".into()),
            tags: "jazz, swing, bebop, blues".into(),
            country: Some("Switzerland".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(14000),
            stationuuid: None,
        },
        RadioStation {
            name: "Radio Swiss Classic".into(),
            url: "http://stream.srg-ssr.ch/m/rsc_de/mp3_128".into(),
            homepage: Some("https://www.radioswissclassic.ch".into()),
            tags: "classical, orchestral, baroque".into(),
            country: Some("Switzerland".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(13500),
            stationuuid: None,
        },
        RadioStation {
            name: "Classic FM (UK)".into(),
            url: "http://media-ice.musicradio.com/ClassicFMMP3".into(),
            homepage: Some("https://www.classicfm.com".into()),
            tags: "classical, orchestral, relax, symphony".into(),
            country: Some("United Kingdom".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(16000),
            stationuuid: None,
        },
        RadioStation {
            name: "WQXR 105.9 FM (Classical)".into(),
            url: "https://stream.wqxr.org/wqxr".into(),
            homepage: Some("https://www.wqxr.org".into()),
            tags: "classical, opera, orchestral, piano".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(10500),
            stationuuid: None,
        },
        // Rock, Metal & Pop
        RadioStation {
            name: "SomaFM: Indie Pop Rocks!".into(),
            url: "https://ice6.somafm.com/indiepop-128-mp3".into(),
            homepage: Some("https://somafm.com/indiepop/".into()),
            tags: "indie rock, pop, alternative".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(9200),
            stationuuid: None,
        },
        RadioStation {
            name: "SomaFM: Metal Detector".into(),
            url: "https://ice6.somafm.com/metal-128-mp3".into(),
            homepage: Some("https://somafm.com/metal/".into()),
            tags: "metal, hard rock, heavy".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(8900),
            stationuuid: None,
        },
        RadioStation {
            name: "Kiss FM 92.5 (Brasil)".into(),
            url: "https://26593.live.streamtheworld.com/RADIO_KISSFM_ADP.aac".into(),
            homepage: Some("https://kissfm.com.br".into()),
            tags: "rock, classic rock, hard rock, metal".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
            codec: Some("AAC".into()),
            votes: Some(6400),
            stationuuid: None,
        },
        // Brazil Stations
        RadioStation {
            name: "Antena 1 FM 94.7 (Brasil)".into(),
            url: "http://antena1.newradio.it/stream?ext=.mp3".into(),
            homepage: Some("https://www.antena1.com.br".into()),
            tags: "pop, adult contemporary, international".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(11200),
            stationuuid: None,
        },
        RadioStation {
            name: "Rádio Eldorado FM (Brasil)".into(),
            url: "https://cast4.audiostream.com.br:2652/mp3".into(),
            homepage: Some("https://www.eldorado.com.br".into()),
            tags: "mpb, rock, news, culture".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(7100),
            stationuuid: None,
        },
        RadioStation {
            name: "NovaBrasil FM (Brasil)".into(),
            url: "https://playerservices.streamtheworld.com/api/livestream-redirect/NOVABRASIL_FORAAC.aac".into(),
            homepage: Some("https://novabrasilfm.com.br".into()),
            tags: "mpb, bossa nova, brasil".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
            codec: Some("AAC".into()),
            votes: Some(5800),
            stationuuid: None,
        },
        RadioStation {
            name: "Rádio Batuta MPB (Brasil)".into(),
            url: "http://radioims.out.airtime.pro:8000/radioims_a".into(),
            homepage: Some("https://radiobatuta.ims.com.br".into()),
            tags: "mpb, samba, bossa nova, brasil".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
            codec: Some("MP3".into()),
            votes: Some(4200),
            stationuuid: None,
        },
    ]
}

pub fn custom_stations_path() -> Result<std::path::PathBuf> {
    Ok(crate::config::project_dirs()?
        .config_dir()
        .join("custom_radios.json"))
}

pub fn load_custom_stations() -> Vec<RadioStation> {
    let Ok(path) = custom_stations_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_custom_stations(stations: &[RadioStation]) -> Result<()> {
    let path = custom_stations_path()?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(stations)?;
    std::fs::write(&path, data)?;
    Ok(())
}

pub fn add_custom_station(station: RadioStation) -> Result<()> {
    let mut current = load_custom_stations();
    current.retain(|s| s.url != station.url);
    current.insert(0, station);
    save_custom_stations(&current)
}

pub fn all_stations() -> Vec<RadioStation> {
    let mut list = load_custom_stations();
    let mut curated = curated_stations();
    list.append(&mut curated);
    list
}

const SERVERS: &[&str] = &[
    "https://de1.api.radio-browser.info",
    "https://nl1.api.radio-browser.info",
    "https://at1.api.radio-browser.info",
];

#[derive(Deserialize)]
struct ApiStation {
    name: String,
    url_resolved: Option<String>,
    url: String,
    homepage: Option<String>,
    tags: Option<String>,
    country: Option<String>,
    bitrate: Option<u32>,
    codec: Option<String>,
    votes: Option<u32>,
    stationuuid: Option<String>,
}

fn query_radio_browser_path(path: &str) -> Result<Vec<RadioStation>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!(
            "Noctune/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/WendellOttoni/Noctune)"
        ))
        .build()?;

    let mut last_err = anyhow!("Nenhuma estação encontrada nos servidores do Radio-Browser");
    for server in SERVERS {
        let endpoint = format!("{server}{path}");
        match client.get(&endpoint).send() {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(stations) = resp.json::<Vec<ApiStation>>() {
                    let results: Vec<RadioStation> = stations
                        .into_iter()
                        .filter(|s| !s.name.trim().is_empty())
                        .map(|s| RadioStation {
                            name: s.name.trim().to_string(),
                            url: s.url_resolved.filter(|u| !u.is_empty()).unwrap_or(s.url),
                            homepage: s.homepage.filter(|h| !h.is_empty()),
                            tags: s.tags.unwrap_or_default(),
                            country: s.country.filter(|c| !c.is_empty()),
                            bitrate: s.bitrate.filter(|&b| b > 0),
                            codec: s.codec.filter(|c| !c.is_empty()),
                            votes: s.votes,
                            stationuuid: s.stationuuid.filter(|u| !u.is_empty()),
                        })
                        .collect();
                    if !results.is_empty() {
                        return Ok(results);
                    }
                }
            }
            Ok(resp) => {
                last_err = anyhow!("Radio-Browser retornou status HTTP {}", resp.status());
            }
            Err(e) => {
                last_err = anyhow!("Erro de conexão com Radio-Browser: {e}");
            }
        }
    }
    Err(last_err)
}

pub fn fetch_top_voted(limit: u32) -> Result<Vec<RadioStation>> {
    query_radio_browser_path(&format!("/json/stations/topvote/{limit}"))
}

pub fn fetch_by_tag(tag: &str, limit: u32) -> Result<Vec<RadioStation>> {
    let encoded = urlencoding_simple(tag);
    query_radio_browser_path(&format!(
        "/json/stations/bytag/{encoded}?limit={limit}&order=votes&reverse=true"
    ))
}

pub fn fetch_by_country(country: &str, limit: u32) -> Result<Vec<RadioStation>> {
    let encoded = urlencoding_simple(country);
    query_radio_browser_path(&format!(
        "/json/stations/bycountry/{encoded}?limit={limit}&order=votes&reverse=true"
    ))
}

/// Query the Radio-Browser public API for stations matching the query.
pub fn search_radio_browser(query: &str, limit: u32) -> Result<Vec<RadioStation>> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return fetch_top_voted(limit);
    }

    let encoded_query = urlencoding_simple(query_trimmed);
    let search_path = format!(
        "/json/stations/search?name={encoded_query}&limit={limit}&order=votes&reverse=true"
    );
    if let Ok(res) = query_radio_browser_path(&search_path) {
        if !res.is_empty() {
            return Ok(res);
        }
    }

    let tag_path =
        format!("/json/stations/bytag/{encoded_query}?limit={limit}&order=votes&reverse=true");
    query_radio_browser_path(&tag_path)
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}
