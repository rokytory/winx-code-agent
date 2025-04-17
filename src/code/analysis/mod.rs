// Módulo de análise de código - parte da refatoração Winx
// Consolida as funcionalidades de análise que antes estavam espalhadas

mod project;
mod semantic;
mod static_analyzer;
mod syntax;

pub use project::*;
pub use semantic::*;
pub use static_analyzer::*;
pub use syntax::*;
