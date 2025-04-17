use tracing::info;
use super::language::{Language, get_language};

/// Registra o mapeamento de descrições de ferramentas para o framework MCP
pub fn register_tool_descriptions() {
    // This function will be called when the server initializes
    // It can be extended in the future to register mappings or hooks
    // for tool descriptions in the MCP framework if needed
    info!("Registered tool description localization support");
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
    
    /// Obtém a descrição no idioma atual
    pub fn current(&self) -> &'static str {
        self.get(get_language())
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
