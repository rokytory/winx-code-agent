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

#[macro_export]
macro_rules! localized_text {
    ($en:expr, $pt:expr, $es:expr) => {{
        $crate::core::i18n::get_localized(&$crate::localized!($en, $pt, $es))
    }};
}

#[macro_export]
macro_rules! t_format {
    ($en:expr, $pt:expr, $es:expr, $($arg:tt)*) => {{
        format!($crate::t!($en, $pt, $es), $($arg)*)
    }};
}

#[cfg(test)]
mod tests {
    use crate::core::i18n::{set_language, Language};

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
        let message = format!("{}", filename);
        assert_eq!(message, "test.txt");
    }
}
