//! Lightweight streaming pre-buffer / prefetch system.
//! Prepares initial streaming decoders for adjacent previous and next tracks
//! in background threads to enable instantaneous (0ms) manual track switching.

use std::path::PathBuf;
use crate::audio::SymphoniaSource;

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
    pub rx: Option<std::sync::mpsc::Receiver<(SlotKind, PathBuf, Result<SymphoniaSource, String>)>>,
    pub tx: Option<std::sync::mpsc::Sender<(SlotKind, PathBuf, Result<SymphoniaSource, String>)>>,
}

impl PrefetchSlots {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            next: None,
            prev: None,
            building_next: None,
            building_prev: None,
            rx: Some(rx),
            tx: Some(tx),
        }
    }

    pub fn invalidate(&mut self) {
        self.next = None;
        self.prev = None;
        self.building_next = None;
        self.building_prev = None;
    }
}
