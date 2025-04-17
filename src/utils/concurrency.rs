use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

use crate::utils::localized_error;

/// Lock timeout in milliseconds
const LOCK_TIMEOUT_MS: u64 = 5000;

/// Time to wait between consecutive operations on the same file in milliseconds
const OPERATION_DELAY_MS: u64 = 500;

/// Status of a file lock
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockStatus {
    /// File is locked for reading
    ReadLocked,
    /// File is locked for writing
    WriteLocked,
    /// File is not locked
    Unlocked,
}

/// File lock manager to prevent concurrent access issues
#[derive(Debug, Clone)]
pub struct FileLockManager {
    /// Map of file paths to lock status and timestamp
    locks: Arc<RwLock<HashMap<PathBuf, (LockStatus, Instant)>>>,
    /// Map of file paths to last operation time
    last_operations: Arc<RwLock<HashMap<PathBuf, Instant>>>,
}

impl FileLockManager {
    /// Create a new file lock manager
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            last_operations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a read lock on a file
    pub async fn lock_for_reading(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        debug!("Acquiring read lock for file: {}", path.display());

        // Wait for adequate delay since last operation on this file
        self.wait_since_last_operation(&path).await?;

        let start_time = Instant::now();
        loop {
            // Check if we've waited too long for the lock
            if start_time.elapsed().as_millis() > LOCK_TIMEOUT_MS as u128 {
                return Err(localized_error(
                    format!("Timeout waiting for read lock on file: {}", path.display()),
                    format!("Timeout aguardando bloqueio de leitura no arquivo: {}", path.display()),
                    format!("Tiempo de espera agotado esperando el bloqueo de lectura en el archivo: {}", path.display())
                ));
            }

            // Check current lock status - use tokio RwLock for async safety
            let locks = self.locks.read().await;
            match locks.get(&path) {
                Some((LockStatus::WriteLocked, _)) => {
                    // File is write-locked, need to wait
                    drop(locks);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Some((LockStatus::ReadLocked, _)) => {
                    // File is already read-locked, that's fine
                    // Drop the read lock so we can acquire a write lock to update
                    drop(locks);
                    
                    // Update timestamp with a write lock
                    let mut locks = self.locks.write().await;
                    if let Some((_, timestamp)) = locks.get(&path) {
                        let current = *timestamp;
                        locks.insert(path.clone(), (LockStatus::ReadLocked, Instant::now()));
                    } else {
                        locks.insert(path.clone(), (LockStatus::ReadLocked, Instant::now()));
                    }
                    drop(locks);
                    break;
                }
                _ => {
                    // File is not locked, acquire read lock
                    drop(locks);
                    
                    // Update with a write lock
                    let mut locks = self.locks.write().await;
                    locks.insert(path.clone(), (LockStatus::ReadLocked, Instant::now()));
                    drop(locks);
                    break;
                }
            }
        }

        // Update last operation time
        let mut last_ops = self.last_operations.write().await;
        last_ops.insert(path.clone(), Instant::now());
        drop(last_ops);

        debug!("Read lock acquired for file: {}", path.display());
        Ok(())
    }

    /// Get a write lock on a file
    pub async fn lock_for_writing(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        debug!("Acquiring write lock for file: {}", path.display());

        // Wait for adequate delay since last operation on this file
        self.wait_since_last_operation(&path).await?;

        let start_time = Instant::now();
        loop {
            // Check if we've waited too long for the lock
            if start_time.elapsed().as_millis() > LOCK_TIMEOUT_MS as u128 {
                return Err(localized_error(
                    format!("Timeout waiting for write lock on file: {}", path.display()),
                    format!("Timeout aguardando bloqueio de escrita no arquivo: {}", path.display()),
                    format!("Tiempo de espera agotado esperando el bloqueo de escritura en el archivo: {}", path.display())
                ));
            }

            // Read current lock status
            let locks = self.locks.read().await;
            if !locks.contains_key(&path) || 
               matches!(locks.get(&path), Some((LockStatus::Unlocked, _))) {
                // File is not locked, acquire write lock
                drop(locks);
                
                // Update with a write lock
                let mut locks = self.locks.write().await;
                locks.insert(path.clone(), (LockStatus::WriteLocked, Instant::now()));
                drop(locks);
                break;
            } else {
                // File is locked, need to wait
                drop(locks);
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        }

        // Update last operation time
        let mut last_ops = self.last_operations.write().await;
        last_ops.insert(path.clone(), Instant::now());
        drop(last_ops);

        debug!("Write lock acquired for file: {}", path.display());
        Ok(())
    }

    /// Release a lock on a file
    pub async fn release_lock(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        debug!("Releasing lock for file: {}", path.display());

        // First read current timestamp if exists
        let locks = self.locks.read().await;
        let timestamp = locks.get(&path).map(|(_, ts)| *ts).unwrap_or_else(Instant::now);
        drop(locks);
        
        // Now update with write lock
        let mut locks = self.locks.write().await;
        locks.insert(path.clone(), (LockStatus::Unlocked, timestamp));
        drop(locks);

        debug!("Lock released for file: {}", path.display());
        Ok(())
    }

    /// Check if a file is locked
    pub async fn is_locked(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref().to_path_buf();
        
        let locks = self.locks.read().await;
        let result = matches!(locks.get(&path), Some((status, _)) if *status != LockStatus::Unlocked);
        drop(locks);
        
        Ok(result)
    }

    /// Check if a file is locked for writing
    pub async fn is_write_locked(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref().to_path_buf();
        
        let locks = self.locks.read().await;
        let result = matches!(locks.get(&path), Some((LockStatus::WriteLocked, _)));
        drop(locks);
        
        Ok(result)
    }

    /// Wait until enough time has passed since the last operation on a file
    async fn wait_since_last_operation(&self, path: &Path) -> Result<()> {
        let last_ops = self.last_operations.read().await;
        let last_op = last_ops.get(path).cloned();
        drop(last_ops);
        
        if let Some(last_op_time) = last_op {
            let elapsed = last_op_time.elapsed().as_millis();
            let required_delay = OPERATION_DELAY_MS as u128;
            
            if elapsed < required_delay {
                let wait_time = required_delay - elapsed;
                debug!("Waiting {}ms before accessing file: {}", wait_time, path.display());
                tokio::time::sleep(Duration::from_millis(wait_time as u64)).await;
            }
        }
        
        Ok(())
    }

    /// Check if enough time has passed since the last operation on a file
    pub async fn is_cooldown_passed(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = path.as_ref().to_path_buf();
        
        let last_ops = self.last_operations.read().await;
        let result = if let Some(last_op) = last_ops.get(&path) {
            let elapsed = last_op.elapsed().as_millis();
            elapsed >= OPERATION_DELAY_MS as u128
        } else {
            // No previous operation, cooldown is passed
            true
        };
        drop(last_ops);
        
        Ok(result)
    }

    /// Clean up expired locks
    pub async fn cleanup_expired_locks(&self) -> Result<()> {
        debug!("Cleaning up expired locks");
        
        // Find expired locks (read first)
        let locks = self.locks.read().await;
        let expired: Vec<PathBuf> = locks
            .iter()
            .filter(|(_, (status, timestamp))| {
                *status != LockStatus::Unlocked && 
                timestamp.elapsed().as_millis() > LOCK_TIMEOUT_MS as u128 * 2
            })
            .map(|(path, _)| path.clone())
            .collect();
        drop(locks);
        
        // Release expired locks (write after)
        if !expired.is_empty() {
            let mut locks = self.locks.write().await;
            for path in expired {
                debug!("Releasing expired lock for file: {}", path.display());
                locks.insert(path.clone(), (LockStatus::Unlocked, Instant::now()));
            }
            drop(locks);
        }
        
        Ok(())
    }
}

// Global file lock manager singleton using tokio's OnceCell instead of lazy_static
static GLOBAL_LOCK_MANAGER: tokio::sync::OnceCell<FileLockManager> = tokio::sync::OnceCell::const_new();

/// Get the global file lock manager instance
pub async fn get_lock_manager() -> &'static FileLockManager {
    GLOBAL_LOCK_MANAGER.get_or_init(|| async { FileLockManager::new() }).await
}

/// File operation guard for automatic locking and unlocking
pub struct FileOperationGuard {
    path: PathBuf,
    lock_manager: Arc<FileLockManager>,
    released: bool,
}

impl FileOperationGuard {
    /// Create a new file operation guard for reading
    pub async fn for_reading(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        // Need to initialize the global lock manager first
        let lock_manager = {
            let manager = get_lock_manager().await;
            Arc::new(manager.clone())
        };
        
        lock_manager.lock_for_reading(&path).await?;
        
        Ok(Self {
            path,
            lock_manager,
            released: false,
        })
    }
    
