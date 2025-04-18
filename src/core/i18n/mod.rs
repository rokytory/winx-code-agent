// Módulo de internacionalização - parte da refatoração do Winx
// Centraliza funcionalidades relacionadas a idiomas

mod descriptions;
mod language;

pub use descriptions::*;
pub use language::*;

/// Inicializa o suporte a idiomas
pub fn init_language_support() {
    language::init_language_support();
    descriptions::register_tool_descriptions();
}
