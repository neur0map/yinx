// Async processing pipeline with bounded channels for backpressure handling

use crate::daemon::ipc::IpcMessage;
use crate::error::Result;
use crate::patterns::PatternRegistry;
use crate::storage::StorageManager;
use chrono::Utc;
use rusqlite::params;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

/// Capture event to be processed through the pipeline
#[derive(Debug, Clone)]
pub struct CaptureEvent {
    pub session_id: String,
    pub timestamp: i64,
    pub command: String,
    pub output: String,
    pub exit_code: i32,
    pub cwd: String,
}

impl From<IpcMessage> for Option<CaptureEvent> {
    fn from(msg: IpcMessage) -> Self {
        match msg {
            IpcMessage::Capture {
                session_id,
                timestamp,
                command,
                output,
                exit_code,
                cwd,
            } => Some(CaptureEvent {
                session_id,
                timestamp,
                command,
                output,
                exit_code,
                cwd,
            }),
            _ => None,
        }
    }
}

/// Processing pipeline that receives captures and stores them
pub struct Pipeline {
    /// Channel for receiving capture events
    pub(super) capture_tx: mpsc::Sender<CaptureEvent>,
    /// Handle to the storage worker task
    storage_handle: Option<tokio::task::JoinHandle<()>>,
    /// Flush interval for time-based flushing
    flush_interval: Duration,
}

impl Pipeline {
    /// Create a new pipeline with the given configuration
    pub fn new(
        storage: Arc<StorageManager>,
        patterns: Arc<PatternRegistry>,
        buffer_size: usize,
        flush_interval_secs: u64,
    ) -> Self {
        let (capture_tx, capture_rx) = mpsc::channel(buffer_size);
        let flush_interval = Duration::from_secs(flush_interval_secs);

        // Spawn storage worker task
        let storage_handle = Some(tokio::spawn(async move {
            storage_worker(capture_rx, storage, patterns, flush_interval).await;
        }));

        Self {
            capture_tx,
            storage_handle,
            flush_interval,
        }
    }

    /// Send a capture event through the pipeline
    /// Returns an error if the channel is closed
    pub async fn send(&self, event: CaptureEvent) -> Result<()> {
        self.capture_tx
            .send(event)
            .await
            .map_err(|_| crate::error::YinxError::Daemon("Pipeline channel closed".to_string()))?;
        Ok(())
    }

    /// Shutdown the pipeline gracefully, draining pending captures
    pub async fn shutdown(mut self) {
        // Close the sender so worker knows to finish
        drop(self.capture_tx);

        // Wait for storage worker to finish processing
        if let Some(handle) = self.storage_handle.take() {
            tracing::info!("Waiting for pipeline to drain...");
            let _ = handle.await;
            tracing::info!("Pipeline drained successfully");
        }
    }

    /// Get the flush interval
    pub fn flush_interval(&self) -> Duration {
        self.flush_interval
    }
}

/// Storage worker that receives captures and writes them to storage
async fn storage_worker(
    mut capture_rx: mpsc::Receiver<CaptureEvent>,
    storage: Arc<StorageManager>,
    patterns: Arc<PatternRegistry>,
    flush_interval: Duration,
) {
    let mut flush_timer = time::interval(flush_interval);
    let mut pending_captures: Vec<CaptureEvent> = Vec::new();
    let mut stats = WorkerStats::default();

    loop {
        tokio::select! {
            // Receive capture event
            Some(event) = capture_rx.recv() => {
                pending_captures.push(event);

                // Flush if batch size threshold reached (100 captures)
                if pending_captures.len() >= 100 {
                    flush_batch(&mut pending_captures, &storage, &patterns, &mut stats).await;
                }
            }

            // Time-based flush
            _ = flush_timer.tick() => {
                if !pending_captures.is_empty() {
                    flush_batch(&mut pending_captures, &storage, &patterns, &mut stats).await;
                }
            }

            // Channel closed, drain remaining
            else => {
                if !pending_captures.is_empty() {
                    tracing::info!("Draining {} pending captures", pending_captures.len());
                    flush_batch(&mut pending_captures, &storage, &patterns, &mut stats).await;
                }
                tracing::info!(
                    "Storage worker finished: {} captures processed, {} errors",
                    stats.processed,
                    stats.errors
                );
                break;
            }
        }
    }
}

