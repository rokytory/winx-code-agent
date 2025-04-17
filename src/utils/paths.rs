use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Canonicalize a path, resolving all symlinks
pub fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    path.canonicalize()
        .with_context(|| format!("Failed to canonicalize path: {}", path.display()))
}

/// Get the relative path from the base to the target
pub fn relative_to(target: impl AsRef<Path>, base: impl AsRef<Path>) -> Result<PathBuf> {
    let target = target.as_ref();
    let base = base.as_ref();

    // Convert both paths to absolute paths
    let target_abs = canonicalize(target)?;
    let base_abs = canonicalize(base)?;

    // Get the components of each path
    let target_comps = target_abs.components().collect::<Vec<_>>();
    let base_comps = base_abs.components().collect::<Vec<_>>();

    let mut result = PathBuf::new();
    let mut common_len = 0;

    // Find the common prefix
    for (i, (a, b)) in base_comps.iter().zip(target_comps.iter()).enumerate() {
        if a == b {
            common_len = i + 1;
        } else {
            break;
        }
    }

    // Add ".." for each remaining component in base
    for _ in common_len..base_comps.len() {
        result.push("..");
    }

    // Add the remaining components from target
    for comp in target_comps.iter().skip(common_len) {
        result.push(comp.as_os_str());
    }

    Ok(result)
}

/// Expand the tilde in a path to the home directory
pub fn expand_tilde(path: impl AsRef<Path>) -> PathBuf {
    let path_str = path.as_ref().to_string_lossy();

    if path_str.starts_with("~/") {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(path_str.strip_prefix("~/").unwrap())
    } else {
        path.as_ref().to_path_buf()
    }
}

/// Normalize a path, expanding tilde and resolving symlinks
pub fn normalize(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = expand_tilde(path);
    canonicalize(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let path = expand_tilde("~/test.txt");
        assert_eq!(path, home.join("test.txt"));

        let path = expand_tilde("/tmp/test.txt");
        assert_eq!(path, PathBuf::from("/tmp/test.txt"));
    }

    #[test]
    fn test_relative_to() -> Result<()> {
        let dir = tempdir()?;
        let subdir = dir.path().join("subdir");
        std::fs::create_dir_all(&subdir)?;

        let file1 = dir.path().join("file1.txt");
        let file2 = subdir.join("file2.txt");

        std::fs::write(&file1, "")?;
        std::fs::write(&file2, "")?;

        // Test relative path from parent to child
        let rel = relative_to(&file2, dir.path())?;
        assert_eq!(rel, PathBuf::from("subdir/file2.txt"));

        // Test relative path from child to parent
        let rel = relative_to(&file1, &subdir)?;
        assert_eq!(rel, PathBuf::from("../file1.txt"));

        Ok(())
    }
}
