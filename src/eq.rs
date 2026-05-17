use parking_lot::Mutex;
use rodio::Source;
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Copy)]
pub struct EqState {
    pub low_db: f32,
    pub mid_db: f32,
    pub high_db: f32,
}

impl Default for EqState {
    fn default() -> Self {
        Self {
            low_db: 0.0,
            mid_db: 0.0,
            high_db: 0.0,
        }
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

    pub fn adjust_low(&self, delta: f32) {
        let mut s = self.inner.lock();
        s.low_db = (s.low_db + delta).clamp(-12.0, 12.0);
    }

    pub fn adjust_mid(&self, delta: f32) {
        let mut s = self.inner.lock();
        s.mid_db = (s.mid_db + delta).clamp(-12.0, 12.0);
    }

    pub fn adjust_high(&self, delta: f32) {
        let mut s = self.inner.lock();
        s.high_db = (s.high_db + delta).clamp(-12.0, 12.0);
    }

    pub fn set(&self, state: EqState) {
        *self.inner.lock() = state;
    }
}

pub const PRESETS: &[(&str, EqState)] = &[
    ("Flat",    EqState { low_db:  0.0, mid_db:  0.0, high_db:  0.0 }),
    ("Bass",    EqState { low_db:  6.0, mid_db:  0.0, high_db:  2.0 }),
    ("Vocal",   EqState { low_db: -2.0, mid_db:  4.0, high_db:  1.0 }),
    ("Treble",  EqState { low_db: -2.0, mid_db:  0.0, high_db:  6.0 }),
    ("Loud",    EqState { low_db:  5.0, mid_db:  1.0, high_db:  5.0 }),
    ("V-Shape", EqState { low_db:  8.0, mid_db: -6.0, high_db:  8.0 }),
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
    filters: Vec<[Biquad; 3]>,
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

    fn build_filters(rate: f32, s: &EqState) -> [Biquad; 3] {
        [
            Biquad::low_shelf(rate, 200.0, s.low_db),
            Biquad::peaking(rate, 1000.0, 0.9, s.mid_db),
            Biquad::high_shelf(rate, 5000.0, s.high_db),
        ]
    }

    fn maybe_refresh(&mut self) {
        if self.refresh_in == 0 {
            let snap = self.handle.snapshot();
            if snap.low_db != self.state.low_db
                || snap.mid_db != self.state.mid_db
                || snap.high_db != self.state.high_db
            {
                self.state = snap;
                for ch in self.filters.iter_mut() {
                    let coeffs = Self::build_filters(self.sample_rate, &self.state);
                    ch[0].b0 = coeffs[0].b0;
                    ch[0].b1 = coeffs[0].b1;
                    ch[0].b2 = coeffs[0].b2;
                    ch[0].a1 = coeffs[0].a1;
                    ch[0].a2 = coeffs[0].a2;
                    ch[1].b0 = coeffs[1].b0;
                    ch[1].b1 = coeffs[1].b1;
                    ch[1].b2 = coeffs[1].b2;
                    ch[1].a1 = coeffs[1].a1;
                    ch[1].a2 = coeffs[1].a2;
                    ch[2].b0 = coeffs[2].b0;
                    ch[2].b1 = coeffs[2].b1;
                    ch[2].b2 = coeffs[2].b2;
                    ch[2].a1 = coeffs[2].a1;
                    ch[2].a2 = coeffs[2].a2;
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
        let bypass = self.state.low_db == 0.0
            && self.state.mid_db == 0.0
            && self.state.high_db == 0.0;
        if bypass {
            self.chan_idx = (self.chan_idx + 1) % self.channels.max(1);
            return Some(s);
        }
        let ch = self.chan_idx as usize;
        let chain = &mut self.filters[ch];
        let mut out = chain[0].process(s);
        out = chain[1].process(out);
        out = chain[2].process(out);
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
