use anyhow::{Context, Result};
use std::fmt;

/// Tipos de erro para categorização
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    /// Erro de arquivo (leitura, escrita, permissão)
    File,
    /// Erro de formato (parsing, validação)
    Format,
    /// Erro de estado (estado inválido, inicialização)
    State,
    /// Erro de comando (execução, permissão)
    Command,
    /// Erro de rede (conexão, timeout)
    Network,
    /// Erro de API (resposta, formato)
    Api,
    /// Erro de sistema (ambiente, recursos)
    System,
    /// Erro de usuário (input inválido)
    User,
    /// Erro desconhecido ou não categorizado
    Unknown,
}

/// Trait para adicionar contexto localizado a erros
pub trait ErrorContextExt<T> {
    /// Adiciona contexto localizado ao erro
    fn with_localized_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> (&'static str, &'static str, &'static str);
}

impl<T, E> ErrorContextExt<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_localized_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> (&'static str, &'static str, &'static str),
    {
        self.with_context(|| {
            let (en, pt, es) = f();
            let current_lang = crate::core::i18n::get_language();
            match current_lang {
                crate::core::i18n::Language::English => en.to_string(),
                crate::core::i18n::Language::Portuguese => pt.to_string(),
                crate::core::i18n::Language::Spanish => es.to_string(),
            }
        })
    }
}

/// Estrutura para erros localizados
#[derive(Debug)]
pub struct LocalizedError {
    /// Mensagem em inglês
    pub en: String,
    /// Mensagem em português
    pub pt: String,
    /// Mensagem em espanhol
    pub es: String,
    /// Tipo de erro
    pub error_type: ErrorType,
}

impl LocalizedError {
    /// Cria um novo erro localizado
    pub fn new(
        en: impl Into<String>,
        pt: impl Into<String>,
        es: impl Into<String>,
        error_type: ErrorType,
    ) -> Self {
        Self {
            en: en.into(),
            pt: pt.into(),
            es: es.into(),
            error_type,
        }
    }

    /// Obtém a mensagem no idioma atual
    pub fn message(&self) -> String {
        let current_lang = crate::core::i18n::get_language();
        match current_lang {
            crate::core::i18n::Language::English => self.en.clone(),
            crate::core::i18n::Language::Portuguese => self.pt.clone(),
            crate::core::i18n::Language::Spanish => self.es.clone(),
        }
    }
}

impl fmt::Display for LocalizedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for LocalizedError {}

/// Cria um erro localizado como anyhow::Error
pub fn localized_error(
    en: impl Into<String>,
    pt: impl Into<String>,
    es: impl Into<String>,
) -> anyhow::Error {
    let error = LocalizedError::new(en, pt, es, ErrorType::Unknown);
    anyhow::Error::new(error)
}

/// Cria um erro de arquivo localizado
pub fn file_error(
    en: impl Into<String>,
    pt: impl Into<String>,
    es: impl Into<String>,
) -> anyhow::Error {
    let error = LocalizedError::new(en, pt, es, ErrorType::File);
    anyhow::Error::new(error)
}

/// Cria um erro de formato localizado
pub fn format_error(
    en: impl Into<String>,
    pt: impl Into<String>,
    es: impl Into<String>,
) -> anyhow::Error {
    let error = LocalizedError::new(en, pt, es, ErrorType::Format);
    anyhow::Error::new(error)
}

/// Cria um erro de estado localizado
pub fn state_error(
    en: impl Into<String>,
    pt: impl Into<String>,
    es: impl Into<String>,
) -> anyhow::Error {
    let error = LocalizedError::new(en, pt, es, ErrorType::State);
    anyhow::Error::new(error)
}

/// Cria um erro de comando localizado
pub fn command_error(
    en: impl Into<String>,
    pt: impl Into<String>,
    es: impl Into<String>,
) -> anyhow::Error {
    let error = LocalizedError::new(en, pt, es, ErrorType::Command);
    anyhow::Error::new(error)
}

/// Verifica se um erro é de um determinado tipo
pub fn is_error_type(error: &anyhow::Error, error_type: ErrorType) -> bool {
    error
        .downcast_ref::<LocalizedError>()
        .map(|e| e.error_type == error_type)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::i18n::{set_language, Language};

    #[test]
    fn test_localized_error() {
        // Test English
        set_language(Language::English);
        let error = localized_error("English error", "Erro em português", "Error en español");
        assert_eq!(error.to_string(), "English error");

        // Test Portuguese
        set_language(Language::Portuguese);
        let error = localized_error("English error", "Erro em português", "Error en español");
        assert_eq!(error.to_string(), "Erro em português");

        // Test Spanish
        set_language(Language::Spanish);
        let error = localized_error("English error", "Erro em português", "Error en español");
        assert_eq!(error.to_string(), "Error en español");
    }

    #[test]
    fn test_with_localized_context() {
        set_language(Language::English);

        let result: Result<(), _> =
            Err(anyhow::anyhow!("Base error")).with_localized_context(|| {
                (
                    "Error with context",
                    "Erro com contexto",
                    "Error con contexto",
                )
            });

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Error with context: Base error"
        );

        // Change language and test again
        set_language(Language::Portuguese);

        let result: Result<(), _> =
            Err(anyhow::anyhow!("Base error")).with_localized_context(|| {
                (
                    "Error with context",
                    "Erro com contexto",
                    "Error con contexto",
                )
            });

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Erro com contexto: Base error"
        );
    }
}
