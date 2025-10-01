// Signal handling for graceful daemon shutdown

use crate::error::{Result, YinxError};
use tokio::signal::unix::{signal, Signal as TokioSignal, SignalKind};

/// Signal handler that manages multiple Unix signals
pub struct SignalHandler {
    sigterm: TokioSignal,
    sigint: TokioSignal,
    sighup: TokioSignal,
    sigusr1: TokioSignal,
}

impl SignalHandler {
    /// Create a new signal handler
    /// Sets up handlers for SIGTERM, SIGINT, SIGHUP, and SIGUSR1
    pub fn new() -> Result<Self> {
        let sigterm = signal(SignalKind::terminate()).map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to setup SIGTERM handler".to_string(),
        })?;
        let sigint = signal(SignalKind::interrupt()).map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to setup SIGINT handler".to_string(),
        })?;
        let sighup = signal(SignalKind::hangup()).map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to setup SIGHUP handler".to_string(),
        })?;
        let sigusr1 = signal(SignalKind::user_defined1()).map_err(|e| YinxError::Io {
            source: e,
            context: "Failed to setup SIGUSR1 handler".to_string(),
        })?;

        Ok(Self {
            sigterm,
            sigint,
            sighup,
            sigusr1,
        })
    }

    /// Wait for any signal to be received
    /// Returns a string indicating which signal was received
    pub async fn wait(&mut self) -> &'static str {
        tokio::select! {
            _ = self.sigterm.recv() => {
                tracing::info!("Received SIGTERM");
                "terminate"
            }
            _ = self.sigint.recv() => {
                tracing::info!("Received SIGINT");
                "interrupt"
            }
            _ = self.sighup.recv() => {
                tracing::info!("Received SIGHUP");
                "hangup"
            }
            _ = self.sigusr1.recv() => {
                tracing::info!("Received SIGUSR1");
                "usr1"
            }
        }
    }
}

/// Check if the signal should trigger shutdown
pub fn should_shutdown(sig: &str) -> bool {
    matches!(sig, "terminate" | "interrupt" | "hangup")
}

/// Check if the signal should trigger reload
pub fn should_reload(sig: &str) -> bool {
    matches!(sig, "usr1")
}
