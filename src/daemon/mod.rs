// Daemon module: background process management for terminal capture

mod ipc;
mod pipeline;
mod process;
mod signals;

pub use ipc::{IpcClient, IpcMessage, IpcResponse, IpcServer};
pub use pipeline::{CaptureEvent, Pipeline};
pub use process::ProcessManager;
pub use signals::SignalHandler;

use crate::config::Config;
use crate::error::{Result, YinxError};
use crate::patterns::PatternRegistry;
use crate::storage::StorageManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task;

/// Main daemon struct that manages the lifecycle and components
pub struct Daemon {
    config: Config,
    process_manager: ProcessManager,
    storage: Arc<StorageManager>,
    patterns: Arc<PatternRegistry>,
    pipeline: Option<Pipeline>,
    ipc_server: Option<IpcServer>,
}

impl Daemon {
    /// Create a new daemon instance
    pub fn new(config: Config) -> Result<Self> {
        // Expand tilde in data_dir if needed
        let data_dir = expand_tilde(&config.storage.data_dir);

        // Initialize storage
        let storage = Arc::new(StorageManager::new(data_dir.clone())?);

        // Initialize process manager
        let pid_file = expand_tilde(&config.daemon.pid_file);
        let process_manager = ProcessManager::new(pid_file);

        // Load pattern registry from config files
        let entities_path = expand_tilde(&config.patterns.entities_file);
        let tools_path = expand_tilde(&config.patterns.tools_file);
        let filters_path = expand_tilde(&config.patterns.filters_file);

        let patterns = Arc::new(
            PatternRegistry::from_config_files(&entities_path, &tools_path, &filters_path)
                .map_err(|e| {
                    tracing::warn!("Failed to load pattern registry: {}", e);
                    tracing::warn!("Using default/empty patterns. Run 'yinx config init' to install pattern files.");
                    e
                })?,
        );

        Ok(Self {
            config,
            process_manager,
            storage,
            patterns,
            pipeline: None,
            ipc_server: None,
        })
    }

    /// Start the daemon in the foreground (for testing)
    pub async fn run_foreground(&mut self) -> Result<()> {
        // Acquire PID and lock
        self.process_manager.acquire()?;

        tracing::info!("Daemon starting in foreground mode");

        // Ensure cleanup on exit
        let pm = self.process_manager.clone();
        let cleanup = move || {
            if let Err(e) = pm.release() {
                tracing::error!("Failed to cleanup on exit: {}", e);
            }
        };

        // Setup signal handler
        let mut signal_handler = SignalHandler::new()?;

        // Start IPC server
        let socket_path = expand_tilde(&self.config.daemon.socket_path);
        let mut ipc_server = IpcServer::new(socket_path);
        ipc_server.bind().await?;

        // Start pipeline
        let pipeline = Pipeline::new(
            self.storage.clone(),
            self.patterns.clone(),
            self.config.capture.buffer_size,
            parse_flush_interval(&self.config.capture.flush_interval),
        );

        self.pipeline = Some(pipeline);
        self.ipc_server = Some(ipc_server);

        tracing::info!("Daemon started successfully");

        // Main event loop
        loop {
            tokio::select! {
                // Accept IPC connections
                Ok(stream) = self.ipc_server.as_mut().unwrap().accept() => {
                    let pipeline = self.pipeline.as_ref().unwrap().clone_sender();
                    task::spawn(async move {
                        if let Err(e) = handle_client(stream, pipeline).await {
                            tracing::error!("Client handler error: {}", e);
                        }
                    });
                }

                // Handle signals
                sig = signal_handler.wait() => {
                    if signals::should_shutdown(sig) {
                        tracing::info!("Shutdown signal received");
                        break;
                    } else if signals::should_reload(sig) {
                        tracing::info!("Reload signal received (not implemented yet)");
                    }
                }
            }
        }

        // Shutdown
        self.shutdown().await?;
        cleanup();

        Ok(())
    }

