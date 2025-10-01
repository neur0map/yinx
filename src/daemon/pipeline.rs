// Async processing pipeline with bounded channels for backpressure handling

use crate::daemon::ipc::IpcMessage;
use crate::entities::EntityExtractor;
use crate::error::Result;
use crate::filtering::FilterPipeline;
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
    /// Batch size for count-based flushing
    #[allow(dead_code)] // Used in storage_worker via move before spawn
    batch_size: usize,
    /// Filter pipeline for three-tier filtering
    #[allow(dead_code)] // Used in storage_worker via clone before spawn
    filter_pipeline: Arc<FilterPipeline>,
}

impl Pipeline {
    /// Create a new pipeline with the given configuration
    pub fn new(
        storage: Arc<StorageManager>,
        patterns: Arc<PatternRegistry>,
        buffer_size: usize,
        batch_size: usize,
        flush_interval_secs: u64,
    ) -> Self {
        let (capture_tx, capture_rx) = mpsc::channel(buffer_size);
        let flush_interval = Duration::from_secs(flush_interval_secs);

        // Create filter pipeline
        let filter_pipeline = Arc::new(FilterPipeline::new(patterns.clone()));

        // Spawn storage worker task
        let filter_pipeline_clone = filter_pipeline.clone();
        let storage_handle = Some(tokio::spawn(async move {
            storage_worker(
                capture_rx,
                storage,
                patterns,
                filter_pipeline_clone,
                flush_interval,
                batch_size,
            )
            .await;
        }));

        Self {
            capture_tx,
            storage_handle,
            flush_interval,
            batch_size,
            filter_pipeline,
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
    filter_pipeline: Arc<FilterPipeline>,
    flush_interval: Duration,
    batch_size: usize,
) {
    let mut flush_timer = time::interval(flush_interval);
    flush_timer.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    let mut pending_captures: Vec<CaptureEvent> = Vec::new();
    let mut stats = WorkerStats::default();

    loop {
        tokio::select! {
            // Receive capture event
            maybe_event = capture_rx.recv() => {
                match maybe_event {
                    Some(event) => {
                        pending_captures.push(event);

                        // Flush if batch size threshold reached (from config)
                        if pending_captures.len() >= batch_size {
                            flush_batch(&mut pending_captures, &storage, &patterns, &filter_pipeline, &mut stats).await;
                        }
                    }
                    None => {
                        // Channel closed, drain remaining
                        if !pending_captures.is_empty() {
                            tracing::info!("Draining {} pending captures", pending_captures.len());
                            flush_batch(&mut pending_captures, &storage, &patterns, &filter_pipeline, &mut stats).await;
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

            // Time-based flush
            _ = flush_timer.tick() => {
                if !pending_captures.is_empty() {
                    flush_batch(&mut pending_captures, &storage, &patterns, &filter_pipeline, &mut stats).await;
                }
            }
        }
    }
}

/// Flush a batch of captures to storage
async fn flush_batch(
    captures: &mut Vec<CaptureEvent>,
    storage: &StorageManager,
    patterns: &PatternRegistry,
    filter_pipeline: &FilterPipeline,
    stats: &mut WorkerStats,
) {
    if captures.is_empty() {
        return;
    }

    tracing::debug!("Flushing {} captures to storage", captures.len());

    for capture in captures.drain(..) {
        if let Err(e) = process_capture(&capture, storage, patterns, filter_pipeline).await {
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
    filter_pipeline: &FilterPipeline,
) -> Result<()> {
    // Write output to blob storage
    let (output_hash, compressed, _is_new) = storage.blob_store.write(event.output.as_bytes())?;

    // Detect tool from command using pattern registry
    let tool = patterns.detect_tool(&event.command).map(|t| t.name.clone());

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
    let capture_id = conn.last_insert_rowid();

    // Extract entities from output using PatternRegistry
    let extractor = EntityExtractor::new(patterns.clone());
    let entities = extractor.extract(&event.output);

    // Insert entities into database
    if !entities.is_empty() {
        let entity_records: Vec<(String, String, String, f32)> = entities
            .iter()
            .map(|e| {
                (
                    e.entity_type.clone(),
                    e.value.clone(),
                    e.context.clone(),
                    e.confidence,
                )
            })
            .collect();

        let entity_count = storage
            .database
            .insert_entities(capture_id, &entity_records)?;

        tracing::debug!(
            "Extracted {} entities from capture {} (types: {})",
            entity_count,
            capture_id,
            extractor.get_entity_types(&event.output).join(", ")
        );
    }

    // Run output through filtering pipeline
    let (clusters, filter_stats) =
        filter_pipeline.process_capture(&event.session_id, &event.output)?;

    tracing::debug!(
        "Filtered capture {}: {} lines → {} clusters ({:.1}% reduction) in {}ms",
        capture_id,
        filter_stats.input_lines,
        filter_stats.tier3_clusters,
        if filter_stats.input_lines > 0 {
            (1.0 - filter_stats.tier3_clusters as f32 / filter_stats.input_lines as f32) * 100.0
        } else {
            0.0
        },
        filter_stats.processing_time_ms
    );

    // Insert chunks for each cluster
    for cluster in clusters {
        let metadata_json =
            serde_json::to_string(&cluster.metadata).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO chunks (capture_id, blob_hash, representative_text, cluster_size, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                capture_id,
                &output_hash,
                &cluster.representative,
                cluster.size,
                &metadata_json,
            ],
        )?;
    }

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
        "Processed capture: session={}, command={}, hash={}, chunks={}, entities={}",
        event.session_id,
        event.command,
        output_hash,
        filter_stats.tier3_clusters,
        entities.len()
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
    use crate::patterns::{
        EntitiesConfig, FiltersConfig, Tier1Config, Tier2Config, Tier3Config, ToolsConfig,
    };
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_pipeline_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(StorageManager::new(temp_dir.path().to_path_buf()).unwrap());
        let patterns = create_test_patterns();

        // Use shorter interval for testing (1 second instead of 5)
        let pipeline = Pipeline::new(storage, patterns, 1000, 100, 1);
        assert_eq!(pipeline.flush_interval(), Duration::from_secs(1));

        // Clean shutdown
        pipeline.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
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

        // Use shorter flush interval for testing (100ms)
        let pipeline = Pipeline::new(storage.clone(), patterns, 1000, 100, 1);

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

        // Wait for processing (2 flush intervals to be safe)
        tokio::time::sleep(Duration::from_millis(2500)).await;

        // Shutdown and drain
        pipeline.shutdown().await;

        // Verify capture was stored
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM captures", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
