//! CLI command definitions and parsing
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "yinx",
    version,
    author = "neur0map",
    about = "Intelligent penetration testing companion with AI-powered analysis",
    long_about = "Yinx is a background CLI daemon that captures terminal activity during penetration testing \
                  sessions, intelligently filters noise, semantically indexes findings, and provides instant \
                  retrieval with optional AI assistance."
)]
pub struct Cli {
    /// Global config file path (defaults to ~/.config/yinx/config.toml)
    #[arg(short, long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the Yinx daemon to capture terminal activity
    Start {
        /// Optional session name (defaults to timestamp)
        #[arg(short, long)]
        session: Option<String>,

        /// Profile to use (e.g., "exam", "accuracy", "fast")
        #[arg(short, long)]
        profile: Option<String>,
    },

    /// Stop the Yinx daemon
    Stop,

    /// Show daemon and current session status
    Status,

    /// Query captured data using semantic and keyword search
    Query {
        /// Search query text
        query: String,

        /// Maximum number of results to return
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Show only results from specific tool
        #[arg(short, long)]
        tool: Option<String>,

        /// Show results in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Ask a question with optional LLM assistance
    Ask {
        /// Question to ask
        question: String,

        /// Force offline mode (disable LLM even if configured)
        #[arg(long)]
        offline: bool,

        /// Number of context chunks to retrieve
        #[arg(short = 'n', long, default_value = "20")]
        context_size: usize,
    },

    /// Generate a penetration test report
    Report {
        /// Output file path (defaults to ~/.yinx/reports/<session>/report.md)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Report format
        #[arg(short, long, value_parser = ["markdown", "html", "json"], default_value = "markdown")]
        format: String,

        /// Session ID or name (defaults to current session)
        #[arg(short, long)]
        session: Option<String>,

        /// Include evidence files in export
        #[arg(long)]
        include_evidence: bool,
    },

    /// Export session data for sharing or backup
    Export {
        /// Output path for export archive
        output: PathBuf,

        /// Session ID or name (defaults to current session)
        #[arg(short, long)]
        session: Option<String>,

        /// Include vector and keyword indexes
        #[arg(long)]
        include_indexes: bool,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Internal commands (not for direct use)
    #[command(hide = true)]
    Internal {
        #[command(subcommand)]
        action: InternalAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum InternalAction {
    /// Capture command output and send to daemon
    Capture {
        /// Session ID
        #[arg(long)]
        session_id: String,

        /// Unix timestamp
        #[arg(long)]
        timestamp: i64,

        /// Command that was executed
        #[arg(long)]
        command: String,

        /// Path to file containing command output
        #[arg(long)]
        output_file: PathBuf,

        /// Exit code of the command
        #[arg(long)]
        exit_code: i32,

        /// Current working directory
        #[arg(long)]
        cwd: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show {
        /// Show only a specific section
        #[arg(short, long)]
        section: Option<String>,
    },

    /// Set a configuration value
    Set {
        /// Configuration key in dot notation (e.g., "llm.enabled")
        key: String,

        /// Value to set
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Configuration key in dot notation
        key: String,
    },

    /// Validate configuration file
    Validate {
        /// Path to config file (defaults to standard location)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Initialize default configuration
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Set active profile
    SetProfile {
        /// Profile name (e.g., "exam", "accuracy")
        profile: String,
    },
}

impl Cli {
    /// Parse CLI arguments from command line
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
