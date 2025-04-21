use crate::error::{WinxError, WinxResult};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc::{self, Sender};

const PROMPT_CONST: &str = "winx ";

/// Status of a running process
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessStatus {
    /// Process is still running
    Running,
    /// Process has exited with given code
    Exited(i32),
    /// No process is currently running
    NotRunning,
}

/// A command runner that manages an interactive shell
pub struct CommandRunner {
    process: Option<Child>,
    stdout_buffer: Arc<Mutex<String>>,
    stderr_buffer: Arc<Mutex<String>>,
    status: Arc<Mutex<ProcessStatus>>,
    last_command: Arc<Mutex<String>>,
    cwd: Arc<Mutex<String>>,
    tx_input: Option<Sender<String>>,
    tx_ctrl: Option<Sender<i32>>,
    screen_session: Arc<Mutex<Option<String>>>,
}

impl Clone for CommandRunner {
    fn clone(&self) -> Self {
        Self {
            process: None, // Cannot clone Child
            stdout_buffer: Arc::clone(&self.stdout_buffer),
            stderr_buffer: Arc::clone(&self.stderr_buffer),
            status: Arc::clone(&self.status),
            last_command: Arc::clone(&self.last_command),
            cwd: Arc::clone(&self.cwd),
            tx_input: self.tx_input.clone(),
            tx_ctrl: self.tx_ctrl.clone(),
            screen_session: Arc::clone(&self.screen_session),
        }
    }
}

impl Drop for CommandRunner {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}

