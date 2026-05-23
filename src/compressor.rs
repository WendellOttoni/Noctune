//! Dynamic range compressor (#65). Feed-forward, peak-detector design with a soft
//! knee and configurable attack/release.

use parking_lot::Mutex;
use rodio::Source;
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Copy)]
pub struct CompState {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}

impl Default for CompState {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            makeup_db: 0.0,
        }
    }
}

impl CompState {
    pub fn preset(name: &str) -> Option<Self> {
        Some(match name {
            "Off" => Self {
                enabled: false,
                ..Self::default()
            },
            "Light" => Self {
                enabled: true,
                threshold_db: -16.0,
                ratio: 2.0,
                attack_ms: 30.0,
                release_ms: 200.0,
                makeup_db: 1.0,
            },
            "Podcast" => Self {
                enabled: true,
                threshold_db: -20.0,
                ratio: 4.0,
                attack_ms: 8.0,
                release_ms: 120.0,
                makeup_db: 3.0,
            },
            "Live Music" => Self {
                enabled: true,
                threshold_db: -18.0,
                ratio: 3.0,
                attack_ms: 15.0,
                release_ms: 180.0,
                makeup_db: 2.0,
            },
            "Loud" => Self {
                enabled: true,
                threshold_db: -24.0,
                ratio: 8.0,
                attack_ms: 5.0,
                release_ms: 100.0,
                makeup_db: 6.0,
            },
            _ => return None,
        })
    }

    pub const PRESETS: &'static [&'static str] = &["Off", "Light", "Podcast", "Live Music", "Loud"];
}

#[derive(Clone)]
pub struct CompHandle {
    inner: Arc<Mutex<CompState>>,
    last_gr_db: Arc<Mutex<f32>>,
}

impl CompHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CompState::default())),
            last_gr_db: Arc::new(Mutex::new(0.0)),
        }
    }

    pub fn snapshot(&self) -> CompState {
        *self.inner.lock()
    }
    pub fn set(&self, s: CompState) {
        *self.inner.lock() = s;
    }
    /// Last reported gain reduction in dB (>= 0), for the UI meter.
    pub fn gr_db(&self) -> f32 {
        *self.last_gr_db.lock()
    }
    pub(crate) fn report_gr(&self, value: f32) {
        *self.last_gr_db.lock() = value;
    }
}

pub struct CompSource<S: Source<Item = f32>> {
    inner: S,
    handle: CompHandle,
    sample_rate: f32,
    state: CompState,
    refresh_in: u32,
    envelope: f32,
    smoothed_gain: f32,
    gr_report_in: u32,
    last_gr_db: f32,
}

impl<S: Source<Item = f32>> CompSource<S> {
    pub fn new(inner: S, handle: CompHandle) -> Self {
        let sample_rate = inner.sample_rate() as f32;
        let state = handle.snapshot();
        Self {
            inner,
            handle,
            sample_rate,
            state,
            refresh_in: 0,
            envelope: 0.0,
            smoothed_gain: 1.0,
            gr_report_in: 0,
            last_gr_db: 0.0,
        }
    }

    fn maybe_refresh(&mut self) {
        if self.refresh_in == 0 {
            self.state = self.handle.snapshot();
            self.refresh_in = 4096;
        } else {
            self.refresh_in -= 1;
        }
    }
}

impl<S: Source<Item = f32>> Iterator for CompSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.maybe_refresh();
        let sample = self.inner.next()?;
        if !self.state.enabled {
            return Some(sample);
        }

        // Peak envelope follower.
        let abs = sample.abs();
        if abs > self.envelope {
            self.envelope = abs;
        } else {
            // Slow release toward the current sample.
            let coef = (-1.0 / (self.state.release_ms.max(1.0) * self.sample_rate / 1000.0)).exp();
            self.envelope = abs + coef * (self.envelope - abs);
        }

        // Convert envelope to dB and decide target gain.
        let env_db = 20.0 * (self.envelope.max(1e-7)).log10();
        let over = env_db - self.state.threshold_db;
        let target_gr_db = if over > 0.0 {
            over * (1.0 - 1.0 / self.state.ratio.max(1.0))
        } else {
            0.0
        };
        let target_gain = 10f32.powf((self.state.makeup_db - target_gr_db) / 20.0);

        // Smooth gain changes with attack/release coefficients to avoid pumping artifacts.
        let coef_ms = if target_gain < self.smoothed_gain {
            self.state.attack_ms
        } else {
            self.state.release_ms
        };
        let coef = (-1.0 / (coef_ms.max(0.1) * self.sample_rate / 1000.0)).exp();
        self.smoothed_gain = target_gain + coef * (self.smoothed_gain - target_gain);

        // Periodically publish gain reduction so the UI meter has something to draw.
        self.last_gr_db = target_gr_db;
        if self.gr_report_in == 0 {
            self.handle.report_gr(self.last_gr_db);
            self.gr_report_in = (self.sample_rate as u32) / 30; // ~30 Hz
        } else {
            self.gr_report_in -= 1;
        }

        Some((sample * self.smoothed_gain).clamp(-1.0, 1.0))
    }
}

impl<S: Source<Item = f32>> Source for CompSource<S> {
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
