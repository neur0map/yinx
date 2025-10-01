use yinx::cli::{Cli, Commands, ConfigAction, InternalAction};
use yinx::config::Config;
use yinx::daemon::{Daemon, IpcClient, IpcMessage, ProcessManager};
use yinx::error::{Result, YinxError};
use yinx::session::SessionManager;

fn main() -> Result<()> {
    // Initialize logging
    init_logging();

    // Parse CLI arguments
    let cli = Cli::parse_args();

    // Handle commands
    match cli.command {
        Commands::Start { session, profile } => {
            cmd_start(cli.config, session, profile)?;
        }
        Commands::Stop => {
            cmd_stop()?;
        }
        Commands::Status => {
            cmd_status(cli.config)?;
        }
        Commands::Query {
            query,
            limit,
            tool,
            json,
        } => {
            cmd_query(&query, limit, tool, json)?;
        }
        Commands::Ask {
            question,
            offline,
            context_size,
        } => {
            cmd_ask(&question, offline, context_size)?;
        }
        Commands::Report {
            output,
            format,
            session,
            include_evidence,
        } => {
            cmd_report(output, &format, session, include_evidence)?;
        }
        Commands::Export {
            output,
            session,
            include_indexes,
        } => {
            cmd_export(&output, session, include_indexes)?;
        }
        Commands::Config { action } => {
            cmd_config(cli.config, action)?;
        }
        Commands::Internal { action } => {
            cmd_internal(action)?;
        }
    }

    Ok(())
}

fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("yinx=info"));

    fmt().with_env_filter(filter).with_target(false).init();
}

fn cmd_start(
    config_path: Option<std::path::PathBuf>,
    session: Option<String>,
    profile: Option<String>,
) -> Result<()> {
    tracing::info!("Starting yinx daemon...");

    // Load configuration
    let config = load_config(config_path, profile)?;

    tracing::info!("Configuration loaded successfully");

    // Initialize session manager
    let data_dir = expand_path(&config.storage.data_dir)?;
    let mut session_manager = SessionManager::new(data_dir);

    // Create new session
    let session = session_manager.create_session(session)?;

    println!("✓ Starting yinx daemon...");
    println!("  Session: {} ({})", session.name, session.id);
    println!(
        "  Started: {}",
        session.started_at.format("%Y-%m-%d %H:%M:%S")
    );

    // Start daemon (this will fork - parent exits, child continues)
    let mut daemon = Daemon::new(config)?;
    daemon.start_daemon()?;

    // This line is never reached in parent (parent exits in daemon.start())
    // Child process continues as daemon

    Ok(())
}

fn cmd_stop() -> Result<()> {
    use std::thread::sleep;
    use std::time::Duration;

    // Load config to get daemon settings
    let config = load_config(None, None)?;
    let pid_file = expand_path(&config.daemon.pid_file)?;

    // Check daemon status
    let pm = ProcessManager::new(pid_file);
    if !pm.is_running() {
        println!("Daemon is not running");
        return Ok(());
    }

    // Try graceful shutdown first (SIGTERM)
    println!("Sending SIGTERM to daemon...");
    pm.signal(nix::sys::signal::Signal::SIGTERM)?;

    // Wait for daemon to stop gracefully
    for i in 0..5 {
        sleep(Duration::from_millis(500));
        if !pm.is_running() {
            println!("✓ Daemon stopped gracefully");
            return Ok(());
        }
        if i < 4 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    }

    // If still running, force kill with SIGKILL
    if pm.is_running() {
        println!("\nDaemon not responding, sending SIGKILL...");
        pm.signal(nix::sys::signal::Signal::SIGKILL)?;
        sleep(Duration::from_millis(500));

        if !pm.is_running() {
            println!("✓ Daemon force killed");
        } else {
            println!("⚠ Warning: Daemon may still be running (PID file stale)");
        }
    }

    Ok(())
}

