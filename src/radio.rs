use anyhow::{anyhow, Result};
use std::time::Duration;

use crate::audio::Track;

/// Fetches a remote M3U/M3U8/PLS playlist and returns stream tracks.
pub fn fetch_playlist(url: &str) -> Result<Vec<Track>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0")
        .build()?;
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("{url} returned {}", resp.status()));
    }
    let body = resp.text()?;
    let lower = url.to_lowercase();
    let stream_urls = if lower.ends_with(".pls") {
        parse_pls(&body)
    } else {
        parse_m3u(&body)
    };
    Ok(stream_urls
        .into_iter()
        .map(|(title, stream_url)| {
            let mut t = Track::from_url(stream_url);
            if let Some(name) = title {
                t.title = name;
            }
            t
        })
        .collect())
}

fn parse_m3u(content: &str) -> Vec<(Option<String>, String)> {
    let mut result = Vec::new();
    let mut pending_title: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            // #EXTINF:duration,Title
            pending_title = rest
                .split_once(',')
                .map(|x| x.1)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        } else if line.starts_with("http://") || line.starts_with("https://") {
            result.push((pending_title.take(), line.to_string()));
        }
    }
    result
}

fn parse_pls(content: &str) -> Vec<(Option<String>, String)> {
    let mut files: std::collections::BTreeMap<u32, String> = std::collections::BTreeMap::new();
    let mut titles: std::collections::BTreeMap<u32, String> = std::collections::BTreeMap::new();

    for line in content.lines() {
        let line = line.trim();
        let (key, val) = match line.split_once('=') {
            Some(p) => p,
            None => continue,
        };
        let key_lower = key.trim().to_lowercase();
        let val = val.trim();

        if let Some(num_str) = key_lower.strip_prefix("file") {
            if let Ok(n) = num_str.parse::<u32>() {
                if val.starts_with("http://") || val.starts_with("https://") {
                    files.insert(n, val.to_string());
                }
            }
        } else if let Some(num_str) = key_lower.strip_prefix("title") {
            if let Ok(n) = num_str.parse::<u32>() {
                if !val.is_empty() {
                    titles.insert(n, val.to_string());
                }
            }
        }
    }

    files
        .into_iter()
        .map(|(n, url)| (titles.get(&n).cloned(), url))
        .collect()
}
