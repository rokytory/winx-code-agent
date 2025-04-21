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

        // Try to use screen if available
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

        let mut cmd = if use_screen {
            let session = self.get_screen_session().unwrap();
            let mut cmd = Command::new("screen");
            cmd.args(["-r", &session, "-X", "stuff", ""]);
            // We're not using this command directly, but setup for later interaction

            // The actual command for I/O will attach to the session
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
                .env("TERM", "xterm-256color") // Definir variável TERM para evitar erros de terminal
                .arg("-i") // Modo interativo
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // Verificar se o diretório existe
            if !Path::new(&cwd).exists() {
                log::warn!(
                    "Directory does not exist: {}, using home directory instead",
                    cwd
                );
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                cmd.current_dir(home);
            }

            cmd
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
        if self.tx_input.is_none() {
            return Err(WinxError::ShellNotStarted);
        }

        // Verificar se é um comando de mudança de diretório
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
        // Log o comando que será executado para diagnóstico
        log::debug!("Executing bash command: {}", command);

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

        // Log e envie o comando
        let tx = self.tx_input.as_ref().unwrap();

        // Adicionar um comando que garante que a saída não seja truncada ou filtrada
        if !command.starts_with("cd ") && !command.contains("|") && !command.contains(">") {
            // Comando normal com garantia de saída
            let safe_command = format!("{} 2>&1; echo \"WINX_CMD_STATUS=$?\"", command);
            log::debug!("Safe command for execution: {}", safe_command);

            tx.send(format!("{}\n", safe_command)).await.map_err(|e| {
                WinxError::bash_error(format!("Failed to send command to shell: {}", e))
            })?;
        } else {
            // Para comandos complexos, use a abordagem padrão
            tx.send(format!("{}\n", command)).await.map_err(|e| {
                WinxError::bash_error(format!("Failed to send command to shell: {}", e))
            })?;
        }

        // Se for um comando cd, vamos atualizar o diretório de trabalho depois
        if is_cd_command {
            // Espere um pouco para o comando ser executado
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Vamos executar um pwd para descobrir o diretório atual
            let tx = self.tx_input.as_ref().unwrap();
            tx.send("pwd\n".to_string()).await.map_err(|e| {
                WinxError::bash_error(format!("Failed to send pwd command to shell: {}", e))
            })?;

            // Espere um pouco para o comando ser executado
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Obtenha a saída (geralmente, o diretório atual estará no stdout)
            let (stdout, _) = self.get_output();

            // Processe a saída para obter o caminho
            if let Some(pwd_output) = stdout.lines().next() {
                let trimmed_path = pwd_output.trim();
                if !trimmed_path.is_empty() {
                    // Atualize o diretório de trabalho
                    self.update_cwd(trimmed_path.to_string());
                    log::info!("Updated working directory to: {}", trimmed_path);
                }
            }

            // Limpe novamente a saída
            {
                let mut stdout = self.stdout_buffer.lock().unwrap();
                *stdout = String::new();
                let mut stderr = self.stderr_buffer.lock().unwrap();
                *stderr = String::new();
            }

            // Reenvie o comando original
            let tx = self.tx_input.as_ref().unwrap();
            tx.send(format!("{}\n", command)).await.map_err(|e| {
                WinxError::bash_error(format!("Failed to resend original command to shell: {}", e))
            })?;
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

        // Log a saída para diagnóstico
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
