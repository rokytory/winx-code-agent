use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Integration with Git for version control
pub struct GitIntegration {
    /// Repository root path
    repo_path: PathBuf,
}

impl GitIntegration {
    /// Create a new Git integration
    pub fn new(repo_path: impl AsRef<Path>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
        }
    }

    /// Check if the directory is a git repository
    pub async fn is_git_repo(&self) -> bool {
        let output = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&self.repo_path)
            .output()
            .await;

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Create a new branch
    pub async fn create_branch(&self, branch_name: &str) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        info!("Creating git branch: {}", branch_name);

        let output = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to create git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create branch: {}", stderr));
        }

        Ok(())
    }

    /// Checkout an existing branch
    pub async fn checkout_branch(&self, branch_name: &str) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        info!("Checking out git branch: {}", branch_name);

        let output = Command::new("git")
            .args(["checkout", branch_name])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to checkout git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to checkout branch: {}", stderr));
        }

        Ok(())
    }

    /// Stage files for commit
    pub async fn stage_files(&self, files: &[impl AsRef<Path>]) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        let file_paths: Vec<&str> = files
            .iter()
            .map(|path| path.as_ref().to_str().unwrap_or(""))
            .collect();

        if file_paths.is_empty() {
            return Ok(());
        }

        info!("Staging {} files", file_paths.len());

        let mut args = vec!["add"];
        args.extend(file_paths);

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to stage files")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to stage files: {}", stderr));
        }

        Ok(())
    }

    /// Stage all changes
    pub async fn stage_all(&self) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        info!("Staging all changes");

        let output = Command::new("git")
            .args(["add", "."])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to stage all changes")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to stage all changes: {}", stderr));
        }

        Ok(())
    }

    /// Create a commit with the given message
    pub async fn commit(&self, message: &str) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        info!("Creating commit: {}", message);

        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to create commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If there are no changes staged, it's not a fatal error
            if stderr.contains("nothing to commit") {
                warn!("Nothing to commit: {}", stderr);
                return Ok(());
            }
            return Err(anyhow::anyhow!("Failed to create commit: {}", stderr));
        }

        Ok(())
    }

    /// Get the current branch name
    pub async fn get_current_branch(&self) -> Result<String> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get current branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get current branch: {}", stderr));
        }

        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(branch)
    }

    /// Get the list of modified files
    pub async fn get_modified_files(&self) -> Result<Vec<String>> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get modified files")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get modified files: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .filter_map(|line| {
                if line.len() > 3 {
                    Some(line[3..].to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(files)
    }

    /// Stage modified files and create a commit
    pub async fn stage_and_commit(
        &self,
        message: &str,
        files: Option<&[impl AsRef<Path>]>,
    ) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        // Stage files
        if let Some(files) = files {
            self.stage_files(files).await?;
        } else {
            self.stage_all().await?;
        }

        // Create commit
        self.commit(message).await?;

        Ok(())
    }

    /// Create a new branch, stage changes, and commit
    pub async fn create_branch_and_commit(
        &self,
        branch_name: &str,
        commit_message: &str,
        files: Option<&[impl AsRef<Path>]>,
    ) -> Result<()> {
        if !self.is_git_repo().await {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        // Create new branch
        self.create_branch(branch_name).await?;

        // Stage and commit
        self.stage_and_commit(commit_message, files).await?;

        Ok(())
    }
}
