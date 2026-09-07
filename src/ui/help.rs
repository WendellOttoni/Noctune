//! Help reads the exact same command registry and live key bindings as the palette.
use crate::{i18n::Language, keybinds::Bindings, theme::Theme};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_help(
    f: &mut Frame,
    area: Rect,
    scroll: u16,
    theme: &Theme,
    bindings: &Bindings,
    language: Language,
) {
    let popup = super::util::bounded_popup(area, 92, 30);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(language.text(
            " Help — commands and current shortcuts ",
            " Ajuda — comandos e atalhos atuais ",
        ))
        .title_bottom(language.text(
            " Up/Down: scroll | Esc: close ",
            " Cima/Baixo: rolar | Esc: fechar ",
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let lines: Vec<Line> = crate::commands::COMMANDS
        .iter()
        .flat_map(|command| {
            [
                Line::from(vec![Span::styled(
                    format!("{}  :{}", bindings.shortcut(command.action), command.id),
                    theme.accent(),
                )]),
                Line::from(command.title(language)),
            ]
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn help_fits_small_terminals_and_shows_overrides() {
        let config = crate::config::Keybinds {
            play_pause: "Ctrl+x".into(),
            ..crate::config::Config::default().keybinds
        };
        let (bindings, _) = Bindings::from_config(&config);
        let theme: Theme = toml::from_str(include_str!("../../themes/default.toml")).unwrap();
        for (width, height) in [(20, 8), (80, 24), (120, 30)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|f| render_help(f, f.area(), 0, &theme, &bindings, Language::En))
                .unwrap();
            assert_eq!(terminal.backend().buffer().area.width, width);
            assert!(terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.symbol() == "H"));
        }
        assert!(crate::commands::markdown(&bindings, Language::En).contains("Ctrl+x"));
    }
}
