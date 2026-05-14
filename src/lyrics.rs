use std::{fs, path::Path, time::Duration};

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub at: Duration,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    pub fn for_track(audio_path: &Path) -> Option<Self> {
        let lrc_path = audio_path.with_extension("lrc");
        let text = fs::read_to_string(&lrc_path).ok()?;
        let mut lines: Vec<LyricLine> = Vec::new();

        for raw in text.lines() {
            let line = raw.trim();
            if !line.starts_with('[') {
                continue;
            }

            let mut content = line;
            let mut stamps: Vec<Duration> = Vec::new();
            while let Some(end) = content.find(']') {
                let tag = &content[1..end];
                if let Some(d) = parse_stamp(tag) {
                    stamps.push(d);
                }
                content = &content[end + 1..];
                if !content.starts_with('[') {
                    break;
                }
            }

            let body = content.trim().to_string();
            for d in stamps {
                lines.push(LyricLine {
                    at: d,
                    text: body.clone(),
                });
            }
        }
        lines.sort_by_key(|l| l.at);
        if lines.is_empty() {
            None
        } else {
            Some(Self { lines })
        }
    }

    pub fn current_index(&self, elapsed: Duration) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let mut idx = None;
        for (i, l) in self.lines.iter().enumerate() {
            if l.at <= elapsed {
                idx = Some(i);
            } else {
                break;
            }
        }
        idx
    }
}

fn parse_stamp(s: &str) -> Option<Duration> {
    let mut parts = s.split(':');
    let mm: u64 = parts.next()?.parse().ok()?;
    let rest = parts.next()?;
    let mut sec_split = rest.split('.');
    let ss: u64 = sec_split.next()?.parse().ok()?;
    let frac_ms = match sec_split.next() {
        Some(f) => {
            let mut n: u64 = f.parse().ok()?;
            match f.len() {
                1 => n *= 100,
                2 => n *= 10,
                _ => {}
            }
            n
        }
        None => 0,
    };
    Some(Duration::from_millis(mm * 60_000 + ss * 1_000 + frac_ms))
}
