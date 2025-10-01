use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;
use yinx::config::Config;
use yinx::daemon::{Daemon, IpcClient, IpcMessage};
use yinx::storage::StorageManager;

#[tokio::test]
async fn test_daemon_ipc_and_storage() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().to_path_buf();

    // Create minimal pattern config files for test
    let entities_file = base_path.join("entities.toml");
    let tools_file = base_path.join("tools.toml");
    let filters_file = base_path.join("filters.toml");

    std::fs::write(&entities_file, "entity = []\n").unwrap();
    std::fs::write(&tools_file, "tool = []\n").unwrap();
    std::fs::write(
        &filters_file,
        r#"
[tier1]
max_occurrences = 3
normalization_patterns = []

[tier2]
entropy_weight = 0.3
uniqueness_weight = 0.3
technical_weight = 0.2
change_weight = 0.2
score_threshold_percentile = 0.8
max_technical_score = 10.0
technical_patterns = []

[tier3]
cluster_min_size = 2
max_cluster_size = 1000
representative_strategy = "highest_entropy"
cluster_patterns = []
preserve_metadata = []
"#,
    )
    .unwrap();

    // Create storage
    let storage = StorageManager::new(base_path.clone()).unwrap();

    // Create test session
    let conn = storage.database.get_conn().unwrap();
    conn.execute(
        "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params!["test-session", "Test Session", 1000000, "active", 0, 0],
    )
    .unwrap();

    // Create test config
    let socket_path = base_path.join("test.sock");
    let pid_file = base_path.join("test.pid");
    let log_file = base_path.join("logs").join("daemon.log");

    let mut config = Config::default();
    config.daemon.socket_path = socket_path.clone();
    config.daemon.pid_file = pid_file.clone();
    config.daemon.log_file = log_file;
    config.storage.data_dir = base_path.clone();
    config.capture.buffer_size = 100;
    config.capture.batch_size = 10;
    config.capture.flush_interval = "1s".to_string(); // Fast flush for testing
    config.patterns.entities_file = entities_file;
    config.patterns.tools_file = tools_file;
    config.patterns.filters_file = filters_file;

    // Create and start daemon in foreground (not daemonized)
    let mut daemon = Daemon::new(config).unwrap();

    // Spawn daemon in background task
    let daemon_handle = tokio::spawn(async move { daemon.run_foreground().await });

    // Wait for daemon to start
    sleep(Duration::from_millis(100)).await;

    // Create IPC client
    let client = IpcClient::new(socket_path);

    // Test 1: Send status message
    let response = client
        .send(&IpcMessage::Status)
        .await
        .expect("Failed to send status message");
    assert!(response.success, "Status request should succeed");
    println!("✓ Status IPC test passed");

    // Test 2: Send capture message
    let capture_msg = IpcMessage::Capture {
        session_id: "test-session".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        command: "nmap -sV 192.168.1.1".to_string(),
        output: "Starting Nmap 7.80\nNmap scan report...".to_string(),
        exit_code: 0,
        cwd: "/tmp".to_string(),
    };

    let response = client
        .send(&capture_msg)
        .await
        .expect("Failed to send capture");
    assert!(response.success, "Capture should be queued");
    println!("✓ Capture IPC test passed");

    // Wait for capture to be processed (flush interval is 1s in test)
    println!("  Waiting for storage worker to process capture...");
    sleep(Duration::from_millis(1500)).await;

    // Verify capture was stored
    let storage = StorageManager::new(base_path).unwrap();
    let conn = storage.database.get_conn().unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM captures", [], |row| row.get(0))
        .unwrap();

    assert_eq!(count, 1, "Should have 1 capture stored");
    println!("✓ Storage integration test passed");

    // Cleanup: abort daemon
    daemon_handle.abort();

    println!("\n✅ All daemon integration tests passed!");
}

#[test]
fn test_process_manager() {
    use yinx::daemon::ProcessManager;

    let temp_dir = TempDir::new().unwrap();
    let pid_file = temp_dir.path().join("test.pid");
    let pm = ProcessManager::new(pid_file.clone());

    // Test: Not running initially
    assert!(!pm.is_running(), "Should not be running initially");

    // Test: Acquire lock
    pm.acquire().unwrap();
    assert!(pid_file.exists(), "PID file should exist");
    assert!(pm.is_running(), "Should be running after acquire");

    // Test: Cannot acquire twice
    let pm2 = ProcessManager::new(pid_file.clone());
    assert!(
        pm2.acquire().is_err(),
        "Should not be able to acquire twice"
    );

    // Test: Release
    pm.release().unwrap();
    assert!(!pid_file.exists(), "PID file should be removed");
    assert!(!pm.is_running(), "Should not be running after release");

    println!("✅ Process manager tests passed!");
}
