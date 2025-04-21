use env_logger::{Builder, Env};
use std::io::Write;

/// Initialize logging with enhanced formatting and error tracking
pub fn init_logging() {
    // Verifique se já foi inicializado para evitar múltiplas inicializações
    if std::env::var("WINX_LOGGER_INITIALIZED").is_ok() {
        return;
    }

    // Set environment variable to avoid double initialization
    std::env::set_var("WINX_LOGGER_INITIALIZED", "true");

    // Create a custom environment that defaults to debug level if RUST_LOG is not defined
    let env = Env::default().filter_or("RUST_LOG", "debug");

    // Create and configure a custom builder
    let mut builder = Builder::from_env(env);

    // Configure custom format with timestamp, level, and module path
    builder.format(|buf, record| {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

        // Format with timestamp, module path and file/line information for better debugging
        let file_info = if record.level() <= log::Level::Debug {
            format!(
                " [{}:{}]",
                record.file().unwrap_or("<unknown>"),
                record.line().unwrap_or(0)
            )
        } else {
            String::new()
        };

        writeln!(
            buf,
            "[{} {} {}{}] {}",
            timestamp,
            record.level(),
            record.module_path().unwrap_or("<unknown>"),
            file_info,
            record.args()
        )
    });

    // Ensure output is flushed immediately for real-time debugging
    builder.write_style(env_logger::WriteStyle::Always);

    // Initialize the logger
    if let Err(e) = builder.try_init() {
        // If it fails, the logger is probably already initialized
        eprintln!("Warning: Logger initialization failed: {}", e);
        return;
    }

    // Create log file in user's home directory for debug
    let home_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Library/Logs/Claude/winx-debug.log");

    // Log startup message
    log::info!(
        "Winx Code Agent logging initialized at level: {:?}",
        log::max_level()
    );
    log::debug!("Debug log file location: {}", home_dir.to_string_lossy());

    // Log environment information
    if let Ok(workspace) = std::env::var("WINX_WORKSPACE") {
        log::info!(
            "WINX_WORKSPACE environment variable is set to: {}",
            workspace
        );
    } else {
        log::info!("WINX_WORKSPACE environment variable is not set");
    }

    // Log Claude configuration file location
    let claude_config = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Library/Application Support/Claude/claude_desktop_config.json");
    log::info!(
        "Claude configuration file: {}",
        claude_config.to_string_lossy()
    );
}

/// Log an error with context and source code location
#[macro_export]
macro_rules! log_error {
    ($err:expr, $context:expr) => {
        log::error!("{}: {} (at {}:{})", $context, $err, file!(), line!())
    };
    ($err:expr) => {
        log::error!("{} (at {}:{})", $err, file!(), line!())
    };
}

/// Log a warning with context and source code location
#[macro_export]
macro_rules! log_warn {
    ($msg:expr, $context:expr) => {
        log::warn!("{}: {} (at {}:{})", $context, $msg, file!(), line!())
    };
    ($msg:expr) => {
        log::warn!("{} (at {}:{})", $msg, file!(), line!())
    };
}

/// Track error details for development and debugging
#[macro_export]
macro_rules! track_error {
    ($result:expr, $context:expr) => {
        match $result {
            Ok(value) => Ok(value),
            Err(err) => {
                $crate::log_error!(err, $context);
                Err(err)
            }
        }
    };
}

/// Try a fallible operation with context, returning a WinxError on failure
#[macro_export]
macro_rules! try_with_context {
    ($result:expr, $context:expr, $err_fn:expr) => {
        match $result {
            Ok(value) => Ok(value),
            Err(err) => {
                let context_str = format!("{}: {}", $context, err);
                log::error!("{} (at {}:{})", context_str, file!(), line!());
                Err($err_fn(context_str))
            }
        }
    };
}
