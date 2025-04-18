use anyhow::Result;
use std::collections::HashSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Smart directory tree visualizer
pub struct DirectoryTree {
    /// Root path of the tree
    root: PathBuf,
    /// Maximum number of files to display
    #[allow(dead_code)]
    max_files: usize,
    /// Files that should be expanded
    expanded_files: HashSet<PathBuf>,
    /// Directories that should be expanded
    expanded_dirs: HashSet<PathBuf>,
}

impl DirectoryTree {
    /// Create a new directory tree visualizer
    pub fn new(root: impl AsRef<Path>, max_files: usize) -> Result<Self> {
        let root = root.as_ref().to_path_buf();

        if !root.exists() {
            return Err(anyhow::anyhow!(
                "Root path does not exist: {}",
                root.display()
            ));
        }

        if !root.is_dir() {
            return Err(anyhow::anyhow!(
                "Root path is not a directory: {}",
                root.display()
            ));
        }

        Ok(Self {
            root,
            max_files,
            expanded_files: HashSet::new(),
            expanded_dirs: HashSet::new(),
        })
    }

    /// Expand a specific file in the tree
    pub fn expand(&mut self, rel_path: impl AsRef<Path>) -> Result<()> {
        let rel_path = rel_path.as_ref();
        let abs_path = self.root.join(rel_path);

        if !abs_path.exists() {
            return Err(anyhow::anyhow!(
                "Path does not exist: {}",
                abs_path.display()
            ));
        }

        if abs_path.is_file() {
            // Add file to expanded files
            self.expanded_files.insert(abs_path.clone());

            // Add all parent directories to expanded dirs
            let mut current = abs_path.parent().unwrap_or(&self.root).to_path_buf();

            while current.starts_with(&self.root) {
                self.expanded_dirs.insert(current.clone());

                // Move up one directory
                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                } else {
                    break;
                }
            }
        } else if abs_path.is_dir() {
            // Add directory and its parents to expanded dirs
            self.expanded_dirs.insert(abs_path.clone());

            let mut current = abs_path.parent().unwrap_or(&self.root).to_path_buf();

            while current.starts_with(&self.root) {
                self.expanded_dirs.insert(current.clone());

                if let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                } else {
                    break;
                }
            }
        }

        Ok(())
    }

    /// List contents of a directory, sorted with directories first
    fn list_directory(&self, dir_path: &Path) -> Result<Vec<PathBuf>> {
        let mut contents = std::fs::read_dir(dir_path)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>();

        // Sort directories first, then by name
        contents.sort_by(|a, b| match (a.is_dir(), b.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .cmp(
                    &b.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                ),
        });

        Ok(contents)
    }

    /// Count hidden files and directories
    fn count_hidden_items(
        &self,
        dir_path: &Path,
        shown_items: &[PathBuf],
    ) -> Result<(usize, usize)> {
        let all_items = self.list_directory(dir_path)?;
        let shown_items_set: HashSet<_> = shown_items.iter().collect();

        let hidden_items = all_items
            .iter()
            .filter(|p| !shown_items_set.contains(p))
            .collect::<Vec<_>>();

        let hidden_files = hidden_items.iter().filter(|p| p.is_file()).count();
        let hidden_dirs = hidden_items.iter().filter(|p| p.is_dir()).count();

        Ok((hidden_files, hidden_dirs))
    }

    /// Format the directory tree as a string
    pub fn display(&self) -> Result<String> {
        let mut output = String::new();

        // Recursive function to display directory contents
        fn display_recursive(
            tree: &DirectoryTree,
            output: &mut String,
            current_path: &Path,
            indent: usize,
            depth: usize,
        ) -> Result<()> {
            // Print current directory name
            if current_path == tree.root {
                writeln!(output, "{}", current_path.display())?;
            } else {
                let name = current_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                writeln!(
                    output,
                    "{}{}",
                    " ".repeat(indent),
                    name
                )?;
            }

            // Don't recurse beyond depth 1 unless path contains expanded files/dirs
            if depth > 0 && current_path.is_dir() && !tree.expanded_dirs.contains(current_path) {
                return Ok(());
            }

            // For directories, display contents
            if current_path.is_dir() {
                let contents = tree.list_directory(current_path)?;
                let mut shown_items = Vec::new();

                for item in &contents {
                    // Show items if they're expanded or are parents of expanded items
                    let should_show =
                        tree.expanded_files.contains(item) || tree.expanded_dirs.contains(item);

                    if should_show {
                        shown_items.push(item.clone());

                        display_recursive(tree, output, item, indent + 2, depth + 1)?;
                    }
                }

                // Show hidden items count
                let (hidden_files, hidden_dirs) =
                    tree.count_hidden_items(current_path, &shown_items)?;

                if hidden_files > 0 || hidden_dirs > 0 {
                    let mut hidden_message = String::new();

                    if hidden_dirs > 0 {
                        write!(
                            hidden_message,
                            "{} director{}",
                            hidden_dirs,
                            if hidden_dirs == 1 { "y" } else { "ies" }
                        )?;
                    }

                    if hidden_dirs > 0 && hidden_files > 0 {
                        write!(hidden_message, " and ")?;
                    }

                    if hidden_files > 0 {
                        write!(
                            hidden_message,
                            "{} file{}",
                            hidden_files,
                            if hidden_files == 1 { "" } else { "s" }
                        )?;
                    }

                    writeln!(
                        output,
                        "{}{} hidden: {}",
                        " ".repeat(indent + 2),
                        if depth > 0 { "..." } else { "" },
                        hidden_message
                    )?;
                }
            }

            Ok(())
        }

        // Start recursive display
        display_recursive(self, &mut output, &self.root, 0, 0)?;

        Ok(output)
    }

    /// Expand multiple paths at once
    pub fn expand_paths(&mut self, paths: &[impl AsRef<Path>]) -> Result<()> {
        for path in paths {
            self.expand(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_directory_tree_basic() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(temp_path.join("src/lib")).unwrap();
        fs::create_dir_all(temp_path.join("tests")).unwrap();
        fs::write(temp_path.join("README.md"), "test").unwrap();
        fs::write(temp_path.join("src/main.rs"), "test").unwrap();
        fs::write(temp_path.join("src/lib/lib.rs"), "test").unwrap();

        // Create tree and expand some paths
        let mut tree = DirectoryTree::new(temp_path, 10).unwrap();
        tree.expand("src/main.rs").unwrap();
        tree.expand("src/lib/lib.rs").unwrap();

        // Get display output
        let output = tree.display().unwrap();

        // Check that expanded paths are included
        assert!(output.contains("main.rs"));
        assert!(output.contains("lib.rs"));

        // Check that unexpanded paths are not included
        assert!(!output.contains("README.md"));
    }
}
