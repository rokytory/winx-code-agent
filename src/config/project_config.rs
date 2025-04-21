use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WinxProjectConfig {
    // Informações básicas do projeto
    pub project_name: String,
    pub project_type: ProjectType,
    pub main_language: String,
    
    // Estrutura de arquivos e pastas importantes
    pub important_files: Vec<ImportantFile>,
    pub important_directories: Vec<ImportantDirectory>,
    
    // Padrões de codificação detectados
    pub coding_patterns: HashMap<String, CodingPattern>,
    
    // Comandos úteis para este projeto
    pub useful_commands: HashMap<String, UsefulCommand>,
    
    // Histórico de interações bem-sucedidas
    pub successful_interactions: Vec<SuccessfulInteraction>,
    
    // Vocabulário de domínio específico do projeto
    pub domain_vocabulary: HashSet<String>,
    
    // Metadados de tokens e economia
    pub token_economy: TokenEconomyConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ProjectType {
    RustProject,
    PythonProject,
    NodeJsProject,
    JavaProject,
    GoProject,
    OtherProject(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportantFile {
    pub path: PathBuf,
    pub description: String,
    pub purpose: FilePurpose,
    pub last_read: Option<u64>, // timestamp
    pub important_sections: Vec<FileSection>,
    pub read_frequency: usize,  // quantas vezes foi lido
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FilePurpose {
    Configuration,
    MainEntry,
    CoreLogic,
    Test,
    Documentation,
    Dependency,
    Build,
    Other(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileSection {
    pub start_line: usize,
    pub end_line: usize,
    pub description: String,
    pub importance: Importance,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub enum Importance {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportantDirectory {
    pub path: PathBuf,
    pub description: String,
    pub purpose: DirectoryPurpose,
    pub file_patterns: Vec<String>, // glob patterns
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DirectoryPurpose {
    Source,
    Test,
    Build,
    Config,
    Documentation,
    ThirdParty,
    Other(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodingPattern {
    pub pattern_name: String,
    pub description: String,
    pub example_files: Vec<PathBuf>,
    pub confidence: f64, // 0.0 a 1.0
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsefulCommand {
    pub command: String,
    pub description: String,
    pub success_rate: f64,
    pub usage_count: usize,
    pub context: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuccessfulInteraction {
    pub task_description: String,
    pub sequence_of_actions: Vec<ActionRecord>,
    pub user_feedback: Option<String>,
    pub success_rating: f64, // 0.0 a 1.0
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActionRecord {
    pub action_type: String,
    pub parameters: serde_json::Value,
    pub result_summary: String,
    pub tokens_used: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenEconomyConfig {
    pub max_tokens_per_file_read: usize,
    pub prioritize_files_under_lines: usize,
    pub summarization_threshold_lines: usize,
    pub token_budget_per_session: usize,
    pub tokens_spent: usize,
}

impl WinxProjectConfig {
    pub fn new(project_name: String, project_path: &Path) -> Self {
        // Detectar o tipo de projeto com base nos arquivos presentes
        let project_type = detect_project_type(project_path);
        let main_language = detect_main_language(project_path);
        
        // Inicializar com valores padrão
        Self {
            project_name,
            project_type,
            main_language,
            important_files: Vec::new(),
            important_directories: Vec::new(),
            coding_patterns: HashMap::new(),
            useful_commands: HashMap::new(),
            successful_interactions: Vec::new(),
            domain_vocabulary: HashSet::new(),
            token_economy: TokenEconomyConfig {
                max_tokens_per_file_read: 2000,
                prioritize_files_under_lines: 300,
                summarization_threshold_lines: 500,
                token_budget_per_session: 100000,
                tokens_spent: 0,
            },
        }
    }
    
    // Carregar a configuração de um arquivo .winx
    pub fn load(project_path: &Path) -> Result<Self, std::io::Error> {
        let config_path = project_path.join(".winx/project.json");
        if !config_path.exists() {
            // Criar um novo arquivo de configuração padrão
            let project_name = project_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown_project")
                .to_string();
            
            let config = Self::new(project_name, project_path);
            config.save(project_path)?;
            return Ok(config);
        }
        
        let content = std::fs::read_to_string(config_path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    // Salvar a configuração em um arquivo .winx
    pub fn save(&self, project_path: &Path) -> Result<(), std::io::Error> {
        let winx_dir = project_path.join(".winx");
        if !winx_dir.exists() {
            std::fs::create_dir_all(&winx_dir)?;
        }
        
        let config_path = winx_dir.join("project.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }
    
    // Adicionar um arquivo importante
    pub fn add_important_file(&mut self, 
                             path: PathBuf, 
                             description: String, 
                             purpose: FilePurpose) {
        // Verificar se o arquivo já existe
        if let Some(existing) = self.important_files.iter_mut()
            .find(|f| f.path == path) {
            // Atualizar informações existentes
            existing.description = description;
            existing.purpose = purpose;
            existing.read_frequency += 1;
            existing.last_read = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs());
            return;
        }
        
        // Adicionar novo arquivo
        self.important_files.push(ImportantFile {
            path,
            description,
            purpose,
            last_read: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
            important_sections: Vec::new(),
            read_frequency: 1,
        });
    }
    
    // Registrar um comando útil
    pub fn record_useful_command(&mut self, 
                                command: String, 
                                description: String, 
                                context: String,
                                success: bool) {
        let cmd_key = command.clone();
        
        // Atualizar comando existente ou criar novo
        let entry = self.useful_commands.entry(cmd_key).or_insert(UsefulCommand {
            command,
            description,
            success_rate: if success { 1.0 } else { 0.0 },
            usage_count: 0,
            context,
        });
        
        // Atualizar estatísticas
        entry.usage_count += 1;
        let success_value = if success { 1.0 } else { 0.0 };
        entry.success_rate = ((entry.success_rate * (entry.usage_count - 1) as f64) + success_value) 
            / entry.usage_count as f64;
    }
    
    // Adicionar uma interação bem-sucedida
    pub fn add_successful_interaction(&mut self, 
                                     task: String, 
                                     actions: Vec<ActionRecord>,
                                     feedback: Option<String>,
                                     rating: f64) {
        self.successful_interactions.push(SuccessfulInteraction {
            task_description: task,
            sequence_of_actions: actions,
            user_feedback: feedback,
            success_rating: rating,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    }
    
    // Registrar consumo de tokens
    pub fn record_token_usage(&mut self, tokens: usize) {
        self.token_economy.tokens_spent += tokens;
    }
    
    // Verificar se um arquivo já foi lido antes
    pub fn is_file_known(&self, path: &Path) -> bool {
        self.important_files.iter().any(|f| f.path == path)
    }
    
    // Obter arquivos mais importantes para entender o projeto
    pub fn get_key_files(&self, max_count: usize) -> Vec<&ImportantFile> {
        let mut files: Vec<&ImportantFile> = self.important_files.iter().collect();
        
        // Ordenar por importância implícita (combinação de propósito e frequência)
        files.sort_by(|a, b| {
            let a_score = file_importance_score(a);
            let b_score = file_importance_score(b);
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        files.truncate(max_count);
        files
    }
    
    // Obter comandos mais úteis para este projeto
    pub fn get_useful_commands(&self, context: &str, max_count: usize) -> Vec<&UsefulCommand> {
        let mut commands: Vec<&UsefulCommand> = self.useful_commands.values().collect();
        
        // Filtrar por contexto se fornecido
        if !context.is_empty() {
            commands = commands.into_iter()
                .filter(|cmd| cmd.context.contains(context))
                .collect();
        }
        
        // Ordenar por utilidade (taxa de sucesso * contagem de uso)
        commands.sort_by(|a, b| {
            let a_score = a.success_rate * (a.usage_count as f64).sqrt();
            let b_score = b.success_rate * (b.usage_count as f64).sqrt();
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        commands.truncate(max_count);
        commands
    }
}

// Função auxiliar para calcular pontuação de importância de arquivo
fn file_importance_score(file: &ImportantFile) -> f64 {
    let purpose_score = match file.purpose {
        FilePurpose::MainEntry => 1.0,
        FilePurpose::CoreLogic => 0.9,
        FilePurpose::Configuration => 0.8,
        FilePurpose::Build => 0.7,
        FilePurpose::Dependency => 0.6,
        FilePurpose::Test => 0.5,
        FilePurpose::Documentation => 0.4,
        FilePurpose::Other(_) => 0.3,
    };
    
    // Combinar propósito com frequência de leitura normalizada
    purpose_score * (0.1 + 0.9 * (1.0 - 1.0 / (file.read_frequency as f64 + 1.0)))
}

// Detectar tipo de projeto baseado nos arquivos presentes
fn detect_project_type(project_path: &Path) -> ProjectType {
    if project_path.join("Cargo.toml").exists() {
        ProjectType::RustProject
    } else if project_path.join("package.json").exists() {
        ProjectType::NodeJsProject
    } else if project_path.join("pyproject.toml").exists() 
        || project_path.join("setup.py").exists() 
        || project_path.join("requirements.txt").exists() {
        ProjectType::PythonProject
    } else if project_path.join("pom.xml").exists() 
        || project_path.join("build.gradle").exists() {
        ProjectType::JavaProject
    } else if project_path.join("go.mod").exists() {
        ProjectType::GoProject
    } else {
        ProjectType::OtherProject("unknown".to_string())
    }
}

// Detectar linguagem principal baseado nos arquivos
fn detect_main_language(project_path: &Path) -> String {
    // Função simplificada - numa implementação real faria contagem de arquivos
    if project_path.join("Cargo.toml").exists() {
        "Rust".to_string()
    } else if project_path.join("package.json").exists() {
        "JavaScript/TypeScript".to_string()
    } else if project_path.join("pyproject.toml").exists() 
        || project_path.join("setup.py").exists() 
        || project_path.join("requirements.txt").exists() {
        "Python".to_string()
    } else if project_path.join("pom.xml").exists() 
        || project_path.join("build.gradle").exists() {
        "Java".to_string()
    } else if project_path.join("go.mod").exists() {
        "Go".to_string()
    } else {
        "Unknown".to_string()
    }
}
