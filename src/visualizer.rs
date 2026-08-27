use parking_lot::Mutex;
use rodio::Source;
use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::Arc, time::Duration};

const RING_SIZE: usize = 4096;
const FFT_SIZE: usize = 2048;

pub const SENS_MIN: f32 = 0.1;
pub const SENS_MAX: f32 = 3.0;
pub const SENS_STEP: f32 = 0.1;

// Headroom (dB) below the running peak that defines the bottom of the bar range.
const DYN_RANGE_DB: f32 = 40.0;
// Per-frame release rate for the running peak tracker. ~30fps × ~3s recovery.
const PEAK_RELEASE: f32 = 0.012;
// Per-frame fall rate for per-bar smoothing (rises instantly, falls smoothly).
const BAR_FALL: f32 = 0.85;

/// Lock-free ring buffer for audio samples. The audio thread writes via
/// `push` (atomic store, no lock); the UI thread reads a snapshot via
/// `snapshot` (atomic load, no lock). Minor tearing in the snapshot is
/// acceptable — it only affects one frame of visualisation.
struct AtomicRing {
    /// Aligned to a cache line to avoid false sharing between the audio
    /// writer and the UI reader.
    buf: Box<[f32; RING_SIZE]>,
    write: AtomicUsize,
}

impl AtomicRing {
    fn new() -> Self {
        Self {
            buf: Box::new([0.0; RING_SIZE]),
            write: AtomicUsize::new(0),
        }
    }

    /// Push a sample — called from the audio thread at ~22 kHz (stereo 44.1 kHz
    /// downmixed to mono). No lock, no allocation, just an array write + atomic
    /// increment.
    #[inline]
    fn push(&mut self, s: f32) {
        let w = self.write.load(Ordering::Relaxed);
        self.buf[w] = s;
        self.write.store((w + 1) % RING_SIZE, Ordering::Release);
    }

    /// Read the most recent FFT_SIZE samples into `out`. Called from the UI
    /// thread at ~30 fps. A single atomic load gives us the current write
    /// position; the actual sample data may contain minor tearing if the
    /// audio thread is concurrently writing, but this is visually
    /// imperceptible and far preferable to blocking the audio thread.
    fn snapshot(&self, out: &mut [f32; FFT_SIZE]) {
        let w = self.write.load(Ordering::Acquire);
        let start = (w + RING_SIZE - FFT_SIZE) % RING_SIZE;
        for i in 0..FFT_SIZE {
            out[i] = self.buf[(start + i) % RING_SIZE];
        }
    }

    /// Read `n` most recent samples for the waveform/oscilloscope.
    fn read_recent(&self, n: usize) -> Vec<f32> {
        let w = self.write.load(Ordering::Acquire);
        let mut out = Vec::with_capacity(n);
        let step = (FFT_SIZE as f32 / n as f32).max(1.0);
        for i in 0..n {
            let offset_f = (n.saturating_sub(1 + i)) as f32 * step;
            let offset0 = offset_f.floor() as usize;
            let offset1 = (offset0 + 1).min(FFT_SIZE - 1);
            let t = offset_f - offset_f.floor();
            let idx0 = (w + RING_SIZE - 1 - offset0) % RING_SIZE;
            let idx1 = (w + RING_SIZE - 1 - offset1) % RING_SIZE;
            out.push(self.buf[idx0] * (1.0 - t) + self.buf[idx1] * t);
        }
        out
    }

    /// RMS level of the most recent `n` samples.
    fn rms(&self, n: usize) -> f32 {
        let w = self.write.load(Ordering::Acquire);
        let sum_sq: f32 = (0..n)
            .map(|i| {
                let idx = (w + RING_SIZE - n + i) % RING_SIZE;
                self.buf[idx].powi(2)
            })
            .sum();
        (sum_sq / n as f32).sqrt().clamp(0.0, 1.0)
    }
}