fn cmd_status(config_path: Option<std::path::PathBuf>) -> Result<()> {
    let config = load_config(config_path, None)?;
    let pid_file = expand_path(&config.daemon.pid_file)?;
    let data_dir = expand_path(&config.storage.data_dir)?;
    let session_manager = SessionManager::new(data_dir);

    // Check daemon status
    let pm = ProcessManager::new(pid_file);
    let daemon_status = if pm.is_running() {
        if let Ok(pid) = pm.read_pid() {
            format!("Running (PID: {})", pid)
        } else {
            "Running".to_string()
        }
    } else {
        "Stopped".to_string()
    };

    println!("Yinx Status");
    println!("===========");
    println!("\nDaemon: {}", daemon_status);

    // List sessions
    let sessions = session_manager.list_sessions()?;
    println!("\nSessions: {} total", sessions.len());

    if !sessions.is_empty() {
        println!("\nRecent sessions:");
        for session in sessions.iter().take(5) {
            println!(
                "  {} - {} ({})",
                session.name,
                session.status_str(),
                session.started_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
    }

    Ok(())
}

fn cmd_query(_query: &str, _limit: usize, _tool: Option<String>, _json: bool) -> Result<()> {
    println!("Query functionality will be available in Phase 6-7");
    Ok(())
}

fn cmd_ask(_question: &str, _offline: bool, _context_size: usize) -> Result<()> {
    println!("Ask functionality will be available in Phase 8");
    Ok(())
}

fn cmd_report(
    _output: Option<std::path::PathBuf>,
    _format: &str,
    _session: Option<String>,
    _include_evidence: bool,
) -> Result<()> {
    println!("Report generation will be available in Phase 9");
    Ok(())
}

fn cmd_export(
    _output: &std::path::Path,
    _session: Option<String>,
    _include_indexes: bool,
) -> Result<()> {
    println!("Export functionality will be available in Phase 9");
    Ok(())
}

fn cmd_internal(action: InternalAction) -> Result<()> {
    match action {
        InternalAction::Capture {
            session_id,
            timestamp,
            command,
            output_file,
            exit_code,
            cwd,
        } => {
            // Read output from file
            let output = std::fs::read_to_string(&output_file).unwrap_or_default();

            // Load config to get socket path
            let config = load_config(None, None)?;
            let socket_path = expand_path(&config.daemon.socket_path)?;

            // Create IPC client and send capture message
            let client = IpcClient::new(socket_path);
            let message = IpcMessage::Capture {
                session_id,
                timestamp,
                command,
                output,
                exit_code,
                cwd,
            };

            // Send message (this is async so we need tokio runtime)
            let rt = tokio::runtime::Runtime::new().map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to create tokio runtime".to_string(),
            })?;
            rt.block_on(async {
                match client.send(&message).await {
                    Ok(_response) => {
                        // Success - cleanup output file
                        let _ = std::fs::remove_file(&output_file);
                        Ok(())
                    }
                    Err(e) => {
                        tracing::debug!("Failed to send capture: {}", e);
                        // Still cleanup output file even on error
                        let _ = std::fs::remove_file(&output_file);
                        Err(e)
                    }
                }
            })
        }
    }
}

fn cmd_config(config_path: Option<std::path::PathBuf>, action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show { section } => {
            let config = load_config(config_path, None)?;
            let json = serde_json::to_string_pretty(&config).map_err(|e| YinxError::Json {
                source: e,
                context: "Failed to serialize config".to_string(),
            })?;

            if let Some(section) = section {
                println!("Section: {}", section);
                println!("Note: Section filtering not yet implemented");
            }

            println!("{}", json);
        }
        ConfigAction::Set { key, value } => {
            println!("Setting {key} = {value}");
            println!("Note: Config modification not yet implemented");
        }
        ConfigAction::Get { key } => {
            println!("Getting value for: {key}");
            println!("Note: Config get not yet implemented");
        }
        ConfigAction::Validate { file } => {
            let path = file.unwrap_or_else(|| Config::default_path().unwrap());
            let config = Config::load(&path)?;
            println!("✓ Configuration is valid");
            println!("  Schema version: {}", config.meta.schema_version);
        }
        ConfigAction::Init { force } => {
            let path = Config::default_path()?;

            if path.exists() && !force {
                println!("Configuration file already exists at: {}", path.display());
                println!("Use --force to overwrite");
                return Ok(());
            }

            // Create parent directory
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| YinxError::Io {
                    source: e,
                    context: format!("Failed to create config directory: {:?}", parent),
                })?;
            }

            // Save default config
            let config = Config::default();
            config.save(&path)?;

            println!("✓ Configuration initialized at: {}", path.display());

            // Copy pattern template files
            let config_dir = path.parent().unwrap();
            copy_pattern_templates(config_dir, force)?;

            println!("✓ Pattern configuration files installed");
            println!("  - entities.toml: Entity extraction patterns");
            println!("  - tools.toml: Tool detection patterns");
            println!("  - filters.toml: Filtering configuration");
        }
        ConfigAction::SetProfile { profile } => {
            println!("Setting active profile to: {profile}");
            println!("Note: Profile management not yet implemented");
        }
    }

    Ok(())
}

