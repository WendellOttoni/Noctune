use parking_lot::Mutex;
use rodio::Source;
use std::{sync::Arc, time::Duration};

pub const NUM_BANDS: usize = 10;

pub const BAND_FREQS: [f32; NUM_BANDS] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

pub const BAND_LABELS: [&str; NUM_BANDS] = [
    "31Hz", "62Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqState {
    pub bands: [f32; NUM_BANDS],
}

impl Default for EqState {
    fn default() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
        }
    }
}

impl EqState {
    pub fn from_3band(low: f32, mid: f32, high: f32) -> Self {
        Self {
            bands: [
                low,
                low * 0.9,
                low * 0.7,
                mid * 0.5,
                mid,
                mid,
                mid * 0.5,
                high * 0.7,
                high * 0.9,
                high,
            ],
        }
    }

    pub fn from_bands(bands: [f32; NUM_BANDS]) -> Self {
        Self { bands }
    }

    pub fn low_db(&self) -> f32 {
        (self.bands[0] + self.bands[1] + self.bands[2]) / 3.0
    }

    pub fn mid_db(&self) -> f32 {
        (self.bands[3] + self.bands[4] + self.bands[5] + self.bands[6]) / 4.0
    }

    pub fn high_db(&self) -> f32 {
        (self.bands[7] + self.bands[8] + self.bands[9]) / 3.0
    }

    pub fn is_flat(&self) -> bool {
        self.bands.iter().all(|b| b.abs() < f32::EPSILON)
    }
}

#[derive(Clone)]
pub struct EqHandle {
    inner: Arc<Mutex<EqState>>,
}

impl EqHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EqState::default())),
        }
    }

    pub fn snapshot(&self) -> EqState {
        *self.inner.lock()
    }

    pub fn adjust_band(&self, band: usize, delta: f32) {
        let mut s = self.inner.lock();
        if band < NUM_BANDS {
            s.bands[band] = (s.bands[band] + delta).clamp(-12.0, 12.0);
        }
    }

    pub fn set_band(&self, band: usize, gain_db: f32) {
        let mut s = self.inner.lock();
        if band < NUM_BANDS {
            s.bands[band] = gain_db.clamp(-12.0, 12.0);
        }
    }

    pub fn adjust_low(&self, delta: f32) {
        let mut s = self.inner.lock();
        for i in 0..3 {
            s.bands[i] = (s.bands[i] + delta).clamp(-12.0, 12.0);
        }
    }

    pub fn adjust_mid(&self, delta: f32) {
        let mut s = self.inner.lock();
        for i in 3..7 {
            s.bands[i] = (s.bands[i] + delta).clamp(-12.0, 12.0);
        }
    }

    pub fn adjust_high(&self, delta: f32) {
        let mut s = self.inner.lock();
        for i in 7..10 {
            s.bands[i] = (s.bands[i] + delta).clamp(-12.0, 12.0);
        }
    }

    pub fn set(&self, state: EqState) {
        *self.inner.lock() = state;
    }
}