/// Flush a batch of captures to storage
async fn flush_batch(
    captures: &mut Vec<CaptureEvent>,
    storage: &StorageManager,
    patterns: &PatternRegistry,
    stats: &mut WorkerStats,
) {
    if captures.is_empty() {
        return;
    }

    tracing::debug!("Flushing {} captures to storage", captures.len());

    for capture in captures.drain(..) {
        if let Err(e) = process_capture(&capture, storage, patterns).await {
            tracing::error!("Failed to process capture: {}", e);
            stats.errors += 1;
        } else {
            stats.processed += 1;
        }
    }
}

/// Process a single capture: write blob and insert database record
async fn process_capture(
    event: &CaptureEvent,
    storage: &StorageManager,
    patterns: &PatternRegistry,
) -> Result<()> {
    // Write output to blob storage
    let (output_hash, compressed, _is_new) = storage.blob_store.write(event.output.as_bytes())?;

    // Detect tool from command using pattern registry
    let tool = patterns
        .detect_tool(&event.command)
        .map(|t| t.name.clone());

    // Insert capture record in database
    let conn = storage.database.get_conn()?;
    conn.execute(
        "INSERT INTO captures (session_id, timestamp, command, output_hash, tool, exit_code, cwd)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            &event.session_id,
            event.timestamp,
            &event.command,
            &output_hash,
            tool.as_deref(),
            event.exit_code,
            &event.cwd,
        ],
    )?;

    // Insert/update blob metadata
    let blob_size = event.output.len() as i64;
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO blobs (hash, size, created_at, compressed, ref_count)
         VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT(hash) DO UPDATE SET ref_count = ref_count + 1",
        params![&output_hash, blob_size, now, compressed],
    )?;

    // Update session capture count
    conn.execute(
        "UPDATE sessions SET capture_count = capture_count + 1 WHERE id = ?1",
        params![&event.session_id],
    )?;

    tracing::trace!(
        "Processed capture: session={}, command={}, hash={}",
        event.session_id,
        event.command,
        output_hash
    );

    Ok(())
}


/// Statistics for the storage worker
#[derive(Default)]
struct WorkerStats {
    processed: u64,
    errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{EntitiesConfig, FiltersConfig, ToolsConfig, Tier1Config, Tier2Config, Tier3Config};
    use tempfile::TempDir;

    fn create_test_patterns() -> Arc<PatternRegistry> {
        // Create minimal test configs
        let entities_config = EntitiesConfig { entity: vec![] };
        let tools_config = ToolsConfig { tool: vec![] };
        let filters_config = FiltersConfig {
            tier1: Tier1Config {
                max_occurrences: 3,
                normalization_patterns: vec![],
            },
            tier2: Tier2Config {
                entropy_weight: 0.3,
                uniqueness_weight: 0.3,
                technical_weight: 0.2,
                change_weight: 0.2,
                score_threshold_percentile: 0.8,
                technical_patterns: vec![],
                max_technical_score: 10.0,
            },
            tier3: Tier3Config {
                cluster_min_size: 2,
                max_cluster_size: 1000,
                representative_strategy: "highest_entropy".to_string(),
                cluster_patterns: vec![],
                preserve_metadata: vec![],
            },
        };

        Arc::new(
            PatternRegistry::from_configs(entities_config, tools_config, filters_config).unwrap(),
        )
    }

    #[tokio::test]
    async fn test_pipeline_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(StorageManager::new(temp_dir.path().to_path_buf()).unwrap());
        let patterns = create_test_patterns();

        let pipeline = Pipeline::new(storage, patterns, 1000, 5);
        assert_eq!(pipeline.flush_interval(), Duration::from_secs(5));

        // Clean shutdown
        pipeline.shutdown().await;
    }

    #[tokio::test]
    async fn test_pipeline_send_capture() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(StorageManager::new(temp_dir.path().to_path_buf()).unwrap());
        let patterns = create_test_patterns();

        // Create test session first
        let conn = storage.database.get_conn().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["test-session", "Test", 1000000, "active", 0, 0],
        )
        .unwrap();

        let pipeline = Pipeline::new(storage.clone(), patterns, 1000, 1);

        // Send a capture
        let event = CaptureEvent {
            session_id: "test-session".to_string(),
            timestamp: Utc::now().timestamp(),
            command: "nmap -sV 192.168.1.1".to_string(),
            output: "Nmap scan report...".to_string(),
            exit_code: 0,
            cwd: "/tmp".to_string(),
        };

        pipeline.send(event).await.unwrap();

        // Wait a bit for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Shutdown and drain
        pipeline.shutdown().await;

        // Verify capture was stored
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM captures", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
