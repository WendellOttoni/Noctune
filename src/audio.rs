use anyhow::{anyhow, Context, Result};
use rodio::{source::Source, Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::{
    eq::{EqHandle, EqSource},
    metadata::{probe, TrackMeta},
    visualizer::{VizSource, VizTap},
};

pub enum CrossfadeStatus {
    None,
    InProgress,
    Complete,
}

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,
    current: Option<Track>,
    volume: f32,
    started_at: Option<Instant>,
    paused_offset: Duration,
    tap: VizTap,
    eq: EqHandle,
    pub rg_scale: f32,
    // crossfade state
    fade_sink: Option<Sink>,
    fade_current: Option<Track>,
    fade_started_at: Option<Instant>,
    crossfade_start: Option<Instant>,
    pub crossfade_secs: f32,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub duration: Option<Duration>,
    pub replaygain_track_db: Option<f32>,
    pub replaygain_album_db: Option<f32>,
}

impl Track {
    pub fn from_path(path: PathBuf) -> Self {
        let fallback_title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self {
            path,
            title: fallback_title,
            artist: None,
            album: None,
            genre: None,
            year: None,
            duration: None,
            replaygain_track_db: None,
            replaygain_album_db: None,
        }
    }

    pub fn from_path_with_meta(path: PathBuf) -> Self {
        let mut t = Self::from_path(path.clone());
        let meta: TrackMeta = probe(&path);
        if let Some(title) = meta.title {
            t.title = title;
        }
        t.artist = meta.artist;
        t.album = meta.album;
        t.genre = meta.genre;
        t.year = meta.year;
        t.duration = meta.duration;
        t.replaygain_track_db = meta.replaygain_track_db;
        t.replaygain_album_db = meta.replaygain_album_db;
        t
    }

    pub fn display(&self) -> String {
        match &self.artist {
            Some(a) => format!("{a} — {}", self.title),
            None => self.title.clone(),
        }
    }
}