impl CommandRunner {
    /// Create a new command runner
    pub fn new(initial_dir: &str) -> Self {
        Self {
            process: None,
            stdout_buffer: Arc::new(Mutex::new(String::new())),
            stderr_buffer: Arc::new(Mutex::new(String::new())),
            status: Arc::new(Mutex::new(ProcessStatus::NotRunning)),
            last_command: Arc::new(Mutex::new(String::new())),
            cwd: Arc::new(Mutex::new(initial_dir.to_string())),
            tx_input: None,
            tx_ctrl: None,
            screen_session: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if screen is available on the system
    fn is_screen_available() -> bool {
        Command::new("which")
            .arg("screen")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Create a screen session name based on current time
    fn generate_screen_name() -> String {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        let timestamp = now % 1000000; // Last 6 digits of timestamp
        format!("winx.{}", timestamp)
    }

    /// Start a new screen session and return its name
    fn start_screen_session(&self, cwd: &str) -> WinxResult<String> {
        // Check if screen is available
        if !Self::is_screen_available() {
            return Err(WinxError::bash_error(
                "screen is not available on this system",
            ));
        }

        // Generate session name
        let session_name = Self::generate_screen_name();

        // Create screen session
        let status = Command::new("screen")
            .args(["-dmS", &session_name])
            .current_dir(cwd)
            .status()
            .map_err(|e| {
                WinxError::bash_error(format!("Failed to execute screen command: {}", e))
            })?;

        if !status.success() {
            return Err(WinxError::bash_error(format!(
                "Failed to create screen session. Exit code: {}",
                status.code().unwrap_or(-1)
            )));
        }

        // Store the session name
        {
            let mut screen = self.screen_session.lock().unwrap();
            *screen = Some(session_name.clone());
        }

        Ok(session_name)
    }

    /// Get the current screen session name
    pub fn get_screen_session(&self) -> Option<String> {
        self.screen_session.lock().unwrap().clone()
    }

    /// Start the shell process
    pub fn start_shell(&mut self) -> WinxResult<()> {
        let cwd = self.cwd.lock().unwrap().clone();

        // Simplified approach for command execution without interactive terminal
        // To work around the "Must be connected to a terminal" problem
        let use_non_interactive_shell = true;

        let mut cmd = if use_non_interactive_shell {
            // Non-interactive version that doesn't require a connected terminal
            let mut cmd = Command::new("bash");
            cmd.current_dir(&cwd)
                .env("PS1", PROMPT_CONST)
                .env("TERM", "dumb") // Simpler terminal that doesn't require advanced features
                .arg("-c") // Non-interactive mode
                .arg("echo 'Shell initialized in non-interactive mode'") // Initial test command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            cmd
        } else {
            // Abordagem original com screen
            let use_screen = if Self::is_screen_available() {
                match self.start_screen_session(&cwd) {
                    Ok(session) => {
                        log::info!("Started screen session: {}", session);
                        true
                    }
                    Err(e) => {
                        log::warn!("Failed to start screen session: {}", e);
                        log::debug!("Screen session error details: {:?}", e);
                        false
                    }
                }
            } else {
                log::info!("Screen not available, using direct bash");
                false
            };

            if use_screen {
                let session = self.get_screen_session().unwrap();
                let mut real_cmd = Command::new("screen");
                real_cmd
                    .args(["-S", &session])
                    .current_dir(&cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                real_cmd
            } else {
                let mut cmd = Command::new("bash");
                cmd.current_dir(&cwd)
                    .env("PS1", PROMPT_CONST)
                    .env("TERM", "xterm-256color")
                    .arg("-i") // Interactive mode
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                // Check if directory exists
                if !Path::new(&cwd).exists() {
                    log::warn!(
                        "Directory does not exist: {}, using home directory instead",
                        cwd
                    );
                    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                    cmd.current_dir(home);
                }

                cmd
            }
        };

        let mut child = cmd
            .spawn()
            .map_err(|e| WinxError::bash_error(format!("Failed to spawn shell process: {}", e)))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        // Set up channels for communication
        let (tx_input, rx_input) = mpsc::channel::<String>(100);
        let (tx_ctrl, rx_ctrl) = mpsc::channel::<i32>(10);

        self.tx_input = Some(tx_input);
        self.tx_ctrl = Some(tx_ctrl);

        // Clone references for threads
        let stdout_buffer = Arc::clone(&self.stdout_buffer);
        let stderr_buffer = Arc::clone(&self.stderr_buffer);
        let stdout_status = Arc::clone(&self.status);
        let stderr_status = Arc::clone(&self.status);

        // Handle stdout
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let mut buffer = stdout_buffer.lock().unwrap();
                *buffer += &line;
                *buffer += "\n";
            }

            // Process has ended if we get here
            *stdout_status.lock().unwrap() = ProcessStatus::Exited(0);
        });

        // Handle stderr
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let mut buffer = stderr_buffer.lock().unwrap();
                *buffer += &line;
                *buffer += "\n";
            }

            // Process might have ended if we get here
            if *stderr_status.lock().unwrap() == ProcessStatus::Running {
                *stderr_status.lock().unwrap() = ProcessStatus::Exited(1);
            }
        });

        // Handle stdin - writing to process
        let mut stdin_writer = stdin;
        thread::spawn(move || {
            let mut rx_input = rx_input;
            let mut rx_ctrl = rx_ctrl;

            loop {
                let received = tokio::runtime::Runtime::new().unwrap().block_on(async {
                    tokio::select! {
                        Some(input) = rx_input.recv() => {
                            let _ = stdin_writer.write_all(input.as_bytes());
                            let _ = stdin_writer.flush();
                            true
                        }
                        Some(signal) = rx_ctrl.recv() => {
                            // Handle control signals (e.g., SIGINT = 2)
                            if signal == 2 {
                                let _ = stdin_writer.write_all(&[3]); // Ctrl+C
                                let _ = stdin_writer.flush();
                            }
                            true
                        }
                        else => false,
                    }
                });

                if !received {
                    break;
                }
            }
        });

        *self.status.lock().unwrap() = ProcessStatus::Running;
        self.process = Some(child);

        Ok(())
    }

    /// Execute a command
    pub async fn execute(&self, command: &str) -> WinxResult<()> {
        // Log the command for diagnostic purposes
        log::info!("Executing bash command: {}", command);

        // Always try direct execution first for better reliability
        let direct_result = self.execute_direct_command(command);

        if direct_result.is_ok() {
            log::debug!("Direct command execution successful");
            return direct_result;
        }

        log::debug!("Direct command execution failed, falling back to shell execution");

        if self.tx_input.is_none() {
            log::error!("Shell not initialized - cannot execute command");
            return Err(WinxError::ShellNotStarted);
        }

        // Check if it's a directory change command
        let is_cd_command = command.trim().starts_with("cd ");

        // Check if the command contains a background operation indicator
        let is_background = command.trim().ends_with("&");

        // If using screen and it's a background operation, we can handle it better
        if is_background && self.get_screen_session().is_some() {
            // Instead of using &, use screen to properly background the process
            let screen_cmd = command.trim().trim_end_matches("&").trim();

            // Store the modified command
            {
                let mut last_cmd = self.last_command.lock().unwrap();
                *last_cmd = format!("screen -d -m {}", screen_cmd);
            }

            // Clear previous output
            {
                let mut stdout = self.stdout_buffer.lock().unwrap();
                *stdout = String::new();
                let mut stderr = self.stderr_buffer.lock().unwrap();
                *stderr = String::new();
            }

            // Create a screen detached session for the background command
            let tx = self.tx_input.as_ref().unwrap();
            tx.send(format!("screen -d -m {}\n", screen_cmd))
                .await
                .map_err(|e| {
                    WinxError::bash_error(format!("Failed to send command to shell: {}", e))
                })?;

            // Add some information to the stdout
            let mut stdout = self.stdout_buffer.lock().unwrap();
            *stdout = format!("Command running in background: {}\n", screen_cmd);

            return Ok(());
        }

        // For non-background commands or if screen is not available
        // Clear previous output
        {
            let mut stdout = self.stdout_buffer.lock().unwrap();
            *stdout = String::new();
            let mut stderr = self.stderr_buffer.lock().unwrap();
            *stderr = String::new();
        }

        // Store the command
        {
            let mut last_cmd = self.last_command.lock().unwrap();
            *last_cmd = command.to_string();
        }

        // Log and send the command
        let tx = self.tx_input.as_ref().unwrap();

        // Add a command that ensures output is not truncated or filtered
        if !command.starts_with("cd ") && !command.contains("|") && !command.contains(">") {
            // Normal command with guaranteed output
            let safe_command = format!("{} 2>&1; echo \"WINX_CMD_STATUS=$?\"", command);
            log::debug!("Safe command for execution: {}", safe_command);

            tx.send(format!("{}\n", safe_command)).await.map_err(|e| {
                WinxError::bash_error(format!("Failed to send command to shell: {}", e))
            })?;
        } else {
            // For complex commands, use the standard approach
            tx.send(format!("{}\n", command)).await.map_err(|e| {
                WinxError::bash_error(format!("Failed to send command to shell: {}", e))
            })?;
        }

        // If it's a cd command, update the working directory afterwards
        if is_cd_command {
            // Wait a bit for the command to be executed
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // For cd commands, use the direct execution approach to update the directory
            let dir = command.trim()[3..].trim();
            let cwd = self.get_cwd();
            let new_path = if dir.starts_with("/") {
                PathBuf::from(dir)
            } else {
                PathBuf::from(&cwd).join(dir)
            };

            // Try to get the canonical path
            match std::fs::canonicalize(&new_path) {
                Ok(canonical_path) => {
                    // Directly update the working directory
                    self.update_cwd(canonical_path.to_string_lossy().to_string());
                    log::info!("Updated working directory to: {}", self.get_cwd());
                }
                Err(e) => {
                    log::warn!("Failed to canonicalize path {}: {}", new_path.display(), e);

                    // Try to execute pwd directly
                    let output = Command::new("pwd").current_dir(&new_path).output();

                    if let Ok(output) = output {
                        let pwd_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !pwd_output.is_empty() {
                            self.update_cwd(pwd_output);
                            log::info!("Updated working directory to: {}", self.get_cwd());
                        }
                    }
                }
            }

            // Clear the output
            {
                let mut stdout = self.stdout_buffer.lock().unwrap();
                *stdout = format!("Changed directory to: {}\n", self.get_cwd());
                let mut stderr = self.stderr_buffer.lock().unwrap();
                *stderr = String::new();
            }
        }

        Ok(())
    }

    /// Executes a command directly using std::process::Command instead of interactive shell
    /// This works around the "Must be connected to a terminal" problem
    fn execute_direct_command(&self, command: &str) -> WinxResult<()> {
        log::info!("Attempting direct command execution for: {}", command);

        // Verifica se o comando é composto (contém && ou ||)
        if command.contains("&&") || command.contains("||") || command.contains(";") {
            // Vamos executar em um shell para usar operadores compostos
            log::info!("Detected compound command, using shell execution");

            // Get current working directory
            let cwd = self.cwd.lock().unwrap().clone();

            // Executa o comando em um shell
            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&cwd)
                .env(
                    "PATH",
                    std::env::var("PATH").unwrap_or_else(|_| {
                        "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
                    }),
                )
                .env(
                    "HOME",
                    dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                        .to_string_lossy()
                        .to_string(),
                )
                .output()
                .map_err(|e| {
                    WinxError::bash_error(format!("Failed to execute compound command: {}", e))
                })?;

            // Processa saída
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            // Update buffers
            {
                let mut stdout_buf = self.stdout_buffer.lock().unwrap();
                *stdout_buf = stdout;
            }

            {
                let mut stderr_buf = self.stderr_buffer.lock().unwrap();
                *stderr_buf = stderr;
            }

            // Update status
            {
                let mut status = self.status.lock().unwrap();
                *status = ProcessStatus::Exited(output.status.code().unwrap_or(0));
            }

            // Store the command
            {
                let mut last_cmd = self.last_command.lock().unwrap();
                *last_cmd = command.to_string();
            }

            // Se for um comando cd, precisamos atualizar o diretório após execução
            if command.contains("cd ") {
                // Executar pwd para descobrir o diretório atual
                let pwd_output = std::process::Command::new("bash")
                    .arg("-c")
                    .arg("pwd")
                    .current_dir(&cwd)
                    .output();

                if let Ok(pwd_output) = pwd_output {
                    let new_dir = String::from_utf8_lossy(&pwd_output.stdout)
                        .trim()
                        .to_string();
                    if !new_dir.is_empty() {
                        self.update_cwd(new_dir);
                        log::info!("Updated working directory to: {}", self.get_cwd());

                        // Atualiza a saída para mostrar a mudança de diretório
                        let mut stdout_buf = self.stdout_buffer.lock().unwrap();
                        *stdout_buf =
                            format!("Changed directory to: {}\n{}", self.get_cwd(), stdout_buf);
                    }
                }
            }

            return Ok(());
        }

        // Special handling for cd commands - update internal state only
        if command.trim().starts_with("cd ") {
            let dir = command.trim()[3..].trim();
            let cwd = self.get_cwd();
            let new_path = if dir.starts_with("/") {
                std::path::PathBuf::from(dir)
            } else {
                std::path::PathBuf::from(&cwd).join(dir)
            };

            // Update the internal working directory
            match std::fs::canonicalize(&new_path) {
                Ok(canonical_path) => {
                    self.update_cwd(canonical_path.to_string_lossy().to_string());

                    // Update buffers with success message
                    {
                        let mut stdout_buf = self.stdout_buffer.lock().unwrap();
                        *stdout_buf = format!("Changed directory to: {}\n", self.get_cwd());
                    }

                    {
                        let mut stderr_buf = self.stderr_buffer.lock().unwrap();
                        *stderr_buf = String::new();
                    }

                    // Update status
                    {
                        let mut status = self.status.lock().unwrap();
                        *status = ProcessStatus::Exited(0);
                    }

                    // Store the command
                    {
                        let mut last_cmd = self.last_command.lock().unwrap();
                        *last_cmd = command.to_string();
                    }

                    return Ok(());
                }
                Err(e) => {
                    // Failed to change directory
                    {
                        let mut stderr_buf = self.stderr_buffer.lock().unwrap();
                        *stderr_buf = format!("cd: {}: {}\n", dir, e);
                    }

                    // Update status
                    {
                        let mut status = self.status.lock().unwrap();
                        *status = ProcessStatus::Exited(1);
                    }

                    return Err(WinxError::bash_error(format!(
                        "Failed to change directory: {}",
                        e
                    )));
                }
            }
        }

        // For regular commands, use process execution
        // Extract command and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(WinxError::bash_error("Empty command"));
        }

        let cmd_name = parts[0];
        let args = &parts[1..];

        // Get current working directory
        let cwd = self.cwd.lock().unwrap().clone();

        // Execute command with explicit environment variables
        let mut cmd = std::process::Command::new(cmd_name);
        cmd.args(args)
            .current_dir(&cwd)
            .env(
                "PATH",
                std::env::var("PATH")
                    .unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()),
            )
            .env(
                "HOME",
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                    .to_string_lossy()
                    .to_string(),
            );

        // Execute and capture output
        let output = cmd
            .output()
            .map_err(|e| WinxError::bash_error(format!("Failed to execute command: {}", e)))?;

        // Process output
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        log::debug!("Direct command stdout: {}", stdout);
        if !stderr.is_empty() {
            log::debug!("Direct command stderr: {}", stderr);
        }

        // Update buffers
        {
            let mut stdout_buf = self.stdout_buffer.lock().unwrap();
            *stdout_buf = stdout;
        }

        {
            let mut stderr_buf = self.stderr_buffer.lock().unwrap();
            *stderr_buf = stderr;
        }

        // Update status
        {
            let mut status = self.status.lock().unwrap();
            *status = ProcessStatus::Exited(output.status.code().unwrap_or(0));
        }

        // Store the command
        {
            let mut last_cmd = self.last_command.lock().unwrap();
            *last_cmd = command.to_string();
        }

        Ok(())
    }

    /// Send text to the process
    pub async fn send_text(&self, text: &str) -> WinxResult<()> {
        if self.tx_input.is_none() {
            return Err(WinxError::ShellNotStarted);
        }

        let tx = self.tx_input.as_ref().unwrap();
        tx.send(text.to_string())
            .await
            .map_err(|e| WinxError::bash_error(format!("Failed to send text to shell: {}", e)))?;

        Ok(())
    }

    /// Send an interrupt signal to the process
    pub async fn send_interrupt(&self) -> WinxResult<()> {
        if self.tx_ctrl.is_none() {
            return Err(WinxError::ShellNotStarted);
        }

        let tx = self.tx_ctrl.as_ref().unwrap();
        tx.send(2).await.map_err(|e| {
            WinxError::bash_error(format!("Failed to send interrupt to shell: {}", e))
        })?; // SIGINT = 2

        Ok(())
    }

    /// Get the current output
    pub fn get_output(&self) -> (String, String) {
        let stdout = self.stdout_buffer.lock().unwrap().clone();
        let stderr = self.stderr_buffer.lock().unwrap().clone();

        // Log the output for diagnostic purposes
        log::debug!(
            "Command output - stdout len: {}, stderr len: {}",
            stdout.len(),
            stderr.len()
        );

        if stdout.len() < 100 {
            log::debug!("Full stdout: {:?}", stdout);
        } else {
            log::debug!("Stdout preview: {:?}...", &stdout[..100]);
        }

        if !stderr.is_empty() {
            log::debug!("Stderr: {:?}", stderr);
        }

        (stdout, stderr)
    }

    /// Check the status with timeout
    pub async fn check_status(&self, timeout_secs: f64) -> ProcessStatus {
        let status_clone = Arc::clone(&self.status);
        let start_time = std::time::Instant::now();

        loop {
            {
                let status = status_clone.lock().unwrap();
                if *status != ProcessStatus::Running {
                    return status.clone();
                }
            }

            // Check if timeout reached
            if start_time.elapsed().as_secs_f64() > timeout_secs {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Return current status
        self.status.lock().unwrap().clone()
    }

    /// Update the current working directory
    pub fn update_cwd(&self, new_cwd: String) {
        let mut cwd = self.cwd.lock().unwrap();
        *cwd = new_cwd;
    }

    /// Get the current working directory
    pub fn get_cwd(&self) -> String {
        self.cwd.lock().unwrap().clone()
    }

    /// Get formatted status information
    pub fn get_status_info(&self) -> String {
        let status = self.status.lock().unwrap().clone();
        let cwd = self.get_cwd();

        match status {
            ProcessStatus::Running => format!("status = still running\ncwd = {}\n", cwd),
            ProcessStatus::Exited(code) => format!(
                "status = process exited with code {}\ncwd = {}\n",
                code, cwd
            ),
            ProcessStatus::NotRunning => format!("status = no process running\ncwd = {}\n", cwd),
        }
    }
}
