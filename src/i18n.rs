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
    pub fn text(self, en: &'static str, pt: &'static str) -> &'static str {
        match self {
            Self::En => en,
            Self::PtBr => pt,
        }
    }
}