fn load_config(config_path: Option<std::path::PathBuf>, profile: Option<String>) -> Result<Config> {
    let path = config_path.unwrap_or_else(|| Config::default_path().unwrap());

    if !path.exists() {
        tracing::warn!(
            "Config file not found, using defaults. Run 'yinx config init' to create one."
        );
        return Ok(Config::default());
    }

    if let Some(profile) = profile {
        Config::load_with_profile(&path, &profile)
    } else {
        Config::load(&path)
    }
}

fn copy_pattern_templates(config_dir: &std::path::Path, force: bool) -> Result<()> {
    // Check if template files exist in config-templates/ directory
    // If not, we'll create minimal default templates
    let repo_root = std::env::current_dir().ok();

    let entities_path = config_dir.join("entities.toml");
    let tools_path = config_dir.join("tools.toml");
    let filters_path = config_dir.join("filters.toml");

    // Try to copy from config-templates/ if available
    if let Some(root) = repo_root {
        let template_dir = root.join("config-templates");

        if template_dir.exists() {
            // Copy from templates
            if force || !entities_path.exists() {
                std::fs::copy(template_dir.join("entities.toml"), &entities_path).ok();
            }
            if force || !tools_path.exists() {
                std::fs::copy(template_dir.join("tools.toml"), &tools_path).ok();
            }
            if force || !filters_path.exists() {
                std::fs::copy(template_dir.join("filters.toml"), &filters_path).ok();
            }
            return Ok(());
        }
    }

    // Fallback: Create minimal default files
    if force || !entities_path.exists() {
        let entities_content = include_str!("../config-templates/entities.toml");
        std::fs::write(&entities_path, entities_content).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to write entities.toml: {:?}", entities_path),
        })?;
    }

    if force || !tools_path.exists() {
        let tools_content = include_str!("../config-templates/tools.toml");
        std::fs::write(&tools_path, tools_content).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to write tools.toml: {:?}", tools_path),
        })?;
    }

    if force || !filters_path.exists() {
        let filters_content = include_str!("../config-templates/filters.toml");
        std::fs::write(&filters_path, filters_content).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to write filters.toml: {:?}", filters_path),
        })?;
    }

    Ok(())
}

fn expand_path(path: &std::path::Path) -> Result<std::path::PathBuf> {
    let path_str = path
        .to_str()
        .ok_or_else(|| yinx::YinxError::Config("Invalid path encoding".to_string()))?;

    if let Some(stripped) = path_str.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or_else(|| {
            yinx::YinxError::Config("Cannot determine home directory".to_string())
        })?;
        Ok(home.join(stripped))
    } else {
        Ok(path.to_path_buf())
    }
}

// Extension trait for SessionStatus
trait SessionStatusExt {
    fn status_str(&self) -> &str;
}

impl SessionStatusExt for yinx::session::Session {
    fn status_str(&self) -> &str {
        match self.status {
            yinx::session::SessionStatus::Active => "Active",
            yinx::session::SessionStatus::Paused => "Paused",
            yinx::session::SessionStatus::Stopped => "Stopped",
        }
    }
}
