use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub struct FileState {
    // Track file hashes and permissions
    whitelist_for_overwrite: std::collections::HashMap<PathBuf, FileWhitelistData>,
}

pub struct FileWhitelistData {
    pub file_hash: String,
    pub line_ranges_read: Vec<(usize, usize)>,
    pub total_lines: usize,
}

impl FileWhitelistData {
    pub fn new(
        file_hash: String,
        line_ranges_read: Vec<(usize, usize)>,
        total_lines: usize,
    ) -> Self {
        Self {
            file_hash,
            line_ranges_read,
            total_lines,
        }
    }

    pub fn get_percentage_read(&self) -> f64 {
        if self.total_lines == 0 {
            return 100.0;
        }

        let mut lines_read = std::collections::HashSet::new();
        for (start, end) in &self.line_ranges_read {
            for line in *start..=*end {
                lines_read.insert(line);
            }
        }

        (lines_read.len() as f64 / self.total_lines as f64) * 100.0
    }

    pub fn is_read_enough(&self) -> bool {
        self.get_percentage_read() >= 99.0
    }
}

impl FileState {
    pub fn new() -> Self {
        Self {
            whitelist_for_overwrite: std::collections::HashMap::new(),
        }
    }

    pub fn add_to_whitelist(
        &mut self,
        file_path: &Path,
        ranges: Vec<(usize, usize)>,
    ) -> Result<()> {
        let content = fs::read(file_path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let file_hash = format!("{:x}", hasher.finalize());

        let total_lines = content.iter().filter(|&&b| b == b'\n').count() + 1;

        let entry = self
            .whitelist_for_overwrite
            .entry(file_path.to_path_buf())
            .or_insert_with(|| FileWhitelistData::new(file_hash.clone(), Vec::new(), total_lines));

        // Update hash if changed
        entry.file_hash = file_hash;

        // Add new ranges
        for range in ranges {
            entry.line_ranges_read.push(range);
        }

        Ok(())
    }

    pub fn can_overwrite(&self, file_path: &Path) -> bool {
        if let Some(data) = self.whitelist_for_overwrite.get(file_path) {
            // Check if file hash matches and enough of the file has been read
            if let Ok(content) = fs::read(file_path) {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                let current_hash = format!("{:x}", hasher.finalize());

                return current_hash == data.file_hash && data.is_read_enough();
            }
        }

        false
    }
}

impl Default for FileState {
    fn default() -> Self {
        Self::new()
    }
}
