use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BashState {
    pub cwd: PathBuf,
    pub workspace_root: PathBuf,
    pub mode: String,
}

#[derive(Debug, Clone)]
pub enum ProcessStatus {
    Running,
    Exited(i32),
    NotRunning,
}

impl BashState {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            mode: "wcgw".to_string(),
        }
    }

    pub fn update_cwd(&mut self, new_cwd: PathBuf) {
        self.cwd = new_cwd;
    }

    pub fn set_workspace_root(&mut self, workspace_root: PathBuf) {
        self.workspace_root = workspace_root;
    }

    pub fn set_mode(&mut self, mode: String) {
        self.mode = mode;
    }

    pub fn get_status(&self) -> String {
        format!("status = process exited\ncwd = {}\n", self.cwd.display())
    }
}

impl Default for BashState {
    fn default() -> Self {
        Self::new()
    }
}
