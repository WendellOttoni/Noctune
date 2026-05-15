use parking_lot::Mutex;
use rodio::Source;
use rustfft::{num_complex::Complex32, FftPlanner};
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

pub struct SampleRing {
    buf: [f32; RING_SIZE],
    write: usize,
}

impl SampleRing {
    fn new() -> Self {
        Self {
            buf: [0.0; RING_SIZE],
            write: 0,
        }
    }

    fn push(&mut self, s: f32) {
        self.buf[self.write] = s;
        self.write = (self.write + 1) % RING_SIZE;
    }

    fn snapshot(&self, out: &mut [f32; FFT_SIZE]) {
        let start = (self.write + RING_SIZE - FFT_SIZE) % RING_SIZE;
        for i in 0..FFT_SIZE {
            out[i] = self.buf[(start + i) % RING_SIZE];
        }
    }
}

struct VizState {
    sensitivity: f32,
    peak_db: f32,
    smoothed: Vec<f32>,
}

impl VizState {
    fn new(sensitivity: f32) -> Self {
        Self {
            sensitivity: sensitivity.clamp(SENS_MIN, SENS_MAX),
            peak_db: -20.0,
            smoothed: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct VizTap {
    ring: Arc<Mutex<SampleRing>>,
    state: Arc<Mutex<VizState>>,
}

impl VizTap {
    pub fn new(sensitivity: f32) -> Self {
        Self {
            ring: Arc::new(Mutex::new(SampleRing::new())),
            state: Arc::new(Mutex::new(VizState::new(sensitivity))),
        }
    }

    pub fn sensitivity(&self) -> f32 {
        self.state.lock().sensitivity
    }

    /// Adjust sensitivity by `delta`, clamped to [SENS_MIN, SENS_MAX]. Returns the new value.
    pub fn adjust_sensitivity(&self, delta: f32) -> f32 {
        let mut s = self.state.lock();
        s.sensitivity = (s.sensitivity + delta).clamp(SENS_MIN, SENS_MAX);
        s.sensitivity
    }

    pub fn compute_bars(&self, n_bars: usize) -> Vec<f32> {
        let mut window = [0.0f32; FFT_SIZE];
        {
            let ring = self.ring.lock();
            ring.snapshot(&mut window);
        }

        for (i, s) in window.iter_mut().enumerate() {
            let n = FFT_SIZE as f32;
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1.0)).cos();
            *s *= w;
        }

        let mut buf: Vec<Complex32> = window.iter().map(|&r| Complex32::new(r, 0.0)).collect();
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        fft.process(&mut buf);

        let half = FFT_SIZE / 2;
        // Normalize FFT magnitude by window length so peak_db is roughly in dBFS.
        let norm_factor = 2.0 / FFT_SIZE as f32;
        let mags: Vec<f32> = buf[..half].iter().map(|c| c.norm() * norm_factor).collect();

        let mut state = self.state.lock();
        let sensitivity = state.sensitivity;

        let mut raw_db = vec![-100.0f32; n_bars];
        let mut frame_peak_db = -100.0f32;
        let min_bin = 2.0_f32;
        let max_bin = half as f32;
        for b in 0..n_bars {
            let lo = min_bin * (max_bin / min_bin).powf(b as f32 / n_bars as f32);
            let hi = min_bin * (max_bin / min_bin).powf((b + 1) as f32 / n_bars as f32);
            let lo_i = (lo as usize).max(1);
            let hi_i = (hi as usize).max(lo_i + 1).min(half);
            let slice = &mags[lo_i..hi_i];
            let avg = if slice.is_empty() {
                0.0
            } else {
                slice.iter().sum::<f32>() / slice.len() as f32
            };
            let db = 20.0 * (avg * sensitivity + 1e-6).log10();
            raw_db[b] = db;
            if db > frame_peak_db {
                frame_peak_db = db;
            }
        }

        // Fast attack, slow release on the peak reference (the "0 dB" of the display).
        if frame_peak_db > state.peak_db {
            state.peak_db = frame_peak_db;
        } else {
            state.peak_db = state.peak_db + (frame_peak_db - state.peak_db) * PEAK_RELEASE;
        }
        // Don't let the peak collapse to silence — keep a sane minimum reference.
        if state.peak_db < -30.0 {
            state.peak_db = -30.0;
        }

        let peak = state.peak_db;
        let floor = peak - DYN_RANGE_DB;
        let span = (peak - floor).max(1.0);

        if state.smoothed.len() != n_bars {
            state.smoothed = vec![0.0; n_bars];
        }
        let mut bars = vec![0.0f32; n_bars];
        for b in 0..n_bars {
            let norm = ((raw_db[b] - floor) / span).clamp(0.0, 1.0);
            let prev = state.smoothed[b];
            let v = norm.max(prev * BAR_FALL);
            state.smoothed[b] = v;
            bars[b] = v;
        }

        bars
    }

    fn push_mono(&self, s: f32) {
        self.ring.lock().push(s);
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