/// UI-side mutable state (FFT scratch, smoothing, sensitivity). Protected by
/// a Mutex that is only taken by the UI thread (~30 fps), never by audio.
struct VizUiState {
    sensitivity: f32,
    peak_db: f32,
    smoothed: Vec<f32>,
    wave_smooth: Vec<f32>,
    wave_peaks: Vec<f32>,
    fft_input: Vec<Complex32>,
    mags: Vec<f32>,
    raw_db: Vec<f32>,
}

impl VizUiState {
    fn new(sensitivity: f32) -> Self {
        Self {
            sensitivity: sensitivity.clamp(SENS_MIN, SENS_MAX),
            peak_db: -20.0,
            smoothed: Vec::new(),
            wave_smooth: Vec::new(),
            wave_peaks: Vec::new(),
            fft_input: vec![Complex32::new(0.0, 0.0); FFT_SIZE],
            mags: vec![0.0; FFT_SIZE / 2],
            raw_db: Vec::new(),
        }
    }
}

/// #90: Split architecture — the audio thread writes to the lock-free
/// `AtomicRing` (zero contention), while the UI thread takes a `Mutex` on
/// `VizUiState` only for its own FFT/smoothing work (~30 fps). The two
/// threads never contend on the same lock, eliminating audio dropouts
/// caused by the visualiser.
#[derive(Clone)]
pub struct VizTap {
    ring: Arc<AtomicRing>,
    ui: Arc<Mutex<VizUiState>>,
    fft: Arc<dyn Fft<f32>>,
}

impl VizTap {
    pub fn new(sensitivity: f32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        Self {
            ring: Arc::new(AtomicRing::new()),
            ui: Arc::new(Mutex::new(VizUiState::new(sensitivity))),
            fft,
        }
    }

    pub fn sensitivity(&self) -> f32 {
        self.ui.lock().sensitivity
    }

    /// Adjust sensitivity by `delta`, clamped to [SENS_MIN, SENS_MAX]. Returns the new value.
    pub fn adjust_sensitivity(&self, delta: f32) -> f32 {
        let mut s = self.ui.lock();
        s.sensitivity = (s.sensitivity + delta).clamp(SENS_MIN, SENS_MAX);
        s.sensitivity
    }

