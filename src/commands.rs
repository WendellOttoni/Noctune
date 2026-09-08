//! Single registry shared by the palette, help and exported shortcut documentation.
use crate::{
    i18n::Language,
    keybinds::{Action, Bindings},
};
pub struct Command {
    pub id: &'static str,
    pub en: &'static str,
    pub pt: &'static str,
    pub action: Action,
}
impl Command {
    pub fn title(&self, language: Language) -> &'static str {
        language.text(self.en, self.pt)
    }
}
pub const COMMANDS: &[Command] = &[
    Command {
        id: "language",
        en: "Switch English / Portuguese",
        pt: "Alternar português / inglês",
        action: Action::ToggleLanguage,
    },
    Command {
        id: "symbols",
        en: "Toggle simple symbols",
        pt: "Alternar símbolos simples",
        action: Action::ToggleSymbols,
    },
    Command {
        id: "quit",
        en: "Quit",
        pt: "Sair",
        action: Action::Quit,
    },
    Command {
        id: "help",
        en: "Help and shortcuts",
        pt: "Ajuda e atalhos",
        action: Action::Help,
    },
    Command {
        id: "search",
        en: "Search library",
        pt: "Buscar na biblioteca",
        action: Action::Search,
    },
    Command {
        id: "focus",
        en: "Switch focus",
        pt: "Alternar foco",
        action: Action::Tab,
    },
    Command {
        id: "play",
        en: "Play / pause",
        pt: "Tocar / pausar",
        action: Action::PlayPause,
    },
    Command {
        id: "next",
        en: "Next track",
        pt: "Próxima faixa",
        action: Action::Next,
    },
    Command {
        id: "prev",
        en: "Previous track",
        pt: "Faixa anterior",
        action: Action::Prev,
    },
    Command {
        id: "stop",
        en: "Stop playback",
        pt: "Parar reprodução",
        action: Action::Stop,
    },
    Command {
        id: "shuffle",
        en: "Toggle shuffle",
        pt: "Alternar ordem aleatória",
        action: Action::Shuffle,
    },
    Command {
        id: "repeat",
        en: "Cycle repeat mode",
        pt: "Alternar repetição",
        action: Action::Repeat,
    },
    Command {
        id: "sort",
        en: "Sort library",
        pt: "Ordenar biblioteca",
        action: Action::Sort,
    },
    Command {
        id: "sleep",
        en: "30-minute sleep timer",
        pt: "Temporizador de 30 minutos",
        action: Action::SleepTimer,
    },
    Command {
        id: "save-playlist",
        en: "Save queue as playlist",
        pt: "Salvar fila como playlist",
        action: Action::SavePlaylist,
    },
    Command {
        id: "playlists",
        en: "Open saved playlists",
        pt: "Abrir playlists salvas",
        action: Action::LoadPlaylist,
    },
    Command {
        id: "volume-up",
        en: "Increase volume",
        pt: "Aumentar volume",
        action: Action::VolumeUp,
    },
    Command {
        id: "volume-down",
        en: "Decrease volume",
        pt: "Diminuir volume",
        action: Action::VolumeDown,
    },
    Command {
        id: "seek-back",
        en: "Seek back five seconds",
        pt: "Voltar cinco segundos",
        action: Action::SeekBack,
    },
    Command {
        id: "seek-forward",
        en: "Seek forward five seconds",
        pt: "Avançar cinco segundos",
        action: Action::SeekForward,
    },
    Command {
        id: "up",
        en: "Move selection up",
        pt: "Mover seleção para cima",
        action: Action::SelectionUp,
    },
    Command {
        id: "down",
        en: "Move selection down",
        pt: "Mover seleção para baixo",
        action: Action::SelectionDown,
    },
    Command {
        id: "open",
        en: "Play selection / open folder",
        pt: "Tocar seleção / abrir pasta",
        action: Action::ActivateSelection,
    },
    Command {
        id: "enqueue",
        en: "Add selection to queue",
        pt: "Adicionar seleção à fila",
        action: Action::Enqueue,
    },
    Command {
        id: "remove",
        en: "Remove selected queue entry",
        pt: "Remover seleção da fila",
        action: Action::RemoveQueueItem,
    },
    Command {
        id: "clear",
        en: "Clear queue",
        pt: "Limpar fila",
        action: Action::ClearQueue,
    },
    Command {
        id: "spotify-login",
        en: "Sign in to Spotify",
        pt: "Entrar no Spotify",
        action: Action::SpotifyLogin,
    },
    Command {
        id: "spotify-toggle",
        en: "Spotify Connect play / pause",
        pt: "Spotify Connect: tocar / pausar",
        action: Action::SpotifyToggle,
    },
    Command {
        id: "view",
        en: "Cycle library view",
        pt: "Alternar visão da biblioteca",
        action: Action::ToggleView,
    },
    Command {
        id: "eq-low-up",
        en: "Increase bass",
        pt: "Aumentar graves",
        action: Action::EqLowUp,
    },
    Command {
        id: "eq-low-down",
        en: "Decrease bass",
        pt: "Diminuir graves",
        action: Action::EqLowDown,
    },
    Command {
        id: "eq-mid-up",
        en: "Increase mids",
        pt: "Aumentar médios",
        action: Action::EqMidUp,
    },
    Command {
        id: "eq-mid-down",
        en: "Decrease mids",
        pt: "Diminuir médios",
        action: Action::EqMidDown,
    },
    Command {
        id: "eq-high-up",
        en: "Increase treble",
        pt: "Aumentar agudos",
        action: Action::EqHighUp,
    },
    Command {
        id: "eq-high-down",
        en: "Decrease treble",
        pt: "Diminuir agudos",
        action: Action::EqHighDown,
    },
    Command {
        id: "url",
        en: "Import URL or playlist",
        pt: "Importar URL ou playlist",
        action: Action::OpenUrl,
    },
    Command {
        id: "eq-preset",
        en: "Cycle EQ preset",
        pt: "Alternar preset do equalizador",
        action: Action::EqPreset,
    },
    Command {
        id: "rescan",
        en: "Rescan music folders",
        pt: "Reescanear pastas de música",
        action: Action::Rescan,
    },
    Command {
        id: "info",
        en: "Track information",
        pt: "Informações da faixa",
        action: Action::TrackInfo,
    },
    Command {
        id: "theme",
        en: "Cycle theme",
        pt: "Alternar tema",
        action: Action::CycleTheme,
    },
    Command {
        id: "viz-up",
        en: "Increase visualizer sensitivity",
        pt: "Aumentar sensibilidade visual",
        action: Action::VizSensUp,
    },
    Command {
        id: "viz-down",
        en: "Decrease visualizer sensitivity",
        pt: "Diminuir sensibilidade visual",
        action: Action::VizSensDown,
    },
    Command {
        id: "undo",
        en: "Undo queue change",
        pt: "Desfazer alteração na fila",
        action: Action::UndoQueue,
    },
    Command {
        id: "history",
        en: "Recently played",
        pt: "Tocadas recentemente",
        action: Action::RecentlyPlayed,
    },
    Command {
        id: "audio",
        en: "Audio controls",
        pt: "Controles de áudio",
        action: Action::ShowAudioPanel,
    },
    Command {
        id: "replaygain",
        en: "Cycle ReplayGain",
        pt: "Alternar ReplayGain",
        action: Action::ReplayGain,
    },
    Command {
        id: "viz",
        en: "Cycle visualizer",
        pt: "Alternar visualizador",
        action: Action::CycleVizMode,
    },
    Command {
        id: "fav",
        en: "Toggle favorite",
        pt: "Alternar favorito",
        action: Action::ToggleFavorite,
    },
    Command {
        id: "mini",
        en: "Toggle compact player",
        pt: "Alternar player compacto",
        action: Action::ToggleMini,
    },
    Command {
        id: "lastfm-login",
        en: "Sign in to Last.fm",
        pt: "Entrar no Last.fm",
        action: Action::LastfmLogin,
    },
    Command {
        id: "device",
        en: "Choose audio output",
        pt: "Escolher saída de áudio",
        action: Action::SelectDevice,
    },
    Command {
        id: "eq",
        en: "Graphic equalizer",
        pt: "Equalizador gráfico",
        action: Action::EqTuner,
    },
    Command {
        id: "profiles",
        en: "Configuration profiles",
        pt: "Perfis de configuração",
        action: Action::Profiles,
    },
    Command {
        id: "spotify",
        en: "Browse Spotify (Connect required)",
        pt: "Explorar Spotify (requer Connect)",
        action: Action::SpotifyBrowser,
    },
    Command {
        id: "radio-mode",
        en: "Radio recommendations",
        pt: "Recomendações de rádio",
        action: Action::RadioMode,
    },
    Command {
        id: "stats",
        en: "Listening statistics",
        pt: "Estatísticas de audição",
        action: Action::ShowStats,
    },
    Command {
        id: "lastfm",
        en: "Last.fm dashboard",
        pt: "Painel Last.fm",
        action: Action::LastfmPanel,
    },
    Command {
        id: "tags",
        en: "Edit track metadata",
        pt: "Editar metadados",
        action: Action::EditTags,
    },
    Command {
        id: "radio",
        en: "Online radio directory",
        pt: "Diretório de rádios online",
        action: Action::RadioBrowser,
    },
    Command {
        id: "update",
        en: "Check for updates",
        pt: "Verificar atualizações",
        action: Action::SelfUpdate,
    },
    Command {
        id: "library",
        en: "Show library",
        pt: "Mostrar biblioteca",
        action: Action::ViewLibrary,
    },
    Command {
        id: "queue",
        en: "Focus queue",
        pt: "Focar fila",
        action: Action::ViewQueue,
    },
    Command {
        id: "view-radio",
        en: "Show radio view",
        pt: "Mostrar rádios",
        action: Action::ViewRadio,
    },
    Command {
        id: "files",
        en: "Browse folders",
        pt: "Explorar pastas",
        action: Action::ViewBrowser,
    },
    Command {
        id: "lyrics",
        en: "Synchronized lyrics",
        pt: "Letras sincronizadas",
        action: Action::ShowLyrics,
    },
    Command {
        id: "commands",
        en: "Command palette",
        pt: "Paleta de comandos",
        action: Action::CommandPalette,
    },
    Command {
        id: "subsonic",
        en: "Subsonic / Navidrome",
        pt: "Subsonic / Navidrome",
        action: Action::SubsonicBrowser,
    },
    Command {
        id: "vault",
        en: "Cloud Vault (experimental)",
        pt: "Cloud Vault (experimental)",
        action: Action::VaultBrowser,
    },
    Command {
        id: "share",
        en: "Publish playlist (external service)",
        pt: "Publicar playlist (serviço externo)",
        action: Action::SharePlaylist,
    },
    Command {
        id: "browse",
        en: "Browse public playlists",
        pt: "Explorar playlists públicas",
        action: Action::BrowsePlaylists,
    },
    Command {
        id: "endless",
        en: "Toggle endless autoplay",
        pt: "Alternar reprodução infinita",
        action: Action::ToggleEndlessMode,
    },
    Command {
        id: "download",
        en: "Download current stream",
        pt: "Baixar stream atual",
        action: Action::DownloadCurrent,
    },
    Command {
        id: "cache",
        en: "Audio cache usage",
        pt: "Uso do cache de áudio",
        action: Action::CacheStatus,
    },
    Command {
        id: "cache-clear",
        en: "Clear unused audio cache",
        pt: "Limpar cache de áudio não utilizado",
        action: Action::ClearAudioCache,
    },
];
pub fn markdown(bindings: &Bindings, language: Language) -> String {
    let mut out = String::from(language.text("# Noctune shortcuts\n\nGenerated by `noctune commands`. Custom bindings are reflected in the in-app help.\n\n| Command | Action | Shortcut |\n|---|---|---|\n", "# Atalhos do Noctune\n\nGerado por `noctune commands`. Os atalhos personalizados aparecem na ajuda do app.\n\n| Comando | Ação | Atalho |\n|---|---|---|\n"));
    for command in COMMANDS {
        out.push_str(&format!(
            "| `:{}` | {} | `{}` |\n",
            command.id,
            command.title(language),
            bindings.shortcut(command.action)
        ));
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_has_unique_actions_and_ids() {
        let mut ids = std::collections::HashSet::new();
        let mut actions = std::collections::HashSet::new();
        for command in COMMANDS {
            assert!(ids.insert(command.id));
            assert!(actions.insert(command.action));
        }
    }
}
