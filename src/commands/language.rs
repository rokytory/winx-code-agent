use crate::core::i18n::{get_language, set_language, Language};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Estrutura para alternar o idioma da interface
#[derive(Debug, Serialize, Deserialize)]
pub struct LanguageRequest {
    /// Código do idioma (en, pt, es)
    pub language_code: String,
}

/// Lista os idiomas disponíveis e o idioma atual
pub fn list_available_languages() -> Result<String> {
    let current = get_language();

    let response = format!(
        "Available languages:
- English (en)
- Portuguese (pt)
- Spanish (es)

Current language: {} ({})",
        current.native_name(),
        current.code()
    );

    Ok(response)
}

/// Altera o idioma atual da interface
pub fn change_language(json_request: &str) -> Result<String> {
    let request: LanguageRequest = serde_json::from_str(json_request)?;
    let language_code = request.language_code.trim().to_lowercase();

    let new_language = match language_code.as_str() {
        "en" => Language::English,
        "pt" => Language::Portuguese,
        "es" => Language::Spanish,
        _ => {
            return Ok(format!(
                "Invalid language code: {}. Available codes: en, pt, es",
                language_code
            ));
        }
    };

    let old_language = get_language();
    set_language(new_language);

    info!(
        "Language changed from {} ({}) to {} ({})",
        old_language.native_name(),
        old_language.code(),
        new_language.native_name(),
        new_language.code()
    );

    let messages = match new_language {
        Language::English => "Language changed to English successfully.",
        Language::Portuguese => "Idioma alterado para Português com sucesso.",
        Language::Spanish => "Idioma cambiado a Español con éxito.",
    };

    Ok(messages.to_string())
}
