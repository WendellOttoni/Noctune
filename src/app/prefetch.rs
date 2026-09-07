//! Lightweight streaming pre-buffer / prefetch system.
//! Prepares initial streaming decoders for adjacent previous and next tracks
//! in background threads to enable instantaneous (0ms) manual track switching.

use crate::audio::SymphoniaSource;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    Next,
    Prev,
}

pub struct PreloadedTrack {
    pub path: PathBuf,
    pub source: SymphoniaSource,
}

pub struct PrefetchSlots {
    pub next: Option<PreloadedTrack>,
    pub prev: Option<PreloadedTrack>,
    pub building_next: Option<PathBuf>,
    pub building_prev: Option<PathBuf>,
    pub next_worker: crate::worker::LatestWorker,
    pub prev_worker: crate::worker::LatestWorker,
    pub next_request: u64,
    pub prev_request: u64,
    pub rx: Option<
        std::sync::mpsc::Receiver<(SlotKind, u64, PathBuf, Result<SymphoniaSource, String>)>,
    >,
    pub tx:
        Option<std::sync::mpsc::Sender<(SlotKind, u64, PathBuf, Result<SymphoniaSource, String>)>>,
}

impl PrefetchSlots {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            next: None,
            prev: None,
            building_next: None,
            building_prev: None,
            next_worker: crate::worker::LatestWorker::new(),
            prev_worker: crate::worker::LatestWorker::new(),
            next_request: 0,
            prev_request: 0,
            rx: Some(rx),
            tx: Some(tx),
        }
    }

    pub fn invalidate(&mut self) {
        self.next_worker.cancel();
        self.prev_worker.cancel();
        self.next_request = self.next_request.wrapping_add(1);
        self.prev_request = self.prev_request.wrapping_add(1);
        self.next = None;
        self.prev = None;
        self.building_next = None;
        self.building_prev = None;
    }
}
