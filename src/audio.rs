use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{source::Source, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    process::Child,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymphoniaError,
    formats::{FormatOptions, FormatReader},
    io::{MediaSourceStream, ReadOnlySource},
    meta::MetadataOptions,
    probe::Hint,
};

use crate::{
    compressor::{CompHandle, CompSource},
    eq::{EqHandle, EqSource},
    metadata::{probe, TrackMeta},
    visualizer::{VizSource, VizTap},
};

// Wrapper to satisfy symphonia's `Read + Send + Sync` bound for non-Sync readers.
// Safety: SymphoniaSource is !Sync, so the inner reader is accessed from one thread only.
struct SyncWrap<R>(R);
impl<R: Read> Read for SyncWrap<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}
unsafe impl<R: Send> Send for SyncWrap<R> {}
unsafe impl<R: Send> Sync for SyncWrap<R> {}

pub struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    buf: Vec<f32>,
    buf_pos: usize,
    sample_buf: Option<SampleBuffer<f32>>,
    // Holds the yt-dlp child process so it's killed when the source is dropped.
    _child: Option<Child>,
}

// Safety: FormatReader/Decoder internals are not Sync, but SymphoniaSource is !Sync
// and is only moved to rodio's audio thread — never shared across threads.
unsafe impl Send for SymphoniaSource {}

impl SymphoniaSource {
    fn from_mss(mss: MediaSourceStream, hint: Hint, child: Option<Child>) -> Result<Self> {
        let fmt_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &MetadataOptions::default())
            .map_err(|e| anyhow!("audio probe failed: {e}"))?;

        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow!("no audio track found"))?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| anyhow!("codec init: {e}"))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            buf: Vec::new(),
            buf_pos: 0,
            sample_buf: None,
            _child: child,
        })
    }

    pub fn from_file(file: File, hint: Hint) -> Result<Self> {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        Self::from_mss(mss, hint, None)
    }

    pub fn from_bytes(bytes: Vec<u8>, hint: Hint) -> Result<Self> {
        // Cursor<Vec<u8>> implements MediaSource with is_seekable=true, which allows
        // format readers that require seeking (e.g. MP4/M4A moov atom lookup).
        let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
        Self::from_mss(mss, hint, None)
    }

    pub fn from_reader<R: Read + Send + 'static>(reader: R, hint: Hint) -> Result<Self> {
        let mss = MediaSourceStream::new(
            Box::new(ReadOnlySource::new(SyncWrap(reader))),
            Default::default(),
        );
        Self::from_mss(mss, hint, None)
    }

    pub fn from_child(
        mut child: Child,
        hint: Hint,
        stream_err: Arc<Mutex<Option<String>>>,
    ) -> Result<Self> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("yt-dlp stdout not piped"))?;
        // Drain stderr in a background thread; write the last non-empty line to the error slot
        // so the app can surface it in the status bar.
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let mut last = String::new();
                for line in BufReader::new(stderr).lines().flatten() {
                    if !line.trim().is_empty() {
                        last = line;
                    }
                }
                if !last.is_empty() {
                    *stream_err.lock().unwrap() = Some(format!("yt-dlp: {last}"));
                }
            });
        }
        let mss = MediaSourceStream::new(
            Box::new(ReadOnlySource::new(SyncWrap(stdout))),
            Default::default(),
        );
        Self::from_mss(mss, hint, Some(child))
    }

    /// Seek the underlying container to `time`. Cheap on indexed formats (mp3/flac/ogg);
    /// returns Err for unseekable sources (e.g. piped yt-dlp stdout, HTTP streams without
    /// range support). Issue #59.
    pub fn seek_to(&mut self, time: Duration) -> Result<()> {
        use symphonia::core::formats::{SeekMode, SeekTo};
        use symphonia::core::units::Time;
        let secs = time.as_secs();
        let frac = f64::from(time.subsec_nanos()) / 1_000_000_000.0;
        self.format
            .seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time {
                        seconds: secs,
                        frac,
                    },
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| anyhow!("seek: {e}"))?;
        self.decoder.reset();
        self.buf.clear();
        self.buf_pos = 0;
        Ok(())
    }

    fn fill_buf(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return false;
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(_) => return false,
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let cap = decoded.capacity() as u64;
                    let sbuf = match &mut self.sample_buf {
                        Some(sb) if (sb.capacity() as u64) >= cap => sb,
                        _ => {
                            self.sample_buf = Some(SampleBuffer::<f32>::new(cap, spec));
                            self.sample_buf.as_mut().unwrap()
                        }
                    };
                    sbuf.copy_interleaved_ref(decoded);
                    self.buf.clear();
                    self.buf.extend_from_slice(sbuf.samples());
                    self.buf_pos = 0;
                    return !self.buf.is_empty();
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => return false,
            }
        }
    }
}