pub const PRESETS: &[(&str, EqState)] = &[
    (
        "Flat",
        EqState {
            bands: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        },
    ),
    (
        "Bass Boost",
        EqState {
            bands: [7.0, 6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 1.0, 2.0, 2.0],
        },
    ),
    (
        "Vocal Boost",
        EqState {
            bands: [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 2.0, 1.0, 0.0],
        },
    ),
    (
        "Treble Boost",
        EqState {
            bands: [-2.0, -2.0, -1.0, 0.0, 0.0, 1.0, 3.0, 5.0, 7.0, 8.0],
        },
    ),
    (
        "Rock",
        EqState {
            bands: [5.0, 4.0, 3.0, 1.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0],
        },
    ),
    (
        "Electronic",
        EqState {
            bands: [6.0, 5.0, 4.0, 1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        },
    ),
    (
        "Hip-Hop",
        EqState {
            bands: [7.0, 6.5, 5.0, 2.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        },
    ),
    (
        "Jazz",
        EqState {
            bands: [3.0, 3.0, 2.0, 1.0, 2.0, 2.0, 1.0, 2.0, 3.0, 3.0],
        },
    ),
    (
        "Classical",
        EqState {
            bands: [4.0, 3.0, 2.5, 2.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
        },
    ),
    (
        "Acoustic",
        EqState {
            bands: [3.5, 3.0, 2.5, 1.5, 2.0, 2.5, 3.0, 3.5, 3.0, 2.5],
        },
    ),
    (
        "Loudness",
        EqState {
            bands: [6.0, 5.0, 3.0, 0.0, -1.0, 0.0, 1.0, 3.0, 5.0, 6.0],
        },
    ),
    (
        "V-Shape",
        EqState {
            bands: [8.0, 6.0, 4.0, 0.0, -4.0, -4.0, 0.0, 4.0, 6.0, 8.0],
        },
    ),
];

#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn low_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let s = 1.0;
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn high_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let s = 1.0;
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn peaking(sample_rate: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

pub struct EqSource<S: Source<Item = f32>> {
    inner: S,
    handle: EqHandle,
    sample_rate: f32,
    channels: u16,
    chan_idx: u16,
    state: EqState,
    refresh_in: u32,
    filters: Vec<[Biquad; NUM_BANDS]>,
}

impl<S: Source<Item = f32>> EqSource<S> {
    pub fn new(inner: S, handle: EqHandle) -> Self {
        let sample_rate = inner.sample_rate() as f32;
        let channels = inner.channels();
        let state = handle.snapshot();
        let filters = (0..channels)
            .map(|_| Self::build_filters(sample_rate, &state))
            .collect();
        Self {
            inner,
            handle,
            sample_rate,
            channels,
            chan_idx: 0,
            state,
            refresh_in: 0,
            filters,
        }
    }

    fn build_filters(rate: f32, s: &EqState) -> [Biquad; NUM_BANDS] {
        let q = 1.414;
        let mut filters = [Biquad::default(); NUM_BANDS];
        filters[0] = Biquad::low_shelf(rate, BAND_FREQS[0] * 1.5, s.bands[0]);
        for i in 1..NUM_BANDS - 1 {
            filters[i] = Biquad::peaking(rate, BAND_FREQS[i], q, s.bands[i]);
        }
        filters[NUM_BANDS - 1] = Biquad::high_shelf(
            rate,
            BAND_FREQS[NUM_BANDS - 1] / 1.5,
            s.bands[NUM_BANDS - 1],
        );
        filters
    }

    fn maybe_refresh(&mut self) {
        if self.refresh_in == 0 {
            let snap = self.handle.snapshot();
            if snap.bands != self.state.bands {
                self.state = snap;
                for ch in self.filters.iter_mut() {
                    let coeffs = Self::build_filters(self.sample_rate, &self.state);
                    for i in 0..NUM_BANDS {
                        ch[i].b0 = coeffs[i].b0;
                        ch[i].b1 = coeffs[i].b1;
                        ch[i].b2 = coeffs[i].b2;
                        ch[i].a1 = coeffs[i].a1;
                        ch[i].a2 = coeffs[i].a2;
                    }
                }
            }
            self.refresh_in = 4096;
        } else {
            self.refresh_in -= 1;
        }
    }
}

impl<S: Source<Item = f32>> Iterator for EqSource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.maybe_refresh();
        let s = self.inner.next()?;
        if self.state.is_flat() {
            self.chan_idx = (self.chan_idx + 1) % self.channels.max(1);
            return Some(s);
        }
        let ch = self.chan_idx as usize;
        let chain = &mut self.filters[ch];
        let mut out = s;
        for f in chain.iter_mut() {
            out = f.process(out);
        }
        self.chan_idx = (self.chan_idx + 1) % self.channels.max(1);
        Some(out.clamp(-1.0, 1.0))
    }
}

impl<S: Source<Item = f32>> Source for EqSource<S> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_adjusts_within_clamp() {
        let h = EqHandle::new();
        assert_eq!(h.snapshot().bands[0], 0.0);
        h.adjust_band(0, 3.0);
        assert!((h.snapshot().bands[0] - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn handle_clamps_at_extremes() {
        let h = EqHandle::new();
        h.adjust_band(0, 100.0);
        assert_eq!(h.snapshot().bands[0], 12.0);
        h.adjust_band(0, -100.0);
        assert_eq!(h.snapshot().bands[0], -12.0);
    }

    #[test]
    fn handle_set_replaces_state() {
        let h = EqHandle::new();
        h.set(EqState {
            bands: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        });
        let s = h.snapshot();
        assert_eq!(s.bands[0], 1.0);
        assert_eq!(s.bands[9], 10.0);
    }

    #[test]
    fn presets_are_named_and_in_range() {
        assert!(PRESETS.iter().any(|(name, _)| *name == "Flat"));
        for (_, st) in PRESETS {
            for &b in &st.bands {
                assert!(b >= -12.0 && b <= 12.0);
            }
        }
    }
}
