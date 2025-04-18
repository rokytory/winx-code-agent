use std::sync::atomic::{AtomicU8, Ordering};
use tracing::info;

/// Inicializa o suporte a idiomas
pub fn init_language_support() {
    info!("Initializing language support: EN, PT, ES");

    // Define o idioma padrão para inglês
    set_language(Language::English);

    info!("Default language set to: {}", get_language().native_name());
}

/// Enumeração de idiomas suportados
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Inglês (padrão)
    English = 0,
    /// Português
    Portuguese = 1,
    /// Espanhol
    Spanish = 2,
}

impl Language {
    /// Converte código de idioma para enum Language
    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "pt" | "pt-br" | "pt-pt" => Language::Portuguese,
            "es" | "es-es" | "es-mx" | "es-ar" => Language::Spanish,
            _ => Language::English, // Inglês é o padrão
        }
    }

    /// Obtém o código do idioma
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Portuguese => "pt",
            Language::Spanish => "es",
        }
    }

    /// Obtém o nome do idioma em inglês
    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Portuguese => "Portuguese",
            Language::Spanish => "Spanish",
        }
    }

    /// Obtém o nome nativo do idioma
    pub fn native_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Portuguese => "Português",
            Language::Spanish => "Español",
        }
    }

    /// Converte u8 para Language
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Language::Portuguese,
            2 => Language::Spanish,
            _ => Language::English,
        }
    }
}

// Variável global para armazenar o idioma atual
static CURRENT_LANGUAGE: AtomicU8 = AtomicU8::new(Language::English as u8);

/// Define o idioma atual para todas as ferramentas
pub fn set_language(lang: Language) {
    let old_lang = Language::from_u8(CURRENT_LANGUAGE.load(Ordering::SeqCst));

    CURRENT_LANGUAGE.store(lang as u8, Ordering::SeqCst);

    info!(
        "Language changed from {} to {}",
        old_lang.name(),
        lang.name()
    );
}

/// Obtém o idioma atual
pub fn get_language() -> Language {
    Language::from_u8(CURRENT_LANGUAGE.load(Ordering::SeqCst))
}
