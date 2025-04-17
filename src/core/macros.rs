/// Macro para obter strings internacionalizadas
///
/// # Exemplos
///
/// ```
/// use winx::t;
///
/// let message = t!(
///     "File not found",
///     "Arquivo não encontrado",
///     "Archivo no encontrado"
/// );
///
/// // O idioma atual determina qual string será retornada
/// assert_eq!(message, "File not found"); // Se o idioma atual for inglês
/// ```
#[macro_export]
macro_rules! t {
    ($en:expr, $pt:expr, $es:expr) => {{
        let current_lang = $crate::core::i18n::get_language();
        match current_lang {
            $crate::core::i18n::Language::English => $en,
            $crate::core::i18n::Language::Portuguese => $pt,
            $crate::core::i18n::Language::Spanish => $es,
        }
    }};
}

/// Macro para criar uma descrição localizada e obter o texto no idioma atual
///
/// # Exemplos
///
/// ```
/// use winx::localized_text;
///
/// let message = localized_text!(
///     "File not found",
///     "Arquivo não encontrado",
///     "Archivo no encontrado"
/// );
///
/// // O idioma atual determina qual string será retornada
/// assert_eq!(message, "File not found"); // Se o idioma atual for inglês
/// ```
#[macro_export]
macro_rules! localized_text {
    ($en:expr, $pt:expr, $es:expr) => {{
        $crate::core::i18n::get_localized(&$crate::localized!($en, $pt, $es))
    }};
}

/// Macro para formatar strings com suporte a internacionalização
///
/// # Exemplos
///
/// ```
/// use winx::t_format;
///
/// let filename = "example.txt";
/// let message = t_format!(
///     "File {} not found",
///     "Arquivo {} não encontrado",
///     "Archivo {} no encontrado",
///     filename
/// );
///
/// // O idioma atual determina qual string será retornada
/// assert_eq!(message, "File example.txt not found"); // Se o idioma atual for inglês
/// ```
#[macro_export]
macro_rules! t_format {
    ($en:expr, $pt:expr, $es:expr, $($arg:tt)*) => {{
        let fmt_string = $crate::t!($en, $pt, $es);
        format!(fmt_string, $($arg)*)
    }};
}

#[cfg(test)]
mod tests {
    use crate::core::i18n::{Language, set_language};

    #[test]
    fn test_t_macro() {
        // Test English
        set_language(Language::English);
        let message = crate::t!("Hello", "Olá", "Hola");
        assert_eq!(message, "Hello");

        // Test Portuguese
        set_language(Language::Portuguese);
        let message = crate::t!("Hello", "Olá", "Hola");
        assert_eq!(message, "Olá");

        // Test Spanish
        set_language(Language::Spanish);
        let message = crate::t!("Hello", "Olá", "Hola");
        assert_eq!(message, "Hola");
    }

    #[test]
    fn test_t_format_macro() {
        // Test English
        set_language(Language::English);
        let filename = "test.txt";
        let message = crate::t_format!("File {} not found", "Arquivo {} não encontrado", "Archivo {} no encontrado", filename);
        assert_eq!(message, "File test.txt not found");

        // Test Portuguese
        set_language(Language::Portuguese);
        let message = crate::t_format!("File {} not found", "Arquivo {} não encontrado", "Archivo {} no encontrado", filename);
        assert_eq!(message, "Arquivo test.txt não encontrado");
    }
}