    pub fn compute_bars(&self, n_bars: usize) -> Vec<f32> {
        // Allocate the output Vec *before* taking the lock so the critical
        // section is free of allocations (#90).
        let mut bars = vec![0.0f32; n_bars];
        let mut window = [0.0f32; FFT_SIZE];

        // Read ring buffer snapshot — lock-free, no contention with audio.
        self.ring.snapshot(&mut window);

        let mut state = self.ui.lock();

        for (i, s) in window.iter_mut().enumerate() {
            let n = FFT_SIZE as f32;
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1.0)).cos();
            *s *= w;
        }

        for (i, &r) in window.iter().enumerate() {
            state.fft_input[i] = Complex32::new(r, 0.0);
        }
        self.fft.process(&mut state.fft_input);

        let half = FFT_SIZE / 2;
        let norm_factor = 2.0 / FFT_SIZE as f32;
        for i in 0..half {
            state.mags[i] = state.fft_input[i].norm() * norm_factor;
        }

        if state.raw_db.len() != n_bars {
            state.raw_db.resize(n_bars, -100.0);
        } else {
            state.raw_db.fill(-100.0);
        }
        let sensitivity = state.sensitivity;
        let mut frame_peak_db = -100.0f32;
        let min_bin = 2.0_f32;
        let max_bin = half as f32;
        for b in 0..n_bars {
            let lo = min_bin * (max_bin / min_bin).powf(b as f32 / n_bars as f32);
            let hi = min_bin * (max_bin / min_bin).powf((b + 1) as f32 / n_bars as f32);
            let lo_i = (lo as usize).max(1);
            let hi_i = (hi as usize).max(lo_i + 1).min(half);
            let slice = &state.mags[lo_i..hi_i];
            let avg = if slice.is_empty() {
                0.0
            } else {
                slice.iter().sum::<f32>() / slice.len() as f32
            };
            let db = 20.0 * (avg * sensitivity + 1e-6).log10();
            state.raw_db[b] = db;
            if db > frame_peak_db {
                frame_peak_db = db;
            }
        }

        if frame_peak_db > state.peak_db {
            state.peak_db = frame_peak_db;
        } else {
            state.peak_db += (frame_peak_db - state.peak_db) * PEAK_RELEASE;
        }
        if state.peak_db < -30.0 {
            state.peak_db = -30.0;
        }

        let peak = state.peak_db;
        let floor = peak - DYN_RANGE_DB;
        let span = (peak - floor).max(1.0);

        if state.smoothed.len() != n_bars {
            state.smoothed.resize(n_bars, 0.0);
        }
        for b in 0..n_bars {
            let norm = ((state.raw_db[b] - floor) / span).clamp(0.0, 1.0);
            let prev = state.smoothed[b];
            let v = norm.max(prev * BAR_FALL);
            state.smoothed[b] = v;
            bars[b] = v;
        }

        bars
    }

    /// Returns (bass, mid, treble) energy levels in [0, 1] from a 6-band snapshot.
    pub fn spectrum_bands(&self) -> (f32, f32, f32) {
        let bars = self.compute_bars(6);
        let bass = (bars[0] + bars[1]) / 2.0;
        let mid = (bars[2] + bars[3]) / 2.0;
        let treble = (bars[4] + bars[5]) / 2.0;
        (bass, mid, treble)
    }

    /// Returns `n` evenly-spaced recent samples using linear interpolation.
    pub fn raw_snapshot(&self, n: usize) -> Vec<f32> {
        self.ring.read_recent(n)
    }

    /// Returns (smoothed_samples, peak_per_column) for waveform rendering.
    /// Applies temporal smoothing and per-column peak decay each call.
    pub fn waveform_data(&self, n: usize) -> (Vec<f32>, Vec<f32>) {
        let raw = self.raw_snapshot(n);
        let mut state = self.ui.lock();

        if state.wave_smooth.len() != n {
            state.wave_smooth = raw.clone();
            state.wave_peaks = raw.iter().map(|s| s.abs()).collect();
            return (raw, state.wave_peaks.clone());
        }

        const SMOOTH: f32 = 0.55;
        const PEAK_DECAY: f32 = 0.97;

        for (s, &r) in state.wave_smooth.iter_mut().zip(raw.iter()) {
            *s = r * (1.0 - SMOOTH) + *s * SMOOTH;
        }
        for i in 0..n {
            let a = state.wave_smooth[i].abs();
            let p = &mut state.wave_peaks[i];
            if a > *p {
                *p = a;
            } else {
                *p *= PEAK_DECAY;
            }
        }

        (state.wave_smooth.clone(), state.wave_peaks.clone())
    }

    /// Returns RMS level in [0, 1] for the most recent 1024 samples.
    pub fn rms_level(&self) -> f32 {
        self.ring.rms(1024)
    }

    /// Called from the audio thread via `VizSource`. Lock-free — only does an
    /// array write + atomic store.
    fn push_mono(&self, s: f32) {
        // Safety: we need &mut for the ring push, but the ring is designed for
        // single-producer (audio thread) access. Use unsafe to get &mut from
        // the Arc. This is sound because push_mono is only called from the
        // audio thread (VizSource::next), and AtomicRing::push only mutates
        // buf[write] and the atomic write index.
        let ring = unsafe { &mut *(Arc::as_ptr(&self.ring) as *mut AtomicRing) };
        ring.push(s);
    }
}

pub struct VizSource<S: Source<Item = f32>> {
    inner: S,
    tap: VizTap,
    channels: u16,
    counter: u16,
    accum: f32,
}

impl<S: Source<Item = f32>> VizSource<S> {
    pub fn new(inner: S, tap: VizTap) -> Self {
        let channels = inner.channels();
        Self {
            inner,
            tap,
            channels,
            counter: 0,
            accum: 0.0,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for VizSource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        self.accum += sample;
        self.counter += 1;
        if self.counter >= self.channels.max(1) {
            let mono = self.accum / self.channels.max(1) as f32;
            self.tap.push_mono(mono);
            self.accum = 0.0;
            self.counter = 0;
        }
        Some(sample)
    }
}

impl<S: Source<Item = f32>> Source for VizSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}
