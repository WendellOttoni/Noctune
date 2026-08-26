//! Online Radio Directory & Curated Station Hub.
//! Connects to the community-driven Radio-Browser API (+40,000 global stations)
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
            url: "https://play.streamafrica.net/lofigirl".into(),
            homepage: Some("https://lofigirl.com".into()),
            tags: "lofi, chill, instrumental, study".into(),
            country: Some("France".into()),
            bitrate: Some(192),
        },
        RadioStation {
            name: "Nightwave Plaza (Vaporwave)".into(),
            url: "https://radio.plaza.one/mp3".into(),
            homepage: Some("https://plaza.one".into()),
            tags: "vaporwave, future funk, chill".into(),
            country: Some("Worldwide".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "SomaFM: Groove Salad".into(),
            url: "https://ice6.somafm.com/groovesalad-256-mp3".into(),
            homepage: Some("https://somafm.com/groovesalad/".into()),
            tags: "ambient, downtempo, chill".into(),
            country: Some("United States".into()),
            bitrate: Some(256),
        },
        RadioStation {
            name: "Chillhop Radio".into(),
            url: "http://stream.zeno.fm/f3wvbbqmdg8uv".into(),
            homepage: Some("https://chillhop.com".into()),
            tags: "chillhop, lofi, beats".into(),
            country: Some("Netherlands".into()),
            bitrate: Some(128),
        },
        // Synthwave & Cyberpunk
        RadioStation {
            name: "Nightride FM (Synthwave / Retrowave)".into(),
            url: "https://stream.nightride.fm/nightride.m4a".into(),
            homepage: Some("https://nightride.fm".into()),
            tags: "synthwave, retrowave, cyberpunk, 80s".into(),
            country: Some("United States".into()),
            bitrate: Some(256),
        },
        RadioStation {
            name: "SomaFM: DEF CON Radio".into(),
            url: "https://ice6.somafm.com/defcon-256-mp3".into(),
            homepage: Some("https://somafm.com/defcon/".into()),
            tags: "hacker, electronic, ambient, darkwave".into(),
            country: Some("United States".into()),
            bitrate: Some(256),
        },
        // Jazz & Classical
        RadioStation {
            name: "SomaFM: Secret Agent".into(),
            url: "https://ice6.somafm.com/secretagent-128-mp3".into(),
            homepage: Some("https://somafm.com/secretagent/".into()),
            tags: "spy jazz, lounge, surf, 60s".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "Radio Swiss Jazz".into(),
            url: "http://stream.srg-ssr.ch/m/rsj/mp3_128".into(),
            homepage: Some("https://www.radioswissjazz.ch".into()),
            tags: "jazz, swing, bebop, blues".into(),
            country: Some("Switzerland".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "Radio Swiss Classic".into(),
            url: "http://stream.srg-ssr.ch/m/rsc_de/mp3_128".into(),
            homepage: Some("https://www.radioswissclassic.ch".into()),
            tags: "classical, orchestral, baroque".into(),
            country: Some("Switzerland".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "BBC Radio 3 (Classical)".into(),
            url: "http://stream.live.vc.bbcmedia.co.uk/bbc_radio_three".into(),
            homepage: Some("https://www.bbc.co.uk/radio3".into()),
            tags: "classical, opera, culture".into(),
            country: Some("United Kingdom".into()),
            bitrate: Some(128),
        },
        // Rock, Metal & Pop
        RadioStation {
            name: "SomaFM: Indie Pop Rocks!".into(),
            url: "https://ice6.somafm.com/indiepop-128-mp3".into(),
            homepage: Some("https://somafm.com/indiepop/".into()),
            tags: "indie rock, pop, alternative".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "SomaFM: Metal Detector".into(),
            url: "https://ice6.somafm.com/metal-128-mp3".into(),
            homepage: Some("https://somafm.com/metal/".into()),
            tags: "metal, hard rock, heavy".into(),
            country: Some("United States".into()),
            bitrate: Some(128),
        },
        // Brazil Stations
        RadioStation {
            name: "Antena 1 (Brasil)".into(),
            url: "https://stream.antena1.com.br/stream".into(),
            homepage: Some("https://www.antena1.com.br".into()),
            tags: "pop, adult contemporary, international".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "Rádio Eldorado FM (Brasil)".into(),
            url: "https://ice.fabricahost.com.br/eldoradofmsp".into(),
            homepage: Some("https://www.eldorado.com.br".into()),
            tags: "mpb, rock, news, culture".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
        },
        RadioStation {
            name: "NovaBrasil FM (Brasil)".into(),
            url: "https://painel.fabricahost.com.br:8000/stream/novabrasilsp".into(),
            homepage: Some("https://novabrasilfm.com.br".into()),
            tags: "mpb, bossa nova, brasil".into(),
            country: Some("Brazil".into()),
            bitrate: Some(128),
        },
    ]
}

/// Query the Radio-Browser public API for stations matching the query.
pub fn search_radio_browser(query: &str, limit: u32) -> Result<Vec<RadioStation>> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Noctune/0.4.2 (https://github.com/WendellOttoni/Noctune)")
        .build()?;

    let encoded_query = urlencoding_simple(query_trimmed);
    let endpoints = [
        format!("https://de1.api.radio-browser.info/json/stations/search?name={encoded_query}&limit={limit}&order=votes&reverse=true"),
        format!("https://at1.api.radio-browser.info/json/stations/search?name={encoded_query}&limit={limit}&order=votes&reverse=true"),
        format!("https://nl1.api.radio-browser.info/json/stations/bytag/{encoded_query}?limit={limit}&order=votes&reverse=true"),
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
    }

    let mut last_err = anyhow!("no radio stations found");
    for endpoint in endpoints {
        match client.get(&endpoint).send() {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(stations) = resp.json::<Vec<ApiStation>>() {
                    let results: Vec<RadioStation> = stations
                        .into_iter()
                        .filter(|s| !s.name.trim().is_empty())
                        .map(|s| RadioStation {
                            name: s.name.trim().to_string(),
                            url: s.url_resolved.unwrap_or(s.url),
                            homepage: s.homepage.filter(|h| !h.is_empty()),
                            tags: s.tags.unwrap_or_default(),
                            country: s.country.filter(|c| !c.is_empty()),
                            bitrate: s.bitrate.filter(|&b| b > 0),
                        })
                        .collect();
                    if !results.is_empty() {
                        return Ok(results);
                    }
                }
            }
            Ok(resp) => {
                last_err = anyhow!("Radio-Browser API returned {}", resp.status());
            }
            Err(e) => {
                last_err = anyhow!("Radio-Browser connection error: {e}");
            }
        }
    }

    Err(last_err)
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