    /// Create a new file operation guard for writing
    pub async fn for_writing(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        // Need to initialize the global lock manager first
        let lock_manager = {
            let manager = get_lock_manager().await;
            Arc::new(manager.clone())
        };
        
        lock_manager.lock_for_writing(&path).await?;
        
        Ok(Self {
            path,
            lock_manager,
            released: false,
        })
    }
    
    /// Manually release the lock before drop
    pub async fn release(&mut self) -> Result<()> {
        if !self.released {
            self.lock_manager.release_lock(&self.path).await?;
            self.released = true;
        }
        Ok(())
    }
}

impl Drop for FileOperationGuard {
    fn drop(&mut self) {
        if !self.released {
            // Can't use async in drop, so we need to spawn a task
            let path = self.path.clone();
            let lock_manager = self.lock_manager.clone();
            
            tokio::spawn(async move {
                let _ = lock_manager.release_lock(&path).await;
            });
        }
    }
}

/// Synchronous version of cooldown check for sync contexts
pub fn is_cooldown_passed_sync(path: impl AsRef<Path>) -> bool {
    // Create a runtime for the sync operation
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build() {
            Ok(rt) => {
                rt.block_on(async {
                    let manager = get_lock_manager().await;
                    manager.is_cooldown_passed(&path).await.unwrap_or(true)
                })
            },
            Err(_) => true // Default to true on error
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_lock_manager() {
        let lock_manager = FileLockManager::new();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Test read lock
        lock_manager.lock_for_reading(&file_path).await.unwrap();
        assert!(lock_manager.is_locked(&file_path).await.unwrap());
        assert!(!lock_manager.is_write_locked(&file_path).await.unwrap());

        // Release lock
        lock_manager.release_lock(&file_path).await.unwrap();
        assert!(!lock_manager.is_locked(&file_path).await.unwrap());

        // Test write lock
        lock_manager.lock_for_writing(&file_path).await.unwrap();
        assert!(lock_manager.is_locked(&file_path).await.unwrap());
        assert!(lock_manager.is_write_locked(&file_path).await.unwrap());

        // Release lock
        lock_manager.release_lock(&file_path).await.unwrap();
        assert!(!lock_manager.is_locked(&file_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_cooldown() {
        let lock_manager = FileLockManager::new();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // First operation
        lock_manager.lock_for_reading(&file_path).await.unwrap();
        lock_manager.release_lock(&file_path).await.unwrap();

        // Check cooldown
        assert!(!lock_manager.is_cooldown_passed(&file_path).await.unwrap());

        // Wait for cooldown
        tokio::time::sleep(Duration::from_millis(OPERATION_DELAY_MS + 100)).await;
        assert!(lock_manager.is_cooldown_passed(&file_path).await.unwrap());
    }

    #[tokio::test]
    async fn test_operation_guard() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Initialize the lock manager
        let lock_manager = get_lock_manager().await;

        // Test read guard
        {
            let _guard = FileOperationGuard::for_reading(&file_path).await.unwrap();
            assert!(lock_manager.is_locked(&file_path).await.unwrap());
        }
        
        // Need to wait a bit for the async release in drop
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Lock should be released
        assert!(!lock_manager.is_locked(&file_path).await.unwrap());

        // Wait for cooldown
        tokio::time::sleep(Duration::from_millis(OPERATION_DELAY_MS + 100)).await;

        // Test write guard
        {
            let _guard = FileOperationGuard::for_writing(&file_path).await.unwrap();
            assert!(lock_manager.is_write_locked(&file_path).await.unwrap());
        }
        
        // Need to wait a bit for the async release in drop
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Lock should be released
        assert!(!lock_manager.is_locked(&file_path).await.unwrap());
    }
}
