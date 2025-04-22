use crate::error::{WinxError, WinxResult};
use log::{debug, info, warn};
use regex::Regex;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Global counter for unique session names
static SESSION_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct ScreenManager;

impl ScreenManager {
    /// Check if the screen command is available on the system
    pub fn is_screen_available() -> bool {
        Command::new("which")
            .arg("screen")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Generate a unique screen session name
    pub fn generate_session_name() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Format: winx.PID.timestamp.counter (similar to wcgw pattern)
        let pid = std::process::id();
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("winx.{}.{}.{}", pid, timestamp % 1000000, counter)
    }

    /// Get all active WINX screen sessions
    pub fn get_winx_screen_sessions() -> WinxResult<Vec<String>> {
        let output = Command::new("screen")
            .arg("-ls")
            .output()
            .map_err(|e| WinxError::bash_error(format!("Failed to list screen sessions: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();

        // Parse screen output - looking for sessions with "winx." pattern
        let re = Regex::new(r"(winx\.\d+\.\d+\.\d+)").unwrap();

        for line in stdout.lines() {
            if let Some(captures) = re.captures(line) {
                if let Some(session_id) = captures.get(1) {
                    sessions.push(session_id.as_str().to_string());
                }
            }
        }

        Ok(sessions)
    }

    /// Find orphaned WINX screen sessions (parent PID is 1 or process doesn't exist)
    pub fn get_orphaned_winx_screens() -> WinxResult<Vec<String>> {
        let mut orphaned_sessions = Vec::new();
        let sessions = Self::get_winx_screen_sessions()?;

        for session in sessions {
            // Parse session format "winx.PID.timestamp"
            let parts: Vec<&str> = session.split('.').collect();
            if parts.len() >= 2 {
                let pid_str = parts[1];
                if let Ok(pid) = pid_str.parse::<u32>() {
                    // Check if process exists using platform-agnostic method
                    if !Self::is_process_running(pid) {
                        orphaned_sessions.push(session.clone());
                    } else {
                        // On macOS, we can't easily check for parent PID,
                        // so we'll consider any session without an active process as orphaned
                        #[cfg(target_os = "macos")]
                        {
                            // Use ps command to check if the process is orphaned
                            if let Ok(output) = Command::new("ps")
                                .args(["-o", "ppid=", "-p", &pid.to_string()])
                                .output()
                            {
                                if let Ok(ppid_str) = String::from_utf8(output.stdout) {
                                    if let Ok(ppid) = ppid_str.trim().parse::<u32>() {
                                        if ppid == 1 {
                                            orphaned_sessions.push(session.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(orphaned_sessions)
    }

    /// Check if a process is running using platform-agnostic method
    fn is_process_running(pid: u32) -> bool {
        #[cfg(target_os = "macos")]
        {
            // On macOS, use kill(0) to check if process exists
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, check if /proc/[pid] exists
            std::path::Path::new(&format!("/proc/{}", pid)).exists()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            // Fallback for other platforms
            false
        }
    }

    /// Clean up orphaned WINX screen sessions
    pub fn cleanup_orphaned_screens() -> WinxResult<()> {
        let orphaned = Self::get_orphaned_winx_screens()?;

        for session in orphaned {
            info!("Cleaning up orphaned screen session: {}", session);

            Command::new("screen")
                .args(["-S", &session, "-X", "quit"])
                .status()
                .map_err(|e| {
                    WinxError::bash_error(format!(
                        "Failed to kill screen session {}: {}",
                        session, e
                    ))
                })?;
        }

        Ok(())
    }

    /// Clean up all screen sessions matching a specific name pattern
    pub fn cleanup_screen_session(session_name: &str) -> WinxResult<()> {
        // Get all matching sessions
        let output = Command::new("screen")
            .arg("-ls")
            .output()
            .map_err(|e| WinxError::bash_error(format!("Failed to list screen sessions: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let pattern = format!(r"(\d+\.{})\s", regex::escape(session_name));
        let re = Regex::new(&pattern).unwrap();

        let mut sessions_to_kill = Vec::new();

        for line in stdout.lines() {
            if let Some(captures) = re.captures(line) {
                if let Some(session_id) = captures.get(1) {
                    sessions_to_kill.push(session_id.as_str().to_string());
                }
            }
        }

        // Kill each session
        for session in sessions_to_kill {
            warn!("Cleaning up screen session: {}", session);

            Command::new("screen")
                .args(["-S", &session, "-X", "quit"])
                .status()
                .map_err(|e| {
                    WinxError::bash_error(format!(
                        "Failed to kill screen session {}: {}",
                        session, e
                    ))
                })?;
        }

        Ok(())
    }

    /// Start a new screen session
    pub fn start_screen_session(session_name: &str, working_dir: &str) -> WinxResult<()> {
        debug!(
            "Starting screen session '{}' in directory '{}'",
            session_name, working_dir
        );

        // Create a detached screen session with bash
        let status = Command::new("screen")
            .args(["-dmS", session_name, "bash", "--noprofile", "--norc"])
            .current_dir(working_dir)
            .status()
            .map_err(|e| WinxError::bash_error(format!("Failed to start screen session: {}", e)))?;

        if !status.success() {
            return Err(WinxError::bash_error(format!(
                "Failed to create screen session '{}'. Exit code: {}",
                session_name,
                status.code().unwrap_or(-1)
            )));
        }

        info!("Screen session '{}' started successfully", session_name);

        // Initialize the shell environment in the screen session
        Self::send_to_screen(session_name, "export PS1='winx→ '")
            .and_then(|_| Self::send_to_screen(session_name, "export TERM=xterm-256color"))
            .and_then(|_| Self::send_to_screen(session_name, "clear"))?;

        Ok(())
    }

    /// Send a command to a screen session
    pub fn send_to_screen(session_name: &str, command: &str) -> WinxResult<()> {
        debug!("Sending command to screen '{}': {}", session_name, command);

        let status = Command::new("screen")
            .args(["-S", session_name, "-X", "stuff", &format!("{}\n", command)])
            .status()
            .map_err(|e| {
                WinxError::bash_error(format!("Failed to send command to screen: {}", e))
            })?;

        if !status.success() {
            return Err(WinxError::bash_error(format!(
                "Failed to send command to screen '{}'. Exit code: {}",
                session_name,
                status.code().unwrap_or(-1)
            )));
        }

        Ok(())
    }

    /// Check if a screen session exists
    pub fn screen_session_exists(session_name: &str) -> bool {
        let sessions = match Self::get_winx_screen_sessions() {
            Ok(sessions) => sessions,
            Err(_) => return false,
        };

        sessions.iter().any(|s| s.contains(session_name))
    }

    /// Execute a command in a screen session with shell quoting
    pub fn execute_in_screen(session_name: &str, command: &str) -> WinxResult<()> {
        // Handle complex commands that need proper shell interpretation
        let safe_command = if command.contains("&&")
            || command.contains("||")
            || command.contains(";")
            || command.contains("|")
            || command.contains(">")
            || command.contains("<")
        {
            // Escape and properly quote the command for shell interpretation
            format!("bash -c '{}'", command.replace("'", "'\\''"))
        } else {
            command.to_string()
        };

        debug!("Executing in screen '{}': {}", session_name, safe_command);
        Self::send_to_screen(session_name, &safe_command)
    }

    /// Attach to a screen session for interactive use
    pub fn attach_to_screen(session_name: &str) -> WinxResult<()> {
        info!("Attaching to screen session: {}", session_name);

        // Use -r for reattaching and -x for multi-attach
        let status = Command::new("screen")
            .args(["-x", session_name])
            .status()
            .map_err(|e| WinxError::bash_error(format!("Failed to attach to screen: {}", e)))?;

        if !status.success() {
            // Try with -r if -x fails
            let status_r = Command::new("screen")
                .args(["-r", session_name])
                .status()
                .map_err(|e| {
                    WinxError::bash_error(format!("Failed to reattach to screen: {}", e))
                })?;

            if !status_r.success() {
                return Err(WinxError::bash_error(format!(
                    "Failed to attach to screen session '{}'. Exit code: {}",
                    session_name,
                    status.code().unwrap_or(-1)
                )));
            }
        }

        Ok(())
    }

    /// Get the screen hardcopy (current screen content)
    pub fn get_screen_content(session_name: &str) -> WinxResult<String> {
        // Create a temporary file for hardcopy
        let temp_file = format!("/tmp/winx-screen-{}.txt", session_name);

        // Execute hardcopy command
        Self::send_to_screen(session_name, &format!("hardcopy {}", temp_file))?;

        // Wait a bit for the file to be written
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read the content
        let content = std::fs::read_to_string(&temp_file)
            .map_err(|e| WinxError::bash_error(format!("Failed to read screen content: {}", e)))?;

        // Clean up the temporary file
        let _ = std::fs::remove_file(&temp_file);

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_name() {
        let name1 = ScreenManager::generate_session_name();
        // Add a small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));
        let name2 = ScreenManager::generate_session_name();

        assert!(name1.starts_with("winx."));
        assert!(name2.starts_with("winx."));
        assert_ne!(name1, name2);
    }

    #[test]
    fn test_session_name_pattern() {
        let re = Regex::new(r"(winx\.\d+\.\d+\.\d+)").unwrap();

        let test_line = "winx.12345.123456.7\t(Detached)";
        let captures = re.captures(test_line);
        assert!(captures.is_some());

        if let Some(captures) = captures {
            assert_eq!(captures.get(1).unwrap().as_str(), "winx.12345.123456.7");
        }
    }
}
