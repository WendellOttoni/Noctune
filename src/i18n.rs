//! Shared language catalog for navigation, commands, setup and diagnostics.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    En,
    #[serde(rename = "pt-BR", alias = "pt-br")]
    PtBr,
}
impl Language {
    /// Read only the selected language; CLI help must not create configuration
    /// or consume first-run setup by loading and saving a default config.
    pub fn configured() -> Self {
        crate::config::config_path()
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| toml::from_str::<crate::config::Config>(&text).ok())
            .map(|config| config.ui.language)
            .unwrap_or_default()
    }
    pub fn text(self, en: &'static str, pt: &'static str) -> &'static str {
        match self {
            Self::En => en,
            Self::PtBr => pt,
        }
    }
}

/// Keep format strings literal so Rust checks both translations and their arguments.
#[macro_export]
macro_rules! localized_format {
    ($language:expr, $en:literal, $pt:literal $(, $arg:expr)* $(,)?) => {
        match $language {
            $crate::i18n::Language::En => format!($en $(, $arg)*),
            $crate::i18n::Language::PtBr => format!($pt $(, $arg)*),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn formats_preserve_user_content_and_numeric_arguments() {
        let title = "Minha música / My song {live}";
        for (language, expected) in [
            (Language::En, "Playing Minha música / My song {live}: 1.25×"),
            (
                Language::PtBr,
                "Tocando Minha música / My song {live}: 1.25×",
            ),
        ] {
            assert_eq!(
                crate::localized_format!(
                    language,
                    "Playing {title}: {:.2}×",
                    "Tocando {title}: {:.2}×",
                    1.25
                ),
                expected
            );
        }
    }

    #[test]
    fn language_config_round_trips_and_defaults_to_english() {
        assert_eq!(crate::config::Config::default().ui.language, Language::En);
        for code in ["en", "pt-BR", "pt-br"] {
            let config: crate::config::UiConfig =
                toml::from_str(&format!("language = \"{code}\"\n")).unwrap();
            let expected = if code == "en" {
                Language::En
            } else {
                Language::PtBr
            };
            assert_eq!(config.language, expected);
            let saved = toml::to_string(&config).unwrap();
            assert_eq!(
                toml::from_str::<crate::config::UiConfig>(&saved)
                    .unwrap()
                    .language,
                expected
            );
        }
    }
}
