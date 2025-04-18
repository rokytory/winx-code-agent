// FileKnowledgeCache - parte da refatoração do Winx
// Implementa o cache de conhecimento de arquivos

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, warn};

use crate::code::knowledge::FileKnowledgeProvider;
use crate::code::vibe_agent::FileKnowledge;
use crate::utils::fs;

/// Implementação concreta do provedor de conhecimento de arquivos
pub struct FileKnowledgeCache {
    /// Cache de conhecimento de arquivos
    cache: RwLock<HashMap<PathBuf, Arc<RwLock<FileKnowledge>>>>,
    /// Mutex para operações de criação no cache
    creation_mutex: Mutex<()>,
    /// Detecta linguagens de arquivos
    language_detector: Arc<dyn Fn(&Path) -> String + Send + Sync>,
}

impl FileKnowledgeCache {
    /// Cria um novo cache com o detector de linguagem fornecido
    pub fn new(language_detector: impl Fn(&Path) -> String + Send + Sync + 'static) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            creation_mutex: Mutex::new(()),
            language_detector: Arc::new(language_detector),
        }
    }

    /// Obtém ou cria uma entrada de conhecimento para um arquivo
    async fn get_or_create_entry(&self, path: &Path) -> Result<Arc<RwLock<FileKnowledge>>> {
        // Primeiro, tenta uma leitura rápida do cache
        {
            let cache_read = self.cache.read().await;
            if let Some(entry) = cache_read.get(path) {
                return Ok(entry.clone());
            }
        }

        // Se não encontrou, precisa criar com exclusão mútua
        let _lock = self.creation_mutex.lock().await;

        // Verifica novamente se alguém criou enquanto esperávamos
        {
            let cache_read = self.cache.read().await;
            if let Some(entry) = cache_read.get(path) {
                return Ok(entry.clone());
            }
        }

        // Cria nova entrada
        let language = (self.language_detector)(path);
        let knowledge = FileKnowledge::new(path, language).await?;
        let knowledge_lock = Arc::new(RwLock::new(knowledge));

        // Insere no cache
        {
            let mut cache_write = self.cache.write().await;
            cache_write.insert(path.to_path_buf(), knowledge_lock.clone());
        }

        Ok(knowledge_lock)
    }

    /// Remove uma entrada do cache
    pub async fn invalidate(&self, path: &Path) {
        let mut cache = self.cache.write().await;
        cache.remove(path);
    }

    /// Limpa entradas antigas do cache
    pub async fn purge_stale_entries(&self) -> usize {
        let mut cache = self.cache.write().await;
        let before_count = cache.len();

        // Remove entradas que não existem mais
        cache.retain(|path, _| path.exists());

        let after_count = cache.len();
        before_count - after_count
    }
}

#[async_trait]
impl FileKnowledgeProvider for FileKnowledgeCache {
    async fn get_file_info(&self, path: &Path) -> Result<FileKnowledge> {
        let entry = self.get_or_create_entry(path).await?;
        let knowledge = entry.read().await;
        Ok(knowledge.clone())
    }

    async fn mark_read_range(&self, path: &Path, start: usize, end: usize) -> Result<()> {
        let entry = self.get_or_create_entry(path).await?;
        let mut knowledge = entry.write().await;
        knowledge.mark_read_range(start, end)
    }

    async fn has_file_changed(&self, path: &Path) -> Result<bool> {
        let entry = self.get_or_create_entry(path).await?;
        let knowledge = entry.read().await;
        knowledge.has_changed().await
    }

    async fn mark_file_modified(&self, path: &Path) -> Result<()> {
        let entry = self.get_or_create_entry(path).await?;
        let mut knowledge = entry.write().await;
        knowledge.mark_modified().await
    }

    async fn get_unread_ranges(&self, path: &Path) -> Result<Vec<(usize, usize)>> {
        let entry = self.get_or_create_entry(path).await?;
        let knowledge = entry.read().await;
        Ok(knowledge.get_unread_ranges())
    }

    async fn can_edit_file(&self, path: &Path) -> Result<bool> {
        // Para arquivos novos, podemos editar
        if !path.exists() {
            return Ok(true);
        }

        let entry = self.get_or_create_entry(path).await?;
        let knowledge = entry.read().await;

        if knowledge.has_changed().await? {
            debug!("File has changed since last seen: {}", path.display());
            return Ok(false);
        }

        Ok(knowledge.can_edit())
    }

    async fn refresh_file_info(&self, path: &Path) -> Result<()> {
        self.invalidate(path).await;
        let _ = self.get_or_create_entry(path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    fn test_language_detector(_path: &Path) -> String {
        "Test".to_string()
    }

    #[tokio::test]
    async fn test_file_knowledge_cache() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.txt");

        // Criar arquivo de teste
        let mut file = File::create(&file_path).await?;
        file.write_all(b"Test content\nline 2\nline 3").await?;

        // Criar cache
        let cache = FileKnowledgeCache::new(test_language_detector);

        // Obter informações do arquivo
        let info = cache.get_file_info(&file_path).await?;
        assert_eq!(info.total_lines, 3);
        assert_eq!(info.language, "Test");

        // Marcar como lido
        cache.mark_read_range(&file_path, 1, 2).await?;

        // Verificar ranges não lidos
        let unread = cache.get_unread_ranges(&file_path).await?;
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0], (3, 3));

        Ok(())
    }
}
