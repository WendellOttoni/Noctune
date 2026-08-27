//! Help overlay (#100). Redesigned with 2-column card layout, key badges and categorized sections.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme::{parse_color, Theme};

#[derive(Clone, Copy)]
struct ShortcutSection {
    icon: &'static str,
    title: &'static str,
    shortcuts: &'static [(&'static str, &'static str)],
}

const LEFT_SECTIONS: &[ShortcutSection] = &[
    ShortcutSection {
        icon: "▶",
        title: "Playback & Controles",
        shortcuts: &[
            ("Space", "Tocar / Pausar reprodução"),
            ("n / p", "Próxima / Faixa anterior"),
            ("s", "Parar áudio e descarregar"),
            ("← / →", "Buscar -5s / +5s na faixa"),
            ("+ / -", "Aumentar / Diminuir volume"),
            ("m", "Alternar Mini Player"),
            ("Ctrl+P / :", "Paleta de Comandos unificada"),
        ],
    },
    ShortcutSection {
        icon: "📚",
        title: "Biblioteca & Fila",
        shortcuts: &[
            ("Tab", "Alternar foco (Biblioteca / Fila)"),
            ("↑↓ / jk", "Mover cursor de seleção"),
            ("Enter", "Reproduzir faixa / abrir pasta"),
            ("a / d", "Adicionar / Remover da fila"),
            ("c / u", "Limpar fila / Desfazer ação"),
            ("f", "Favoritar / Desfavoritar (♥)"),
            ("/", "Busca rápida (SQLite FTS5)"),
        ],
    },
    ShortcutSection {
        icon: "👁",
        title: "Visões & Navegação",
        shortcuts: &[
            ("1", "Visão Flat (Todas as músicas)"),
            ("2", "Foco na Fila de reprodução"),
            ("3", "Hub de Rádios Online"),
            ("4", "Explorador de Pastas do disco"),
            ("V", "Ciclar modos de visualização"),
            ("H", "Histórico de tocadas recentes"),
            ("Shift+Tab", "Alternar temas visuais"),
        ],
    },
];

const RIGHT_SECTIONS: &[ShortcutSection] = &[
    ShortcutSection {
        icon: "🎚",
        title: "Áudio, EQ & Visualizador",
        shortcuts: &[
            ("e", "Painel de Áudio & EQ 10-Bandas"),
            ("0 / Shift+E", "Ciclar presets de EQ (Rock, Bass...)"),
            ("v", "Ciclar visualizador FFT (Spectrum, VU...)"),
            ("[ / ]", "Ajustar sensibilidade do VU"),
            ("y", "Letras sincronizadas (Karaokê LRC)"),
            ("G", "ReplayGain (Off / Faixa / Álbum)"),
            ("D", "Selecionar saída de áudio"),
        ],
    },
    ShortcutSection {
        icon: "☁",
        title: "Streaming & Nuvem",
        shortcuts: &[
            ("3 / K", "Diretório de rádios (+40k estações)"),
            ("Ctrl+N", "Subsonic / Navidrome Cloud"),
            ("i", "Prompt de URL: YouTube, SoundCloud..."),
            ("P / @", "Spotify Browser & Playback nativo"),
            ("F", "Integração & Scrobbling Last.fm"),
        ],
    },
    ShortcutSection {
        icon: "📁",
        title: "Playlists & Sistema",
        shortcuts: &[
            ("w", "Salvar fila como playlist (.m3u)"),
            ("L", "Carregar playlists locais"),
            ("X", "Publicar playlist na comunidade"),
            ("C", "Explorar & importar playlists públicas"),
            ("T", "Editor de tags ID3 e metadados"),
            ("R", "Reescanear diretórios de música"),
            ("U / Shift+U", "Verificar atualizações no GitHub"),
            ("q / Esc", "Fechar este modal de ajuda"),
        ],
    },
];

fn render_sections(sections: &[ShortcutSection], theme: &Theme) -> Vec<Line<'static>> {
    let fg = parse_color(&theme.colors.foreground);
    let secondary = parse_color(&theme.colors.secondary);
    let accent = parse_color(&theme.colors.accent);
    let muted = parse_color(&theme.colors.muted);

    let sec_header_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let key_badge_style = Style::default().fg(secondary).add_modifier(Modifier::BOLD);
    let sep_style = Style::default().fg(muted);
    let text_style = Style::default().fg(fg);

    let mut lines = Vec::new();

    for (i, sec) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", sec.icon), sec_header_style),
            Span::styled(sec.title, sec_header_style),
        ]));

        for &(key, desc) in sec.shortcuts {
            lines.push(Line::from(vec![
                Span::styled(format!("   {:<12} ", key), key_badge_style),
                Span::styled("─ ", sep_style),
                Span::styled(desc, text_style),
            ]));
        }
    }

    lines
}

pub fn render_help(f: &mut Frame, area: Rect, scroll: u16, theme: &Theme) {
    let w = 86.min(area.width.saturating_sub(2)).max(40);
    let h = 32.min(area.height.saturating_sub(2)).max(12);

    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);

    let muted = parse_color(&theme.colors.muted);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(Span::styled(
            " 📖 Noctune — Guia de Atalhos & Comandos ",
            theme.accent(),
        ))
        .title_bottom(Line::from(vec![
            Span::styled(" [↑/↓/j/k] ", theme.accent()),
            Span::styled("Rolar  ", Style::default().fg(muted)),
            Span::styled(" [Esc/?/q] ", theme.accent()),
            Span::styled("Fechar ", Style::default().fg(muted)),
        ]));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if inner.width >= 72 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(inner);

        let left_lines = render_sections(LEFT_SECTIONS, theme);
        let right_lines = render_sections(RIGHT_SECTIONS, theme);

        let left_p = Paragraph::new(left_lines).scroll((scroll, 0));
        let right_p = Paragraph::new(right_lines).scroll((scroll, 0));

        f.render_widget(left_p, cols[0]);

        // Subtle vertical divider in the middle
        let mut divider_spans = Vec::new();
        for _ in 0..cols[1].height {
            divider_spans.push(Line::from(Span::styled("│", Style::default().fg(muted))));
        }
        f.render_widget(Paragraph::new(divider_spans), cols[1]);

        f.render_widget(right_p, cols[2]);
    } else {
        // Fallback for narrow terminals: single combined column
        let mut all_sections = Vec::new();
        all_sections.extend_from_slice(LEFT_SECTIONS);
        all_sections.extend_from_slice(RIGHT_SECTIONS);

        let lines = render_sections(&all_sections, theme);
        let p = Paragraph::new(lines).scroll((scroll, 0));
        f.render_widget(p, inner);
    }
}
