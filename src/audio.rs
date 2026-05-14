use anyhow::{anyhow, Context, Result};
use rodio::{source::Source, Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::{
    metadata::{probe, TrackMeta},
    visualizer::{VizSource, VizTap},
};

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,
    current: Option<Track>,
    volume: f32,
    started_at: Option<Instant>,
    paused_offset: Duration,
    tap: VizTap,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
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
            duration: None,
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
        t.duration = meta.duration;
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
    pub fn new(volume: f32) -> Result<Self> {
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
            tap: VizTap::new(),
        })
    }

    pub fn tap(&self) -> VizTap {
        self.tap.clone()
    }

    pub fn play(&mut self, track: &Track) -> Result<()> {
        self.play_from(track, Duration::ZERO)
    }

    pub fn play_from(&mut self, track: &Track, offset: Duration) -> Result<()> {
        let path = &track.path;
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| anyhow!("decoding {}: {e}", path.display()))?;
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
        let source = VizSource::new(samples, self.tap.clone());

        let sink = Sink::try_new(&self.handle)?;
        sink.set_volume(self.volume);
        sink.append(source);
        self.sink = sink;

        self.current = Some(track.clone());
        self.started_at = Some(Instant::now());
        self.paused_offset = offset;
        Ok(())
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
        self.sink.stop();
        self.current = None;
        self.started_at = None;
        self.paused_offset = Duration::ZERO;
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
        self.sink.set_volume(self.volume);
    }

    pub fn current(&self) -> Option<&Track> {
        self.current.as_ref()
    }
}
