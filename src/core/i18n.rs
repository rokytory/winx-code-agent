use std::sync::atomic::{AtomicU8, Ordering};
use tracing::info;

/// Inicializa o suporte a idiomas
pub fn init_language_support() {
    info!("Initializing language support: EN, PT, ES");
    
    // Define o idioma padrão para inglês
    set_language(Language::English);
    
    info!("Default language set to: {}", get_language().native_name());
    
    // Register tool description mapping for the MCP framework
    register_tool_descriptions();
}

/// Registra o mapeamento de descrições de ferramentas para o framework MCP
fn register_tool_descriptions() {
    // This function will be called when the server initializes
    // It can be extended in the future to register mappings or hooks
    // for tool descriptions in the MCP framework if needed
    info!("Registered tool description localization support");
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
}

/// Descrição localizada para uma ferramenta
#[derive(Debug, Clone)]
pub struct LocalizedDescription {
    /// Descrição em inglês
    pub en: &'static str,
    /// Descrição em português
    pub pt: &'static str,
    /// Descrição em espanhol
    pub es: &'static str,
}

impl LocalizedDescription {
    /// Cria uma nova descrição localizada
    pub fn new(en: &'static str, pt: &'static str, es: &'static str) -> Self {
        Self { en, pt, es }
    }

    /// Obtém a descrição no idioma especificado
    pub fn get(&self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.en,
            Language::Portuguese => self.pt,
            Language::Spanish => self.es,
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

impl Language {
    /// Converte u8 para Language
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Language::Portuguese,
            2 => Language::Spanish,
            _ => Language::English,
        }
    }
}

/// Macro para criar descrições localizadas facilmente
#[macro_export]
macro_rules! localized {
    ($en:expr, $pt:expr, $es:expr) => {
        $crate::core::i18n::LocalizedDescription::new($en, $pt, $es)
    };
}

/// Obtém a descrição localizada no idioma atual
pub fn get_localized(desc: &LocalizedDescription) -> &'static str {
    desc.get(get_language())
}