impl Player {
    pub fn new(volume: f32, viz_sensitivity: f32) -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()
            .context("could not open default audio output")?;
        let sink = Sink::try_new(&handle)?;
        sink.set_volume(volume);
        Ok(Self {
            _stream: stream,
            handle,
            sink,
            current: None,
            volume,
            started_at: None,
            paused_offset: Duration::ZERO,
            tap: VizTap::new(viz_sensitivity),
            eq: EqHandle::new(),
            rg_scale: 1.0,
            fade_sink: None,
            fade_current: None,
            fade_started_at: None,
            crossfade_start: None,
            crossfade_secs: 3.0,
        })
    }

    pub fn tap(&self) -> VizTap {
        self.tap.clone()
    }

    pub fn eq(&self) -> EqHandle {
        self.eq.clone()
    }

    pub fn play(&mut self, track: &Track) -> Result<()> {
        self.play_from(track, Duration::ZERO)
    }

    fn cancel_crossfade(&mut self) {
        self.fade_sink = None;
        self.fade_current = None;
        self.fade_started_at = None;
        self.crossfade_start = None;
    }

    pub fn play_from(&mut self, track: &Track, offset: Duration) -> Result<()> {
        self.cancel_crossfade();
        let path_str = track.path.to_string_lossy().to_string();
        let is_url = path_str.starts_with("http://") || path_str.starts_with("https://");

        if is_url {
            let bytes = if crate::ytdlp::is_youtube_url(&path_str) {
                crate::ytdlp::download_audio_bytes(&path_str)?
            } else {
                http_get_bytes(&path_str)?
            };
            let decoder = Decoder::new(Cursor::new(bytes))
                .map_err(|e| anyhow!("decoding {path_str}: {e}"))?;
            let samples = decoder.convert_samples::<f32>();
            let viz = VizSource::new(samples, self.tap.clone());
            let eq = EqSource::new(viz, self.eq.clone());
            let sink = Sink::try_new(&self.handle)?;
            sink.set_volume(self.volume * self.rg_scale);
            sink.append(eq);
            self.sink = sink;
        } else {
            let file = File::open(&track.path)
                .with_context(|| format!("opening {}", track.path.display()))?;
            let decoder = Decoder::new(BufReader::new(file))
                .map_err(|e| anyhow!("decoding {}: {e}", track.path.display()))?;
            let mut samples = decoder.convert_samples::<f32>();
            if offset > Duration::ZERO {
                let rate = samples.sample_rate() as u64;
                let ch = samples.channels() as u64;
                let to_skip = (offset.as_millis() as u64 * rate / 1000) * ch;
                for _ in 0..to_skip {
                    if samples.next().is_none() {
                        break;
                    }
                }
            }
            let viz = VizSource::new(samples, self.tap.clone());
            let eq = EqSource::new(viz, self.eq.clone());
            let sink = Sink::try_new(&self.handle)?;
            sink.set_volume(self.volume * self.rg_scale);
            sink.append(eq);
            self.sink = sink;
        }

        self.current = Some(track.clone());
        self.started_at = Some(Instant::now());
        self.paused_offset = offset;
        Ok(())
    }

    pub fn seek_absolute_fraction(&mut self, fraction: f32) -> Result<()> {
        let Some(track) = self.current.clone() else { return Ok(()); };
        let total = track
            .duration
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        if total <= 0 {
            return Ok(());
        }
        let target = (total as f32 * fraction.clamp(0.0, 1.0)) as i64;
        self.play_from(&track, Duration::from_millis(target as u64))
    }

    pub fn seek_relative(&mut self, delta: i64) -> Result<()> {
        let Some(track) = self.current.clone() else { return Ok(()); };
        let cur_ms = self.elapsed().as_millis() as i64;
        let mut new_ms = cur_ms + delta * 1000;
        if new_ms < 0 {
            new_ms = 0;
        }
        if let Some(total) = track.duration {
            let max_ms = total.as_millis().saturating_sub(500) as i64;
            if new_ms > max_ms {
                new_ms = max_ms;
            }
        }
        self.play_from(&track, Duration::from_millis(new_ms as u64))
    }

    pub fn toggle(&mut self) {
        if self.sink.is_paused() {
            self.sink.play();
            self.started_at = Some(Instant::now());
        } else {
            if let Some(start) = self.started_at.take() {
                self.paused_offset += start.elapsed();
            }
            self.sink.pause();
        }
    }

    pub fn stop(&mut self) {
        self.cancel_crossfade();
        self.sink.stop();
        self.current = None;
        self.started_at = None;
        self.paused_offset = Duration::ZERO;
    }

    pub fn remaining(&self) -> Option<Duration> {
        let total = self.current.as_ref()?.duration?;
        Some(total.saturating_sub(self.elapsed()))
    }

    pub fn is_crossfading(&self) -> bool {
        self.crossfade_start.is_some()
    }

    pub fn begin_crossfade(&mut self, track: &Track) -> Result<()> {
        let path_str = track.path.to_string_lossy();
        if path_str.starts_with("http://") || path_str.starts_with("https://") {
            return Ok(());
        }

        let file = File::open(&track.path)
            .with_context(|| format!("opening {}", track.path.display()))?;
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| anyhow!("decoding {}: {e}", track.path.display()))?;
        let samples = decoder.convert_samples::<f32>();
        let viz = VizSource::new(samples, self.tap.clone());
        let eq = EqSource::new(viz, self.eq.clone());

        let new_sink = Sink::try_new(&self.handle)?;
        new_sink.set_volume(0.0);
        new_sink.append(eq);

        self.fade_sink = Some(new_sink);
        self.fade_current = Some(track.clone());
        self.fade_started_at = Some(Instant::now());
        self.crossfade_start = Some(Instant::now());
        Ok(())
    }

    pub fn update_crossfade(&mut self) -> CrossfadeStatus {
        let Some(start) = self.crossfade_start else {
            return CrossfadeStatus::None;
        };
        let progress = (start.elapsed().as_secs_f32() / self.crossfade_secs).clamp(0.0, 1.0);

        self.sink.set_volume(self.volume * self.rg_scale * (1.0 - progress));
        if let Some(sink) = &self.fade_sink {
            sink.set_volume(self.volume * self.rg_scale * progress);
        }

        if progress >= 1.0 || self.sink.empty() {
            if let (Some(new_sink), Some(new_track)) =
                (self.fade_sink.take(), self.fade_current.take())
            {
                new_sink.set_volume(self.volume * self.rg_scale);
                let played = self.fade_started_at.map(|t| t.elapsed()).unwrap_or_default();
                self.sink = new_sink;
                self.current = Some(new_track);
                self.paused_offset = played;
                self.started_at = Some(Instant::now());
            }
            self.fade_started_at = None;
            self.crossfade_start = None;
            CrossfadeStatus::Complete
        } else {
            CrossfadeStatus::InProgress
        }
    }

    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    pub fn is_empty(&self) -> bool {
        self.sink.empty()
    }

    pub fn elapsed(&self) -> Duration {
        let running = self.started_at.map(|t| t.elapsed()).unwrap_or_default();
        self.paused_offset + running
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.5);
        self.sink.set_volume(self.volume * self.rg_scale);
    }

    pub fn current(&self) -> Option<&Track> {
        self.current.as_ref()
    }
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("{url} returned {}", resp.status()));
    }
    let bytes = resp.bytes()?.to_vec();
    Ok(bytes)
}

impl Track {
    pub fn from_url(url: String) -> Self {
        Self {
            path: PathBuf::from(&url),
            title: format!("{} (stream)", url),
            artist: None,
            album: None,
            genre: None,
            year: None,
            duration: None,
            replaygain_track_db: None,
            replaygain_album_db: None,
        }
    }
}
