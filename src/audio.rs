use anyhow::{anyhow, Context, Result};
use rodio::{source::Source, Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::visualizer::{VizSource, VizTap};

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
}

impl Track {
    pub fn from_path(path: PathBuf) -> Self {
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self { path, title }
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

    pub fn play(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let decoder = Decoder::new(BufReader::new(file))
            .map_err(|e| anyhow!("decoding {}: {e}", path.display()))?;
        let source = VizSource::new(decoder.convert_samples::<f32>(), self.tap.clone());

        let sink = Sink::try_new(&self.handle)?;
        sink.set_volume(self.volume);
        sink.append(source);
        self.sink = sink;

        self.current = Some(Track::from_path(path.to_path_buf()));
        self.started_at = Some(Instant::now());
        self.paused_offset = Duration::ZERO;
        Ok(())
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
