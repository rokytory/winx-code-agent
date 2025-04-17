// Módulo de internacionalização - parte da refatoração do Winx
// Centraliza funcionalidades relacionadas a idiomas

mod language;
mod descriptions;

pub use language::*;
pub use descriptions::*;

/// Inicializa o suporte a idiomas
pub fn init_language_support() {
    language::init_language_support();
    descriptions::register_tool_descriptions();
}
