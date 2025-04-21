use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use log::{debug, warn, info};

use crate::cache::{cached_read_file, invalidate_cached_file};
use crate::error_handling::{with_file_context, with_context};

pub struct FileState {
    // Track file hashes and permissions
    whitelist_for_overwrite: std::collections::HashMap<PathBuf, FileWhitelistData>,
    // Track when files were last read
    read_timestamps: std::collections::HashMap<PathBuf, Instant>,
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
            read_timestamps: std::collections::HashMap::new(),
        }
    }
    
    /// Record file read timestamp
    pub fn track_file_read(&mut self, file_path: &Path) {
        self.read_timestamps.insert(file_path.to_path_buf(), Instant::now());
        debug!("Tracked read of file: {}", file_path.display());
    }
    
    /// Check if a file was read recently (within the last 5 minutes)
    pub fn was_read_recently(&self, file_path: &Path) -> bool {
        if let Some(timestamp) = self.read_timestamps.get(file_path) {
            let elapsed = timestamp.elapsed();
            return elapsed.as_secs() < 300; // 5 minutes
        }
        false
    }

    pub fn add_to_whitelist(
        &mut self,
        file_path: &Path,
        ranges: Vec<(usize, usize)>,
    ) -> Result<()> {
        // Record that this file was read
        self.track_file_read(file_path);
        
        // Try to use cached content if available
        let content = match cached_read_file(file_path) {
            Ok(content) => content.into_bytes(),
            Err(_) => fs::read(file_path)?
        };
        
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
        
        // Add the new ranges
        for range in ranges {
            entry.line_ranges_read.push(range);
        }
        
        // Log progress
        debug!("File {} added to whitelist with {} ranges, {}% read", 
               file_path.display(), entry.line_ranges_read.len(), entry.get_percentage_read());
        
        Ok(())
    }

    pub fn can_overwrite(&self, file_path: &Path) -> bool {
        // If we have a whitelist entry for this file
        if let Some(data) = self.whitelist_for_overwrite.get(file_path) {
            // First check if the file has been read enough
            if !data.is_read_enough() {
                debug!("File {} not read enough ({}%)", 
                      file_path.display(), data.get_percentage_read());
                return false;
            }
            
            // Then verify its contents haven't changed
            match with_file_context(|| fs::read(file_path), file_path) {
                Ok(content) => {
                    let mut hasher = Sha256::new();
                    hasher.update(&content);
                    let current_hash = format!("{:x}", hasher.finalize());
                    
                    let unchanged = current_hash == data.file_hash;
                    if !unchanged {
                        warn!("File {} has been modified since it was read", file_path.display());
                    }
                    
                    return unchanged;
                }
                Err(e) => {
                    warn!("Failed to read file {}: {}", file_path.display(), e);
                    return false;
                }
            }
        }
        
        // Special case for new files
        if !file_path.exists() {
            return true;
        }
        
        debug!("File {} not in whitelist", file_path.display());
        false
    }
}

impl Default for FileState {
    fn default() -> Self {
        Self::new()
    }
}
