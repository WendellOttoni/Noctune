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
    pub genre: Option<String>,
    pub year: Option<String>,
    pub duration: Option<Duration>,
    pub replaygain_track_db: Option<f32>,
    pub replaygain_album_db: Option<f32>,
}

fn parse_rg_db(s: &str) -> Option<f32> {
    s.trim().trim_end_matches("dB").trim().parse().ok()
}

#[derive(Debug, Clone, Default)]
pub struct FullMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<String>,
    pub duration: Option<Duration>,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bits_per_sample: Option<u32>,
}

pub fn probe_full(path: &Path) -> FullMeta {
    let Ok(file) = File::open(path) else { return FullMeta::default() };
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }
    let Ok(mut probed) = symphonia::default::get_probe().format(
        &hint, mss, &FormatOptions::default(), &MetadataOptions::default(),
    ) else { return FullMeta::default() };

    let mut m = FullMeta::default();

    if let Some(track) = probed.format.default_track() {
        let p = &track.codec_params;
        m.sample_rate = p.sample_rate;
        m.channels = p.channels.map(|c| c.count() as u8);
        m.bits_per_sample = p.bits_per_sample;
        m.codec = Some(format!("{:?}", p.codec));
        if let (Some(n), Some(rate)) = (p.n_frames, p.sample_rate) {
            m.duration = Some(Duration::from_secs_f64(n as f64 / rate as f64));
        }
    }

    let mut read_tags = |tags: &[symphonia::core::meta::Tag]| {
        for tag in tags {
            let Some(key) = tag.std_key else { continue };
            let v = tag.value.to_string();
            match key {
                StandardTagKey::TrackTitle if m.title.is_none() => m.title = Some(v),
                StandardTagKey::Artist if m.artist.is_none() => m.artist = Some(v),
                StandardTagKey::AlbumArtist if m.album_artist.is_none() => m.album_artist = Some(v),
                StandardTagKey::Album if m.album.is_none() => m.album = Some(v),
                StandardTagKey::Date | StandardTagKey::ReleaseDate
                    if m.year.is_none() => m.year = Some(v),
                StandardTagKey::Genre if m.genre.is_none() => m.genre = Some(v),
                StandardTagKey::TrackNumber if m.track_number.is_none() => m.track_number = Some(v),
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
    m
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
                StandardTagKey::Genre if meta.genre.is_none() => meta.genre = Some(value),
                StandardTagKey::Date | StandardTagKey::ReleaseDate
                    if meta.year.is_none() => meta.year = Some(value),
                StandardTagKey::ReplayGainTrackGain if meta.replaygain_track_db.is_none() => {
                    meta.replaygain_track_db = parse_rg_db(&value);
                }
                StandardTagKey::ReplayGainAlbumGain if meta.replaygain_album_db.is_none() => {
                    meta.replaygain_album_db = parse_rg_db(&value);
                }
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
