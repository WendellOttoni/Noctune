//! Visualizer renderers (#100). Extracted from `ui.rs` — pure code move.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{app::App, theme::parse_color};

pub fn render_visualizer(f: &mut Frame, area: Rect, app: &App) {
    let mode_label = app.viz_mode.label();
    let title = format!(" {} (×{:.1}) [v] ", mode_label, app.tap.sensitivity());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border(false))
        .title(Span::styled(title, app.theme.accent()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    match app.viz_mode {
        crate::app::VizMode::Spectrum => render_viz_spectrum(f, inner, app),
        crate::app::VizMode::Waveform => render_viz_waveform(f, inner, app),
        crate::app::VizMode::VuMeter => render_viz_vu(f, inner, app),
    }
}

fn render_viz_spectrum(f: &mut Frame, inner: Rect, app: &App) {
    let bar_w: u16 = 2;
    let gap: u16 = 1;
    let stride = bar_w + gap;
    let n_bars = (inner.width / stride).max(1) as usize;
    let bars = app.tap.compute_bars(n_bars);

    let blocks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let h = inner.height as usize;
    let primary = parse_color(&app.theme.colors.primary);
    let secondary = parse_color(&app.theme.colors.secondary);
    let accent = parse_color(&app.theme.colors.accent);

    for row in 0..h {
        let mut spans: Vec<Span> = Vec::with_capacity(n_bars * 2);
        let row_from_bottom = h - 1 - row;
        for (i, &val) in bars.iter().enumerate() {
            let bar_units = (val * (h * blocks.len()) as f32) as usize;
            let full_rows = bar_units / blocks.len();
            let rem = bar_units % blocks.len();
            let ch = if row_from_bottom < full_rows {
                '█'
            } else if row_from_bottom == full_rows && rem > 0 {
                blocks[rem.saturating_sub(1)]
            } else {
                ' '
            };
            let color = if val > 0.75 {
                accent
            } else if val > 0.4 {
                primary
            } else {
                secondary
            };
            let bar_text: String = std::iter::repeat_n(ch, bar_w as usize).collect();
            spans.push(Span::styled(bar_text, Style::default().fg(color)));
            if i + 1 < n_bars {
                spans.push(Span::raw(" ".repeat(gap as usize)));
            }
        }
        let r = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }
}

fn render_viz_waveform(f: &mut Frame, inner: Rect, app: &App) {
    let w = inner.width as usize;
    let h = inner.height as usize;
    if w == 0 || h < 3 {
        return;
    }

    let (samples, _) = app.tap.waveform_data(w);
    let mid = h / 2;

    let bg = parse_color(&app.theme.colors.background);
    let accent = parse_color(&app.theme.colors.accent);
    let primary = parse_color(&app.theme.colors.primary);
    let secondary = parse_color(&app.theme.colors.secondary);
    let muted = parse_color(&app.theme.colors.muted);

    // sub[k]: fills (k+1)/8 from BOTTOM with foreground color.
    // Inverted (fg=bg, bg=fill): fills (7-k)/8 from TOP with fill color.
    let sub: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    // Virtual pixel grid: h*8 rows total. s=+1 → pixel 0 (top), s=-1 → pixel h*8-1 (bottom).
    let half_px = (h * 4) as f32;

    struct Col {
        trace_row: usize,
        trace_sub: usize,
        fill_lo: usize,
        fill_hi: usize,
        above: bool,
        below: bool,
    }

    let cols: Vec<Col> = (0..w)
        .map(|c| {
            let s = samples[c].clamp(-1.0, 1.0);
            let px = ((half_px - s * half_px).max(0.0).round() as usize).min(h * 8 - 1);
            let trace_row = px / 8;
            let trace_sub = px % 8;
            let above = trace_row < mid;
            let below = trace_row > mid;

            let (fill_lo, fill_hi) = if above {
                (trace_row + 1, mid + 1)
            } else if below {
                (mid, trace_row)
            } else {
                (0, 0)
            };

            Col {
                trace_row,
                trace_sub,
                fill_lo,
                fill_hi,
                above,
                below,
            }
        })
        .collect();

    for row in 0..h {
        let mut spans = Vec::with_capacity(w);
        for col in 0..w {
            let c = &cols[col];

            let span = if row == c.trace_row {
                if c.above {
                    Span::styled(
                        sub[7 - c.trace_sub].to_string(),
                        Style::default().fg(accent),
                    )
                } else if c.below {
                    if c.trace_sub == 7 {
                        Span::styled('█'.to_string(), Style::default().fg(accent))
                    } else {
                        Span::styled(
                            sub[6 - c.trace_sub].to_string(),
                            Style::default().fg(bg).bg(accent),
                        )
                    }
                } else {
                    let ch = if col % 3 == 0 { '·' } else { ' ' };
                    Span::styled(ch.to_string(), Style::default().fg(muted))
                }
            } else if row >= c.fill_lo && row < c.fill_hi {
                let dist = (row as i32 - mid as i32).unsigned_abs() as f32;
                let max_dist = (c.trace_row as i32 - mid as i32).unsigned_abs() as f32;
                let t = if max_dist > 0.0 { dist / max_dist } else { 0.0 };
                let color = if t > 0.55 { primary } else { secondary };
                Span::styled('█'.to_string(), Style::default().fg(color))
            } else if row == mid {
                let ch = if col % 3 == 0 { '·' } else { ' ' };
                Span::styled(ch.to_string(), Style::default().fg(muted))
            } else {
                Span::styled(' '.to_string(), Style::default())
            };

            spans.push(span);
        }
        let r = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), r);
    }
}

fn render_viz_vu(f: &mut Frame, inner: Rect, app: &App) {
    let rms = app.tap.rms_level();
    let peak_bars = app.tap.compute_bars(1);
    let peak = peak_bars.first().copied().unwrap_or(0.0);

    let w = inner.width as usize;
    let accent = parse_color(&app.theme.colors.accent);
    let primary = parse_color(&app.theme.colors.primary);
    let secondary = parse_color(&app.theme.colors.secondary);
    let muted = parse_color(&app.theme.colors.muted);

    let bar_row =
        |f: &mut Frame, y: u16, label: &str, level: f32, color_hi: ratatui::style::Color| {
            let label_w = 5usize;
            let bar_w = w.saturating_sub(label_w + 2);
            let filled = (level.clamp(0.0, 1.0) * bar_w as f32).round() as usize;
            let empty = bar_w - filled;
            let spans = vec![
                Span::styled(format!("{:>5} ", label), Style::default().fg(muted)),
                Span::styled("█".repeat(filled), Style::default().fg(color_hi)),
                Span::styled("░".repeat(empty), Style::default().fg(secondary)),
                Span::styled(
                    format!(" {:3.0}%", level * 100.0),
                    Style::default().fg(muted),
                ),
            ];
            let r = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(Line::from(spans)), r);
        };

    let mid = inner.y + inner.height / 2;
    bar_row(
        f,
        mid.saturating_sub(1),
        "RMS",
        rms,
        if rms > 0.8 { accent } else { primary },
    );
    bar_row(
        f,
        mid,
        "Peak",
        peak,
        if peak > 0.75 { accent } else { primary },
    );
}
