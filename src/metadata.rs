use std::{fs::File, path::Path, time::Duration};
use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey},
    probe::Hint,
};

#[derive(Debug, Clone, Default)]
pub struct TrackMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
}

pub fn probe(path: &Path) -> TrackMeta {
    let Ok(file) = File::open(path) else {
        return TrackMeta::default();
    };
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let Ok(mut probed) = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return TrackMeta::default();
    };

    let mut meta = TrackMeta::default();

    let track = probed.format.default_track();
    if let Some(track) = track {
        let params = &track.codec_params;
        if let (Some(n_frames), Some(sample_rate)) = (params.n_frames, params.sample_rate) {
            let secs = n_frames as f64 / sample_rate as f64;
            meta.duration = Some(Duration::from_secs_f64(secs));
        }
    }

    let mut read_tags = |tags: &[symphonia::core::meta::Tag]| {
        for tag in tags {
            let Some(key) = tag.std_key else { continue };
            let value = tag.value.to_string();
            match key {
                StandardTagKey::TrackTitle if meta.title.is_none() => meta.title = Some(value),
                StandardTagKey::Artist if meta.artist.is_none() => meta.artist = Some(value),
                StandardTagKey::AlbumArtist if meta.artist.is_none() => meta.artist = Some(value),
                StandardTagKey::Album if meta.album.is_none() => meta.album = Some(value),
                _ => {}
            }
        }
    };

    if let Some(rev) = probed.format.metadata().current() {
        read_tags(rev.tags());
    }
    if let Some(mut md) = probed.metadata.get() {
        if let Some(rev) = md.skip_to_latest() {
            read_tags(rev.tags());
        }
    }

    meta
}
