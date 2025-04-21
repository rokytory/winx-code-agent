use crate::config::project_config::{WinxProjectConfig, FilePurpose, ImportantFile, UsefulCommand};
use std::path::{Path, PathBuf};

/// Carregar configuração compatível com Serena
pub fn load_serena_config(project_path: &Path) -> Option<WinxProjectConfig> {
    // Procurar pelo arquivo de configuração da Serena
    let serena_config_path = project_path.join(".serena/config.json");
    
    if !serena_config_path.exists() {
        return None;
    }
    
    // Tentar ler o arquivo
    if let Ok(content) = std::fs::read_to_string(&serena_config_path) {
        if let Ok(serena_config) = serde_json::from_str::<SerenaConfig>(&content) {
            // Converter para nosso formato de configuração
            return Some(convert_serena_to_winx(serena_config, project_path));
        }
    }
    
    None
}

/// Estrutura de configuração da Serena (simplificada)
#[derive(serde::Deserialize)]
struct SerenaConfig {
    project: SerenaProject,
    files: Vec<SerenaFile>,
    commands: Vec<SerenaCommand>,
    settings: Option<SerenaSettings>,
}

#[derive(serde::Deserialize)]
struct SerenaProject {
    name: String,
    language: String,
    #[serde(default)]
    description: String,
}

#[derive(serde::Deserialize)]
struct SerenaFile {
    path: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    importance: i32,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(serde::Deserialize)]
struct SerenaCommand {
    name: String,
    command: String,
    #[serde(default)]
    description: String,
}

#[derive(serde::Deserialize)]
struct SerenaSettings {
    #[serde(default)]
    token_budget: Option<usize>,
    #[serde(default)]
    max_file_size: Option<usize>,
}

/// Converter configuração Serena para WinxProjectConfig
fn convert_serena_to_winx(serena: SerenaConfig, project_path: &Path) -> WinxProjectConfig {
    // Criar configuração básica
    let mut config = WinxProjectConfig::new(
        serena.project.name,
        project_path
    );
    
    // Definir linguagem principal
    config.main_language = serena.project.language;
    
    // Converter arquivos importantes
    for file in serena.files {
        let file_path = project_path.join(&file.path);
        
        // Determinar o propósito baseado nas tags
        let purpose = if file.tags.contains(&"config".to_string()) {
            FilePurpose::Configuration
        } else if file.tags.contains(&"main".to_string()) {
            FilePurpose::MainEntry
        } else if file.tags.contains(&"test".to_string()) {
            FilePurpose::Test
        } else if file.tags.contains(&"doc".to_string()) {
            FilePurpose::Documentation
        } else {
            FilePurpose::CoreLogic
        };
        
        config.important_files.push(ImportantFile {
            path: file_path,
            description: file.description,
            purpose,
            last_read: None,
            important_sections: Vec::new(),
            read_frequency: 0,
        });
    }
    
    // Converter comandos úteis
    for cmd in serena.commands {
        config.useful_commands.insert(
            cmd.name.clone(),
            UsefulCommand {
                command: cmd.command,
                description: cmd.description,
                success_rate: 1.0, // Assumir 100% inicialmente
                usage_count: 1,     // Iniciar com 1 uso
                context: "From Serena config".to_string(),
            }
        );
    }
    
    // Aplicar configurações de token, se disponíveis
    if let Some(settings) = serena.settings {
        if let Some(budget) = settings.token_budget {
            config.token_economy.token_budget_per_session = budget;
        }
        
        if let Some(max_size) = settings.max_file_size {
            config.token_economy.prioritize_files_under_lines = max_size / 10;
            config.token_economy.summarization_threshold_lines = max_size / 2;
        }
    }
    
    config
}