impl Drop for SymphoniaSource {
    fn drop(&mut self) {
        if let Some(mut child) = self._child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.buf_pos >= self.buf.len() && !self.fill_buf() {
            return None;
        }
        let s = self.buf[self.buf_pos];
        self.buf_pos += 1;
        Some(s)
    }
}

impl Source for SymphoniaSource {
    fn current_frame_len(&self) -> Option<usize> {
        let rem = self.buf.len().saturating_sub(self.buf_pos);
        if rem > 0 {
            Some(rem)
        } else {
            None
        }
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

fn hint_from_path(path: &Path) -> Hint {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    hint
}

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
    comp: CompHandle,
    pub rg_scale: f32,
    pub speed: f32,
    // crossfade state
    fade_sink: Option<Sink>,
    fade_current: Option<Track>,
    fade_started_at: Option<Instant>,
    crossfade_start: Option<Instant>,
    pub crossfade_secs: f32,
    // Streams cannot synchronously open during the few seconds before the song ends,
    // so the source is built in a worker thread and attached when ready (#71).
    crossfade_load_rx: Option<std::sync::mpsc::Receiver<Result<SymphoniaSource, String>>>,
    crossfade_pending: Option<Track>,
    // gapless state
    pub gapless_queued: Option<Track>,
    // last error from a streaming source (yt-dlp stderr)
    stream_err: Arc<Mutex<Option<String>>>,
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
    /// HTTP(S) URL of an album-art thumbnail (#105). Populated for streaming
    /// tracks (e.g. YouTube) where the audio file does not carry embedded art.
    /// Resolved asynchronously by `App` after the track starts; `None` for
    /// local files (cover comes from in-file tags via `metadata::probe_picture`).
    pub cover_url: Option<String>,
    /// Unix timestamp of the file's modification time, captured at scan time
    /// (#87). Persisted in `CacheEntry`. Used by the Smart view's "Recently
    /// Added" category to avoid a per-frame `fs::metadata` call. `None` for
    /// streaming tracks where no local file exists.
    pub added_at: Option<u64>,
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
            cover_url: None,
            added_at: None,
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
        let (stream, handle) =
            OutputStream::try_default().context("could not open default audio output")?;
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
            comp: CompHandle::new(),
            rg_scale: 1.0,
            speed: 1.0,
            fade_sink: None,
            fade_current: None,
            fade_started_at: None,
            crossfade_start: None,
            crossfade_secs: 3.0,
            crossfade_load_rx: None,
            crossfade_pending: None,
            gapless_queued: None,
            stream_err: Arc::new(Mutex::new(None)),
        })
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.5, 2.5);
        self.sink.set_speed(self.speed);
        if let Some(f_sink) = &self.fade_sink {
            f_sink.set_speed(self.speed);
        }
    }

    pub fn tap(&self) -> VizTap {
        self.tap.clone()
    }

    pub fn eq(&self) -> EqHandle {
        self.eq.clone()
    }

    pub fn comp(&self) -> CompHandle {
        self.comp.clone()
    }

    /// Returns and clears the last error message from a streaming source, if any.
    pub fn take_stream_error(&self) -> Option<String> {
        self.stream_err.lock().unwrap().take()
    }

    pub fn play(&mut self, track: &Track) -> Result<()> {
        self.play_from(track, Duration::ZERO)
    }

    fn cancel_crossfade(&mut self) {
        self.fade_sink = None;
        self.fade_current = None;
        self.fade_started_at = None;
        self.crossfade_start = None;
        self.crossfade_load_rx = None;
        self.crossfade_pending = None;
    }

    pub fn is_crossfading(&self) -> bool {
        self.crossfade_start.is_some() || self.crossfade_load_rx.is_some()
    }

    /// Appends the next track to the sink queue for gapless playback.
    /// Only works for local files; URLs are skipped.
    pub fn enqueue_next(&mut self, track: &Track) -> Result<()> {
        let path_str = track.path.to_string_lossy();
        if path_str.starts_with("http://") || path_str.starts_with("https://") {
            return Ok(());
        }
        let file =
            File::open(&track.path).with_context(|| format!("opening {}", track.path.display()))?;
        let source = SymphoniaSource::from_file(file, hint_from_path(&track.path))
            .map_err(|e| anyhow!("decoding {}: {e}", track.path.display()))?;
        let viz = VizSource::new(source, self.tap.clone());
        let eq = EqSource::new(viz, self.eq.clone());
        let comp = CompSource::new(eq, self.comp.clone());
        self.sink.append(comp);
        self.gapless_queued = Some(track.clone());
        Ok(())
    }

    pub fn sink_queue_len(&self) -> usize {
        self.sink.len()
    }

    pub fn play_from(&mut self, track: &Track, offset: Duration) -> Result<()> {
        let source = build_source(track, offset, self.stream_err.clone())?;
        self.play_prepared(source, track, offset)
    }

    /// Returns a clone of the streaming-error slot so background loaders can write to it.
    pub fn stream_err_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.stream_err.clone()
    }

    /// Attach a pre-built `SymphoniaSource` to a fresh sink and start playback.
    /// All blocking work (file open, symphonia probe, yt-dlp spawn) must have already
    /// happened in `build_source` — this method only does the cheap sink swap, so it is
    /// safe to call from the UI thread when a background loader finishes (issue #58).
    pub fn play_prepared(
        &mut self,
        source: SymphoniaSource,
        track: &Track,
        offset: Duration,
    ) -> Result<()> {
        self.cancel_crossfade();
        self.gapless_queued = None;
        let viz = VizSource::new(source, self.tap.clone());
        let eq = EqSource::new(viz, self.eq.clone());
        let comp = CompSource::new(eq, self.comp.clone());
        let sink = Sink::try_new(&self.handle)?;
        sink.set_volume(self.volume * self.rg_scale);
        sink.set_speed(self.speed);
        sink.append(comp);
        self.sink = sink;
        self.current = Some(track.clone());
        self.started_at = Some(Instant::now());
        self.paused_offset = offset;
        Ok(())
    }

    pub fn seek_absolute_fraction(&mut self, fraction: f32) -> Result<()> {
        let Some(track) = self.current.clone() else {
            return Ok(());
        };
        let total = track.duration.map(|d| d.as_millis() as i64).unwrap_or(0);
        if total <= 0 {
            return Ok(());
        }
        let target = (total as f32 * fraction.clamp(0.0, 1.0)) as i64;
        self.play_from(&track, Duration::from_millis(target as u64))
    }

    pub fn seek_relative(&mut self, delta: i64) -> Result<()> {
        let Some(track) = self.current.clone() else {
            return Ok(());
        };
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
        self.gapless_queued = None;
        self.sink.stop();
        self.current = None;
        self.started_at = None;
        self.paused_offset = Duration::ZERO;
    }

    pub fn remaining(&self) -> Option<Duration> {
        let total = self.current.as_ref()?.duration?;
        Some(total.saturating_sub(self.elapsed()))
    }

    pub fn begin_crossfade(&mut self, track: &Track) -> Result<()> {
        let path_str = track.path.to_string_lossy();
        let is_url = path_str.starts_with("http://") || path_str.starts_with("https://");
        if is_url {
            // #71: build the stream source on a worker thread; the fade itself starts
            // once the source is ready (update_crossfade polls the receiver). If the
            // load takes longer than the remaining time of the current track, the
            // fade simply degrades to a hard cut handled by the normal next() flow.
            if self.crossfade_load_rx.is_some() {
                return Ok(()); // already preparing
            }
            let stream_err = self.stream_err.clone();
            let track_owned = track.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let res = build_source(&track_owned, Duration::ZERO, stream_err)
                    .map_err(|e| e.to_string());
                let _ = tx.send(res);
            });
            self.crossfade_load_rx = Some(rx);
            self.crossfade_pending = Some(track.clone());
            return Ok(());
        }

        let file =
            File::open(&track.path).with_context(|| format!("opening {}", track.path.display()))?;
        let source = SymphoniaSource::from_file(file, hint_from_path(&track.path))
            .map_err(|e| anyhow!("decoding {}: {e}", track.path.display()))?;
        self.attach_fade_source(source, track.clone())
    }

    fn attach_fade_source(&mut self, source: SymphoniaSource, track: Track) -> Result<()> {
        let viz = VizSource::new(source, self.tap.clone());
        let eq = EqSource::new(viz, self.eq.clone());
        let comp = CompSource::new(eq, self.comp.clone());
        let new_sink = Sink::try_new(&self.handle)?;
        new_sink.set_volume(0.0);
        new_sink.append(comp);
        self.fade_sink = Some(new_sink);
        self.fade_current = Some(track);
        self.fade_started_at = Some(Instant::now());
        self.crossfade_start = Some(Instant::now());
        Ok(())
    }

    pub fn update_crossfade(&mut self) -> CrossfadeStatus {
        // #71: if we're waiting on a background stream load, check it before doing
        // anything else. When the source arrives, attach it and the fade timer starts.
        if let Some(rx) = &self.crossfade_load_rx {
            match rx.try_recv() {
                Ok(Ok(source)) => {
                    self.crossfade_load_rx = None;
                    if let Some(track) = self.crossfade_pending.take() {
                        if self.attach_fade_source(source, track).is_err() {
                            self.cancel_crossfade();
                            return CrossfadeStatus::None;
                        }
                    }
                }
                Ok(Err(_)) => {
                    // Source build failed — drop the pending fade and let the normal
                    // queue-advance flow take over via a hard cut.
                    self.cancel_crossfade();
                    return CrossfadeStatus::None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return CrossfadeStatus::InProgress,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.cancel_crossfade();
                    return CrossfadeStatus::None;
                }
            }
        }

        let Some(start) = self.crossfade_start else {
            return CrossfadeStatus::None;
        };
        let progress = (start.elapsed().as_secs_f32() / self.crossfade_secs).clamp(0.0, 1.0);

        self.sink
            .set_volume(self.volume * self.rg_scale * (1.0 - progress));
        if let Some(sink) = &self.fade_sink {
            sink.set_volume(self.volume * self.rg_scale * progress);
        }

        if progress >= 1.0 || self.sink.empty() {
            if let (Some(new_sink), Some(new_track)) =
                (self.fade_sink.take(), self.fade_current.take())
            {
                new_sink.set_volume(self.volume * self.rg_scale);
                let played = self
                    .fade_started_at
                    .map(|t| t.elapsed())
                    .unwrap_or_default();
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

    pub fn switch_device(&mut self, device_name: &str) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .output_devices()?
            .find(|d| d.name().as_deref().ok() == Some(device_name))
            .ok_or_else(|| anyhow!("device not found: {device_name}"))?;

        let cur = self.current.clone();
        let offset = self.elapsed();
        let was_paused = self.sink.is_paused();

        self.stop();

        let (stream, handle) = OutputStream::try_from_device(&device)
            .context("could not open selected audio device")?;
        let sink = Sink::try_new(&handle)?;
        sink.set_volume(self.volume * self.rg_scale);

        self._stream = stream;
        self.handle = handle;
        self.sink = sink;

        if let Some(track) = cur {
            self.play_from(&track, offset)?;
            if was_paused {
                self.sink.pause();
            }
        }

        Ok(())
    }
}

pub fn enumerate_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

pub fn default_device_name() -> Option<String> {
    cpal::default_host().default_output_device()?.name().ok()
}

/// Build a ready-to-play `SymphoniaSource` for a track. Performs all blocking work
/// (file open, symphonia probe, yt-dlp spawn, HTTP connect) — call from a background
/// thread when responsiveness matters (issue #58).
pub fn build_source(
    track: &Track,
    offset: Duration,
    stream_err: Arc<Mutex<Option<String>>>,
) -> Result<SymphoniaSource> {
    let path_str = track.path.to_string_lossy().to_string();
    let is_url = path_str.starts_with("http://") || path_str.starts_with("https://");

    let mut source = if is_url {
        if crate::ytdlp::is_youtube_url(&path_str) {
            // Issue #57: pass the seek offset down to yt-dlp/ffmpeg so the stream
            // starts at the requested position instead of from zero.
            crate::ytdlp::spawn_yt_dlp_at(&path_str, offset, stream_err)?
        } else {
            // Try direct HTTP stream first with proper headers and MIME hints
            match open_http_stream(&path_str) {
                Ok(src) => src,
                Err(e) => {
                    tracing::warn!(target: "audio", "direct stream failed ({e}), falling back to yt-dlp/ffmpeg");
                    // Fallback to yt-dlp/ffmpeg for HLS / complex containers
                    crate::ytdlp::spawn_yt_dlp_at(&path_str, offset, stream_err)
                        .map_err(|e2| anyhow!("stream playback failed: direct: {e} | fallback: {e2}"))?
                }
            }
        }
    } else {
        let file =
            File::open(&track.path).with_context(|| format!("opening {}", track.path.display()))?;
        SymphoniaSource::from_file(file, hint_from_path(&track.path))
            .map_err(|e| anyhow!("decoding {}: {e}", track.path.display()))?
    };

    if offset > Duration::ZERO && !is_url {
        // Symphonia's container-level seek is O(log n) on indexed formats; falls back
        // to scanning otherwise. Either way it avoids the per-sample loop used before
        // (issue #59). Streams (yt-dlp/HTTP) handle seek by respawning with a start
        // offset — handled at the caller layer (#57).
        if source.seek_to(offset).is_err() {
            // Fallback: skip frames the slow way if container seek is unsupported.
            let rate = source.sample_rate() as u64;
            let ch = source.channels() as u64;
            let to_skip = (offset.as_millis() as u64 * rate / 1000) * ch;
            for _ in 0..to_skip {
                if source.next().is_none() {
                    break;
                }
            }
        }
    }
    Ok(source)
}

fn open_http_stream(url: &str) -> Result<SymphoniaSource> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Noctune/0.4.3")
        .build()?;

    let resp = client
        .get(url)
        .header("Icy-MetaData", "0")
        .header("Accept", "*/*")
        .send()
        .with_context(|| format!("GET {url}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("{url} returned HTTP {}", resp.status()));
    }

    let mut hint = Hint::new();
    if let Some(ct) = resp.headers().get("content-type").and_then(|v| v.to_str().ok()) {
        let mime = ct.split(';').next().unwrap_or(ct).trim();
        if mime == "audio/mpeg" || mime == "audio/mp3" {
            hint.with_extension("mp3");
            hint.mime_type("audio/mpeg");
        } else if mime == "audio/aac" || mime == "audio/aacp" {
            hint.with_extension("aac");
            hint.mime_type("audio/aac");
        } else if mime == "audio/ogg" || mime == "application/ogg" {
            hint.with_extension("ogg");
            hint.mime_type("audio/ogg");
        } else if mime == "audio/flac" {
            hint.with_extension("flac");
            hint.mime_type("audio/flac");
        } else {
            hint.mime_type(mime);
        }
    }

    if let Some(path_part) = url.split('?').next() {
        if let Some(ext) = path_part.rsplit('.').next() {
            if ext.len() <= 4 && !ext.contains('/') {
                hint.with_extension(ext);
            }
        }
    }

    SymphoniaSource::from_reader(resp, hint)
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
            cover_url: None,
            added_at: None,
        }
    }

    /// Build a streaming track using a title (and optional artist/duration) we
    /// already know — e.g. parsed from a `.m3u` `#EXTINF` line. Avoids the
    /// "<url> (stream)" placeholder title when we have the real name on hand.
    pub fn from_url_with_meta(
        url: String,
        title: Option<String>,
        artist: Option<String>,
        duration: Option<Duration>,
    ) -> Self {
        let mut t = Self::from_url(url);
        if let Some(title) = title {
            t.title = title;
        }
        t.artist = artist;
        t.duration = duration;
        t
    }
}
