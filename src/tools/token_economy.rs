use crate::config::project_config::{WinxProjectConfig, FilePurpose, Importance};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Gerenciador de economia de tokens que decide como ler arquivos de forma eficiente
pub struct TokenManager {
    /// Orçamento total de tokens para a sessão
    budget: usize,
    /// Tokens gastos até agora
    spent: usize,
    /// Configuração do projeto
    project_config: Option<WinxProjectConfig>,
}

// Singleton para o gerenciador de tokens
lazy_static::lazy_static! {
    static ref TOKEN_MANAGER: Arc<Mutex<TokenManager>> = Arc::new(Mutex::new(
        TokenManager::new(100000) // Valor padrão para o orçamento
    ));
}

/// Obter o gerenciador de tokens global
pub fn get_token_manager() -> Arc<Mutex<TokenManager>> {
    TOKEN_MANAGER.clone()
}

impl TokenManager {
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            spent: 0,
            project_config: None,
        }
    }
    
    /// Atualizar com configuração do projeto
    pub fn update_config(&mut self, config: WinxProjectConfig) {
        self.budget = config.token_economy.token_budget_per_session;
        self.spent = config.token_economy.tokens_spent;
        self.project_config = Some(config);
    }
    
    /// Verificar se um arquivo deve ser lido completo ou resumido
    pub fn should_read_full_file(&self, path: &Path, line_count: usize) -> bool {
        // Se não temos configuração, ler arquivos pequenos integralmente
        if self.project_config.is_none() {
            return line_count < 300;
        }
        
        let config = self.project_config.as_ref().unwrap();
        
        // Verificar se é um arquivo importante
        let is_important = config.important_files.iter()
            .any(|f| f.path == path && matches!(f.purpose, 
                FilePurpose::MainEntry | 
                FilePurpose::CoreLogic | 
                FilePurpose::Configuration));
                
        // Ler arquivos importantes ou pequenos integralmente
        is_important || line_count < config.token_economy.prioritize_files_under_lines
    }
    
    /// Decidir quais partes de um arquivo grande ler
    pub fn get_sections_to_read(&self, path: &Path, line_count: usize) 
        -> Vec<(usize, usize)> {
        if self.should_read_full_file(path, line_count) {
            return vec![(1, line_count)];
        }
        
        let mut sections = Vec::new();
        
        // Sempre ler o início do arquivo (cabeçalhos, imports, etc)
        sections.push((1, 50.min(line_count)));
        
        // Se temos configuração com seções importantes
        if let Some(config) = &self.project_config {
            if let Some(file) = config.important_files.iter()
                .find(|f| f.path == path) {
                
                // Adicionar seções marcadas como importantes
                for section in &file.important_sections {
                    if section.importance >= Importance::Medium {
                        sections.push((section.start_line, section.end_line));
                    }
                }
            }
        }
        
        // Se o arquivo não é muito grande, adicionar também o final
        if line_count < 1000 {
            let end_start = (line_count - 100).max(1);
            sections.push((end_start, line_count));
        }
        
        // Mesclar seções sobrepostas
        sections.sort_by_key(|s| s.0);
        let mut merged = Vec::new();
        
        for section in sections {
            if let Some(last) = merged.last_mut() {
                if section.0 <= last.1 + 5 {
                    // Seções próximas ou sobrepostas, mesclar
                    last.1 = last.1.max(section.1);
                } else {
                    // Nova seção separada
                    merged.push(section);
                }
            } else {
                merged.push(section);
            }
        }
        
        merged
    }
    
    /// Registrar uso de tokens
    pub fn record_token_usage(&mut self, tokens: usize) -> bool {
        self.spent += tokens;
        
        // Atualizar na configuração do projeto
        if let Some(config) = &mut self.project_config {
            config.token_economy.tokens_spent += tokens;
        }
        
        // Retornar se ainda estamos dentro do orçamento
        self.spent <= self.budget
    }
    
    /// Obter tokens restantes
    pub fn remaining_tokens(&self) -> usize {
        self.budget.saturating_sub(self.spent)
    }
    
    /// Estimar tokens para ler um arquivo
    pub fn estimate_tokens_for_file(&self, line_count: usize) -> usize {
        // Estimativa simplificada: ~10 tokens por linha
        line_count * 10
    }
    
    /// Decidir se vale a pena ler um arquivo com base no orçamento
    pub fn should_read_file(&self, path: &Path, line_count: usize) -> (bool, String) {
        let estimated_tokens = self.estimate_tokens_for_file(line_count);
        
        // Se for um arquivo conheci