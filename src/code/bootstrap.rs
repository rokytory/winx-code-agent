// Módulo de bootstrap - parte da refatoração do Winx
// Centraliza a inicialização e configuração do sistema

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

use crate::code::knowledge::{FileKnowledgeCache, FileKnowledgeProvider};
use crate::code::vibe_agent::{VibeAgent, VibeAgentFactory};
use crate::core::config::ConfigManager;

/// Inicializa o sistema com configuração padrão
pub async fn initialize_system() -> Result<Arc<VibeAgent>> {
    info!("Inicializando sistema com configuração padrão");

    // Criar o agente com configuração padrão
    let agent = VibeAgentFactory::create();

    Ok(agent)
}

/// Inicializa o sistema com um diretório de projeto
pub async fn initialize_with_project(project_dir: impl AsRef<Path>) -> Result<Arc<VibeAgent>> {
    let project_dir = project_dir.as_ref();
    info!(
        "Inicializando sistema com projeto: {}",
        project_dir.display()
    );

    // Criar o agente
    let agent = VibeAgentFactory::create();

    // Inicializar o agente com o projeto
    agent.initialize(project_dir).await?;

    Ok(agent)
}

/// Inicializa o sistema com configuração personalizada
pub async fn initialize_with_config(config: &ConfigManager) -> Result<Arc<VibeAgent>> {
    info!("Inicializando sistema com configuração personalizada");

    // Criar o cache de conhecimento
    let file_knowledge = Arc::new(FileKnowledgeCache::new(VibeAgent::detect_language));

    // Criar o agente com o provedor personalizado
    let agent = VibeAgentFactory::with_knowledge_provider(file_knowledge);

    // Verificar se há um diretório de projeto na configuração
    if let Some(project_dir) = config.get_default_project_dir() {
        agent.initialize(project_dir).await?;
    }

    Ok(agent)
}

/// Inicializa o sistema para testes
#[cfg(test)]
pub fn initialize_for_testing() -> Arc<VibeAgent> {
    VibeAgentFactory::for_testing()
}