    /// Start the daemon as a background process
    pub fn start_daemon(&mut self) -> Result<()> {
        // Check if already running
        if self.process_manager.is_running() {
            return Err(YinxError::Daemon("Daemon is already running".to_string()));
        }

        // Ensure log directory exists FIRST
        if let Some(parent) = self.config.daemon.log_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to create log directory: {:?}", parent),
            })?;
        }

        // Setup daemonization log files
        let stdout_path = std::fs::File::create(
            self.config.daemon.log_file.with_extension("stdout"),
        )
        .map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to create stdout log file".to_string(),
        })?;
        let stderr_path = std::fs::File::create(
            self.config.daemon.log_file.with_extension("stderr"),
        )
        .map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to create stderr log file".to_string(),
        })?;

        let daemon = daemonize::Daemonize::new()
            .pid_file(&self.config.daemon.pid_file)
            .working_directory(std::env::current_dir().map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to get current directory".to_string(),
            })?)
            .stdout(stdout_path)
            .stderr(stderr_path);

        // Fork and daemonize
        daemon
            .start()
            .map_err(|e| YinxError::Daemon(format!("Failed to daemonize: {}", e)))?;

        // In the daemon process now, start the runtime
        let runtime = tokio::runtime::Runtime::new().map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to create tokio runtime".to_string(),
        })?;
        runtime.block_on(async {
            if let Err(e) = self.run_foreground().await {
                tracing::error!("Daemon error: {}", e);
            }
        });

        Ok(())
    }

    /// Stop the daemon
    pub fn stop_daemon(&self) -> Result<()> {
        if !self.process_manager.is_running() {
            return Err(YinxError::Daemon("Daemon is not running".to_string()));
        }

        // Send SIGTERM
        self.process_manager
            .signal(nix::sys::signal::Signal::SIGTERM)?;

        tracing::info!("Sent shutdown signal to daemon");

        Ok(())
    }

    /// Get daemon status
    pub fn status(&self) -> DaemonStatus {
        if self.process_manager.is_running() {
            let pid = self.process_manager.read_pid().ok();
            DaemonStatus::Running { pid }
        } else {
            DaemonStatus::Stopped
        }
    }

    /// Shutdown the daemon gracefully
    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down daemon");

        // Shutdown pipeline (drains pending captures)
        if let Some(pipeline) = self.pipeline.take() {
            pipeline.shutdown().await;
        }

        // Shutdown IPC server
        if let Some(ipc_server) = self.ipc_server.take() {
            ipc_server.shutdown()?;
        }

        tracing::info!("Daemon shutdown complete");

        Ok(())
    }
}

impl Pipeline {
    /// Clone the sender for use in other tasks
    fn clone_sender(&self) -> tokio::sync::mpsc::Sender<CaptureEvent> {
        self.capture_tx.clone()
    }
}

/// Handle a client connection
async fn handle_client(
    mut stream: tokio::net::UnixStream,
    pipeline: tokio::sync::mpsc::Sender<CaptureEvent>,
) -> Result<()> {
    // Read message
    let message = ipc::read_message(&mut stream).await?;

    // Process message
    let response = match message {
        IpcMessage::Capture { .. } => {
            if let Some(event) = Option::<CaptureEvent>::from(message) {
                match pipeline.send(event).await {
                    Ok(_) => IpcResponse::success("Capture queued"),
                    Err(e) => IpcResponse::error(format!("Failed to queue capture: {}", e)),
                }
            } else {
                IpcResponse::error("Invalid capture message")
            }
        }
        IpcMessage::Status => IpcResponse::success("Daemon is running"),
        IpcMessage::Stop => IpcResponse::success("Shutdown initiated"),
        IpcMessage::Query { .. } => IpcResponse::error("Query not implemented yet (Phase 8)"),
    };

    // Write response
    ipc::write_response(&mut stream, &response).await?;

    Ok(())
}

/// Daemon status
#[derive(Debug, Clone)]
pub enum DaemonStatus {
    Running { pid: Option<i32> },
    Stopped,
}

/// Expand tilde in path
fn expand_tilde(path: &Path) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.strip_prefix("~").unwrap());
        }
    }
    path.to_path_buf()
}

/// Parse flush interval string (e.g., "5s", "100ms")
fn parse_flush_interval(interval: &str) -> u64 {
    let interval = interval.trim();

    // Check "ms" before "s" because "ms" ends with "s"
    if let Some(ms) = interval.strip_suffix("ms") {
        ms.parse::<u64>().unwrap_or(5000) / 1000
    } else if let Some(secs) = interval.strip_suffix("s") {
        secs.parse().unwrap_or(5)
    } else {
        interval.parse().unwrap_or(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flush_interval() {
        assert_eq!(parse_flush_interval("5s"), 5);
        assert_eq!(parse_flush_interval("10s"), 10);
        assert_eq!(parse_flush_interval("1000ms"), 1);
        assert_eq!(parse_flush_interval("5000ms"), 5);
        assert_eq!(parse_flush_interval("7"), 7);
    }

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let path = PathBuf::from("~/.yinx");
        let expanded = expand_tilde(&path);
        assert_eq!(expanded, home.join(".yinx"));

        let path = PathBuf::from("/tmp/yinx");
        let expanded = expand_tilde(&path);
        assert_eq!(expanded, PathBuf::from("/tmp/yinx"));
    }
}
