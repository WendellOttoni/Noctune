use image::DynamicImage;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Kitty,
    Iterm2,
    Blocks,
}

impl Protocol {
    /// Detect from environment variables (safe to call after raw mode).
    pub fn detect() -> Self {
        if std::env::var("KITTY_WINDOW_ID").is_ok()
            || std::env::var("TERM")
                .map_or(false, |t| t.contains("kitty"))
        {
            return Self::Kitty;
        }
        if std::env::var("TERM_PROGRAM")
            .map_or(false, |t| t == "iTerm.app")
            || std::env::var("LC_TERMINAL")
                .map_or(false, |t| t == "iTerm2")
        {
            return Self::Iterm2;
        }
        Self::Blocks
    }
}

pub struct ArtPicker {
    pub protocol: Protocol,
    /// Single-slot cache for the most recently emitted overlay escape sequence
    /// (#91). One slot is enough because only the currently-playing track's art
    /// is rendered; track change or resize invalidates via `key` mismatch.
    cache: Option<(ArtCacheKey, Vec<u8>)>,
}

/// Identifies a cached overlay payload. `generation` is bumped by the caller
/// whenever the underlying `DynamicImage` is replaced; area fields cover resize
/// invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtCacheKey {
    pub generation: u64,
    pub protocol: Protocol,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl ArtPicker {
    pub fn new() -> Self {
        Self {
            protocol: Protocol::detect(),
            cache: None,
        }
    }

    pub fn load(&self, bytes: &[u8]) -> Option<DynamicImage> {
        image::load_from_memory(bytes).ok()
    }

    /// Return cached escape sequence for `key`, or build it via `build` and
    /// store it. The closure runs at most once per (generation, area, protocol).
    pub fn cached_overlay<F>(&mut self, key: ArtCacheKey, build: F) -> &[u8]
    where
        F: FnOnce() -> Vec<u8>,
    {
        let hit = matches!(&self.cache, Some((k, _)) if *k == key);
        if !hit {
            self.cache = Some((key, build()));
        }
        // Safe: we just ensured cache is Some.
        self.cache.as_ref().map(|(_, b)| b.as_slice()).unwrap_or(&[])
    }

    /// Drop the cached payload — call when the album art image itself changes.
    pub fn invalidate(&mut self) {
        self.cache = None;
    }
}

/// Renders album art in `art_area` using half-block Unicode cells.
/// Each character cell is split vertically: upper half = fg color, lower half = bg color.
pub fn render_blocks(f: &mut Frame, area: Rect, img: &DynamicImage) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let w = area.width as u32;
    // Two pixel rows per character row
    let h = (area.height * 2) as u32;
    let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
    let rgb = resized.to_rgb8();

    let lines: Vec<Line> = (0..area.height)
        .map(|row| {
            let spans: Vec<Span> = (0..area.width)
                .map(|col| {
                    let top = rgb.get_pixel(col as u32, row as u32 * 2);
                    let bot = rgb.get_pixel(col as u32, row as u32 * 2 + 1);
                    Span::styled(
                        "▀",
                        Style::default()
                            .fg(Color::Rgb(top[0], top[1], top[2]))
                            .bg(Color::Rgb(bot[0], bot[1], bot[2])),
                    )
                })
                .collect();
            Line::from(spans)
        })
        .collect();

    f.render_widget(Paragraph::new(lines), area);
}

/// Build the Kitty graphics protocol escape sequence for `img` positioned at
/// `area`. Caller writes the returned bytes to stdout after `terminal.draw()`.
/// Kept as a separate build step so the result can be cached (#91) and reused
/// across frames without re-running `resize_exact`/base64 encode.
pub fn build_kitty(img: &DynamicImage, area: Rect, cell_w: u16, cell_h: u16) -> Vec<u8> {
    let pw = (area.width as u32) * (cell_w as u32);
    let ph = (area.height as u32) * (cell_h as u32);
    let resized = img.resize_exact(pw, ph, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let raw: &[u8] = rgba.as_raw();
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw);

    let chunk_size = 4096;
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(chunk_size)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();

    let mut out: Vec<u8> = Vec::with_capacity(encoded.len() + 64 * chunks.len() + 16);
    let _ = write!(out, "\x1b[{};{}H", area.y + 1, area.x + 1);
    for (i, chunk) in chunks.iter().enumerate() {
        let more = if i + 1 < chunks.len() { 1 } else { 0 };
        if i == 0 {
            let _ = write!(
                out,
                "\x1b_Ga=T,f=32,s={pw},v={ph},c={cols},r={rows},q=2,m={more};{chunk}\x1b\\",
                cols = area.width,
                rows = area.height,
            );
        } else {
            let _ = write!(out, "\x1b_Gm={more};{chunk}\x1b\\");
        }
    }
    out
}

/// Build the iTerm2 inline image escape sequence for `img` positioned at
/// `area`. See `build_kitty` for the rationale behind the split.
pub fn build_iterm2(img: &DynamicImage, area: Rect) -> Vec<u8> {
    let pw = area.width as u32 * 8;
    let ph = area.height as u32 * 16;
    let resized = img.resize_exact(pw, ph, image::imageops::FilterType::Lanczos3);

    let mut png_bytes: Vec<u8> = Vec::new();
    resized
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .unwrap_or(());

    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_bytes);
    let len = png_bytes.len();

    let mut out: Vec<u8> = Vec::with_capacity(encoded.len() + 128);
    let _ = write!(out, "\x1b[{};{}H", area.y + 1, area.x + 1);
    let _ = write!(
        out,
        "\x1b]1337;File=inline=1;width={pw}px;height={ph}px;size={len};preserveAspectRatio=0:{encoded}\x07",
    );
    out
}
