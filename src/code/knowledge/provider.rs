// FileKnowledgeProvider - parte da refatoração do Winx
// Define o trait para gerenciamento de conhecimento de arquivos

use anyhow::Result;
use async_trait::async_trait;

use crate::code::vibe_agent::FileKnowledge;

/// Trait que define a API de conhecimento de arquivos
#[async_trait]
pub trait FileKnowledgeProvider: Send + Sync {
    /// Obtém informações sobre um arquivo
    async fn get_file_info(&self, path: &Path) -> Result<FileKnowledge>;

    /// Marca um intervalo de linhas como lido
    async fn mark_read_range(&self, path: &Path, start: usize, end: usize) -> Result<()>;

    /// Verifica se um arquivo foi modificado
    async fn has_file_changed(&self, path: &Path) -> Result<bool>;

    /// Marca um arquivo como modificado
    async fn mark_file_modified(&self, path: &Path) -> Result<()>;

    /// Obtém intervalos não lidos do arquivo
    async fn get_unread_ranges(&self, path: &Path) -> Result<Vec<(usize, usize)>>;

    /// Verifica se o arquivo pode ser editado com segurança
    async fn can_edit_file(&self, path: &Path) -> Result<bool>;

    /// Atualiza todas as informações do arquivo
    async fn refresh_file_info(&self, path: &Path) -> Result<()>;
}
