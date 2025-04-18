use anyhow::{Context, Result};
use glob::glob;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Read a file's contents as string with concurrency control
pub async fn read_file(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    debug!("Reading file with lock: {}", path.display());

    // Acquire a read lock on the file
    let mut guard = crate::utils::concurrency::FileOperationGuard::for_reading(path).await?;

    // Read file content
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    // Release lock explicitly (will also be released on drop)
    let _ = guard.release().await;

    Ok(content)
}

/// Read a file's contents as string (synchronous version)
pub fn read_file_to_string(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    debug!("Reading file synchronously: {}", path.display());

    // We can't use async locks in a sync context, but we can check if cooldown passed
    if !crate::utils::concurrency::is_cooldown_passed_sync(path) {
        warn!("File access cooldown not passed for: {}", path.display());
        return Err(crate::utils::localized_error(
            format!("File {} was accessed too recently. Please try again in a moment.", path.display()),
            format!("O arquivo {} foi acessado muito recentemente. Por favor, tente novamente em um momento.", path.display()),
            format!("El archivo {} fue accedido muy recientemente. Por favor, inténtelo de nuevo en un momento.", path.display())
        ));
    }

    fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))
}

/// Write string content to a file with concurrency control
pub async fn write_file(path: impl AsRef<Path>, content: &str) -> Result<()> {
    let path = path.as_ref();
    debug!("Writing to file with lock: {}", path.display());

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Acquire a write lock on the file
    let mut guard = crate::utils::concurrency::FileOperationGuard::for_writing(path).await?;

    // Write file content
    fs::write(path, content)
        .with_context(|| format!("Failed to write to file: {}", path.display()))?;

    // Release lock explicitly (will also be released on drop)
    let _ = guard.release().await;

    Ok(())
}

/// Write string content to a file (synchronous version)
pub fn write_file_sync(path: impl AsRef<Path>, content: &str) -> Result<()> {
    let path = path.as_ref();
    debug!("Writing to file synchronously: {}", path.display());

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // We can't use async locks in a sync context, but we can check if cooldown passed
    if !crate::utils::concurrency::is_cooldown_passed_sync(path) {
        warn!("File access cooldown not passed for: {}", path.display());
        return Err(crate::utils::localized_error(
            format!("File {} was accessed too recently. Please try again in a moment.", path.display()),
            format!("O arquivo {} foi acessado muito recentemente. Por favor, tente novamente em um momento.", path.display()),
            format!("El archivo {} fue accedido muy recientemente. Por favor, inténtelo de nuevo en un momento.", path.display())
        ));
    }

    // Use versioning to prevent race conditions
    let hash_before = if path.exists() {
        Some(calculate_file_hash(path)?)
    } else {
        None
    };

    let result = fs::write(path, content)
        .with_context(|| format!("Failed to write to file: {}", path.display()));

    // If successful, validate that no concurrent write happened
    if result.is_ok() && hash_before.is_some() {
        // Give the file system a moment to update
        std::thread::sleep(std::time::Duration::from_millis(10));

        let hash_after = calculate_file_hash(path)?;

        // If hash doesn't match expected result, another process might have modified it
        let expected_hash = calculate_string_hash(content);
        if hash_after != expected_hash {
            warn!(
                "File hash after write doesn't match expected: {}",
                path.display()
            );
            return Err(crate::utils::localized_error(
                format!("File {} may have been modified concurrently during write", path.display()),
                format!("O arquivo {} pode ter sido modificado concorrentemente durante a escrita", path.display()),
                format!("El archivo {} puede haber sido modificado concurrentemente durante la escritura", path.display())
            ));
        }
    }

    result
}

/// Calculate a hash for file content
fn calculate_file_hash(path: impl AsRef<Path>) -> Result<String> {
    let content = fs::read(path.as_ref())?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Calculate a hash for a string
fn calculate_string_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check if a file exists
pub fn file_exists(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    path.exists() && path.is_file()
}

/// Find files matching a glob pattern
pub fn find_files(pattern: &str) -> Result<Vec<PathBuf>> {
    debug!("Finding files matching pattern: {}", pattern);

    let paths = glob(pattern)
        .with_context(|| format!("Invalid glob pattern: {}", pattern))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    debug!("Found {} files matching pattern", paths.len());
    Ok(paths)
}

/// Create a directory and all parent directories
pub fn create_dir_all(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    debug!("Creating directory: {}", path.display());

    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))
}

/// Expand ~ to user's home directory in path
pub fn expand_user(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if path.len() > 1 {
                return home.join(&path[2..]).to_string_lossy().to_string();
            } else {
                return home.to_string_lossy().to_string();
            }
        }
    }
    path.to_string()
}

/// Create a temporary directory for file operations
pub fn create_temp_dir(base_dir: impl AsRef<Path>, prefix: &str) -> Result<PathBuf> {
    let base_dir = base_dir.as_ref();

    // Generate a unique timestamp
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Create a unique directory name
    let temp_dir_name = format!("{}_{}", prefix, timestamp);
    let temp_dir = base_dir.join("tmp").join(temp_dir_name);

    // Create the directory
    if !temp_dir.exists() {
        debug!("Creating temporary directory: {}", temp_dir.display());
        fs::create_dir_all(&temp_dir).with_context(|| {
            format!(
                "Failed to create temporary directory: {}",
                temp_dir.display()
            )
        })?;
    }

    Ok(temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::runtime::Runtime;

    #[test]
    fn test_file_operations() {
        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("test.txt");

            // Test writing to file
            write_file(&file_path, "Hello, world!").await.unwrap();
            assert!(file_exists(&file_path));

            // Test reading from file
            let content = read_file(&file_path).await.unwrap();
            assert_eq!(content, "Hello, world!");
        });
    }

    #[test]
    fn test_find_files() {
        let dir = tempdir().unwrap();

        // Create some test files
        fs::write(dir.path().join("test1.txt"), "").unwrap();
        fs::write(dir.path().join("test2.txt"), "").unwrap();
        fs::create_dir_all(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir").join("test3.txt"), "").unwrap();

        // Test glob pattern
        let pattern = dir.path().join("*.txt").to_string_lossy().to_string();
        let files = find_files(&pattern).unwrap();
        assert_eq!(files.len(), 2);

        // Test recursive glob pattern
        let pattern = dir.path().join("**/*.txt").to_string_lossy().to_string();
        let files = find_files(&pattern).unwrap();
        assert_eq!(files.len(), 3);
    }
}
