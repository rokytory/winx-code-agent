use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 5;
const MAX_FILES_PER_DIR: usize = 20;
const MAX_TOTAL_FILES: usize = 200;
const IGNORE_DIRS: [&str; 10] = [
    ".git",
    "node_modules",
    "target",
    "build",
    "dist",
    "venv",
    ".venv",
    ".env",
    "__pycache__",
    ".DS_Store",
];

/// Represents activity metrics of a file
#[derive(Debug, Clone, Default)]
pub struct FileActivity {
    pub read_count: usize,
    pub edit_count: usize,
    pub write_count: usize,
}

impl FileActivity {
    /// Calculate activity score
    pub fn activity_score(&self) -> usize {
        self.read_count * 2 + self.edit_count * 3 + self.write_count
    }
}

/// Repository explorer for analyzing workspace structure
pub struct RepositoryExplorer {
    file_activities: HashMap<PathBuf, FileActivity>,
}

impl RepositoryExplorer {
    /// Create a new repository explorer
    pub fn new() -> Self {
        Self {
            file_activities: HashMap::new(),
        }
    }

    /// Track file activity
    pub fn track_activity(&mut self, path: &Path, activity_type: &str) {
        let entry = self.file_activities.entry(path.to_path_buf()).or_default();

        match activity_type {
            "read" => entry.read_count += 1,
            "edit" => entry.edit_count += 1,
            "write" => entry.write_count += 1,
            _ => (),
        }
    }

    /// Get most active files
    pub fn get_active_files(&self, limit: usize) -> Vec<(PathBuf, usize)> {
        let mut files: Vec<_> = self
            .file_activities
            .iter()
            .map(|(path, activity)| (path.clone(), activity.activity_score()))
            .collect();

        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.truncate(limit);

        files
    }

    /// Check if path should be ignored
    fn should_ignore(&self, path: &Path) -> bool {
        if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
            // Check if directory should be ignored
            if path.is_dir() && IGNORE_DIRS.contains(&file_name) {
                return true;
            }

            // Check if it's a hidden file
            if file_name.starts_with('.') && file_name != "." && file_name != ".." {
                return true;
            }
        }

        false
    }

    /// Explore workspace and get directory tree
    pub fn explore_workspace(&self, root_path: &Path) -> Result<String> {
        let mut output = io::Cursor::new(Vec::new());

        writeln!(output, "{}", root_path.display())?;

        self.explore_directory(root_path, &mut output, 0, 0, &mut HashSet::new(), &mut 0)?;

        Ok(String::from_utf8(output.into_inner())?)
    }

    /// Recursively explore directory
    fn explore_directory(
        &self,
        dir: &Path,
        output: &mut io::Cursor<Vec<u8>>,
        depth: usize,
        indent: usize,
        visited: &mut HashSet<PathBuf>,
        total_files: &mut usize,
    ) -> io::Result<()> {
        // Avoid loops
        if visited.contains(dir) {
            return Ok(());
        }
        visited.insert(dir.to_path_buf());

        // Check depth
        if depth > MAX_DEPTH || *total_files >= MAX_TOTAL_FILES {
            return Ok(());
        }

        // List the directory
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                writeln!(
                    output,
                    "{}Error reading directory: {}",
                    " ".repeat(indent),
                    e
                )?;
                return Ok(());
            }
        };

        // Collect and sort entries (directories first, then files)
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();

            if self.should_ignore(&path) {
                continue;
            }

            if path.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }

        // Sort alphabetically
        dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        // Process directories
        for path in dirs {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                writeln!(output, "{}{}/ (dir)", " ".repeat(indent + 2), name)?;
                self.explore_directory(&path, output, depth + 1, indent + 4, visited, total_files)?;
            }
        }

        // Process files (limiting the number per directory)
        let file_count = files.len();
        let files_to_display = files.iter().take(MAX_FILES_PER_DIR).collect::<Vec<_>>();

        for path in files_to_display {
            if *total_files >= MAX_TOTAL_FILES {
                break;
            }

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                writeln!(output, "{}{}", " ".repeat(indent + 2), name)?;
                *total_files += 1;
            }
        }

        // Show if there are more files that weren't displayed
        if file_count > MAX_FILES_PER_DIR {
            let remaining = file_count - MAX_FILES_PER_DIR;
            writeln!(
                output,
                "{}... ({} more files)",
                " ".repeat(indent + 2),
                remaining
            )?;
        }

        Ok(())
    }

    /// Get the most recently modified files
    pub fn get_recent_files(&self, root_path: &Path, limit: usize) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        self.collect_files_recursive(root_path, &mut files, &mut HashSet::new())?;

        // Sort by modification time (newest first)
        files.sort_by(|a, b| {
            let a_time = fs::metadata(a).and_then(|m| m.modified()).ok();
            let b_time = fs::metadata(b).and_then(|m| m.modified()).ok();

            match (a_time, b_time) {
                (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        // Limit the number of files
        files.truncate(limit);

        Ok(files)
    }

    /// Recursively collect files
    fn collect_files_recursive(
        &self,
        dir: &Path,
        files: &mut Vec<PathBuf>,
        visited: &mut HashSet<PathBuf>,
    ) -> io::Result<()> {
        // Avoid loops
        if visited.contains(dir) {
            return Ok(());
        }
        visited.insert(dir.to_path_buf());

        // List the directory
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();

            if self.should_ignore(&path) {
                continue;
            }

            if path.is_dir() {
                self.collect_files_recursive(&path, files, visited)?;
            } else {
                files.push(path);
            }
        }

        Ok(())
    }
}

impl Default for RepositoryExplorer {
    fn default() -> Self {
        Self::new()
    }
}
