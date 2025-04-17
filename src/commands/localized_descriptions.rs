use crate::core::i18n::{Language, LocalizedDescription};

/// Obtém descrição localizada para "Create a new task session that can be resumed later"
pub fn create_task_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Create a new task session that can be resumed later",
        pt: "Criar uma nova sessão de tarefa que pode ser retomada posteriormente",
        es: "Crear una nueva sesión de tarea que se puede reanudar más tarde",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "List available tasks"
pub fn list_tasks_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "List available tasks",
        pt: "Listar tarefas disponíveis",
        es: "Listar tareas disponibles",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Start or resume a background process"
pub fn start_background_process_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Start or resume a background process",
        pt: "Iniciar ou retomar um processo em segundo plano",
        es: "Iniciar o reanudar un proceso en segundo plano",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Validate syntax of code"
pub fn validate_syntax_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Validate syntax of code",
        pt: "Validar sintaxe do código",
        es: "Validar sintaxis del código",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Send text to a running interactive process"
pub fn send_text_input_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Send text to a running interactive process",
        pt: "Enviar texto para um processo interativo em execução",
        es: "Enviar texto a un proceso interactivo en ejecución",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Send special keys to a running interactive process"
pub fn send_special_keys_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Send special keys to a running interactive process",
        pt: "Enviar teclas especiais para um processo interativo em execução",
        es: "Enviar teclas especiales a un proceso interactivo en ejecución",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Execute a bash command"
pub fn bash_command_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Execute a bash command",
        pt: "Executar um comando bash",
        es: "Ejecutar un comando bash",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Read files from the filesystem"
pub fn read_files_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Read files from the filesystem",
        pt: "Ler arquivos do sistema de arquivos",
        es: "Leer archivos del sistema de archivos",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Write or edit a file"
pub fn file_write_or_edit_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Write or edit a file",
        pt: "Escrever ou editar um arquivo",
        es: "Escribir o editar un archivo",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Execute an SQL query"
pub fn sql_query_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Execute an SQL query",
        pt: "Executar uma consulta SQL",
        es: "Ejecutar una consulta SQL",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Process sequential thinking for problem solving"
pub fn sequential_thinking_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Process sequential thinking for problem solving",
        pt: "Processar pensamento sequencial para resolução de problemas",
        es: "Procesar pensamiento secuencial para resolución de problemas",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Initialize the VibeCode agent with project understanding"
pub fn init_vibe_code_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Initialize the VibeCode agent with project understanding",
        pt: "Inicializar o agente VibeCode com entendimento do projeto",
        es: "Inicializar el agente VibeCode con comprensión del proyecto",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Analyze a file using the VibeCode agent"
pub fn analyze_file_with_vibe_code_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Analyze a file using the VibeCode agent",
        pt: "Analisar um arquivo usando o agente VibeCode",
        es: "Analizar un archivo usando el agente VibeCode",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Apply search/replace with intelligent error handling"
pub fn smart_search_replace_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Apply search/replace with intelligent error handling",
        pt: "Aplicar busca/substituição com tratamento inteligente de erros",
        es: "Aplicar búsqueda/reemplazo con manejo inteligente de errores",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}

/// Obtém descrição localizada para "Generate code suggestions based on project patterns"
pub fn generate_code_suggestions_description() -> &'static str {
    let desc = LocalizedDescription {
        en: "Generate code suggestions based on project patterns",
        pt: "Gerar sugestões de código baseadas em padrões do projeto",
        es: "Generar sugerencias de código basadas en patrones del proyecto",
    };
    
    match crate::core::i18n::get_language() {
        Language::English => desc.en,
        Language::Portuguese => desc.pt,
        Language::Spanish => desc.es,
    }
}
