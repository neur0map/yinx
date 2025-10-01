use rusqlite::params;
use yinx::storage::StorageManager;

#[test]
fn test_real_storage_creation() {
    // Use a test directory instead of ~/.yinx to avoid conflicts
    let home = dirs::home_dir().expect("Could not find home directory");
    let test_dir = home.join(".yinx-test");

    // Clean up any existing test directory
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).ok();
    }

    println!("Creating storage at: {:?}", test_dir);

    // Create storage manager
    let storage = StorageManager::new(test_dir.clone()).expect("Failed to create storage");

    // Verify directory structure
    assert!(test_dir.join("store").exists(), "Machine zone should exist");
    assert!(test_dir.join("reports").exists(), "Human zone should exist");
    assert!(
        test_dir.join("store/blobs").exists(),
        "Blobs directory should exist"
    );
    assert!(
        test_dir.join("store/vectors").exists(),
        "Vectors directory should exist"
    );
    assert!(
        test_dir.join("store/keywords").exists(),
        "Keywords directory should exist"
    );

    let db_path = test_dir.join("store/db.sqlite");
    assert!(db_path.exists(), "Database file should exist");

    println!("✓ Database created at: {:?}", db_path);

    // Test writing a blob
    let test_data =
        b"nmap -sV 192.168.1.1\nStarting Nmap 7.80\nNmap scan report for 192.168.1.1\nHost is up";
    let (hash, compressed, is_new) = storage
        .blob_store
        .write(test_data)
        .expect("Failed to write blob");

    assert!(is_new, "Should be new blob");
    println!("✓ Blob written with hash: {}", hash);

    // Verify blob file exists in sharded location
    let shard1 = &hash[0..2];
    let shard2 = &hash[2..4];
    let blob_path = test_dir.join(format!("store/blobs/{}/{}/{}", shard1, shard2, hash));
    assert!(
        blob_path.exists(),
        "Blob file should exist at {:?}",
        blob_path
    );
    println!("✓ Blob file exists at: {:?}", blob_path);

    // Test database operations
    let conn = storage
        .database
        .get_conn()
        .expect("Failed to get connection");

    // Insert test session
    conn.execute(
        "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            "real-test-session",
            "Real Integration Test",
            1727812765,
            "active",
            1,
            1
        ],
    )
    .expect("Failed to insert session");

    println!("✓ Session inserted into database");

    // Insert test capture
    conn.execute(
        "INSERT INTO captures (session_id, timestamp, command, output_hash, tool, exit_code)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            "real-test-session",
            1727812766,
            "nmap -sV 192.168.1.1",
            &hash,
            "nmap",
            0
        ],
    )
    .expect("Failed to insert capture");

    println!("✓ Capture inserted into database");

    // Insert blob metadata
    conn.execute(
        "INSERT INTO blobs (hash, size, created_at, compressed, ref_count)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![&hash, test_data.len() as i64, 1727812765, compressed, 1],
    )
    .expect("Failed to insert blob metadata");

    println!("✓ Blob metadata inserted into database");

    // Query back the data
    let (session_name, capture_count): (String, i64) = conn
        .query_row(
            "SELECT name, capture_count FROM sessions WHERE id = ?1",
            params!["real-test-session"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Failed to query session");

    assert_eq!(session_name, "Real Integration Test");
    println!(
        "✓ Session query successful: {} (captures: {})",
        session_name, capture_count
    );

    // Query captures joined with blobs
    let (command, blob_size): (String, i64) = conn
        .query_row(
            "SELECT c.command, b.size FROM captures c
         JOIN blobs b ON c.output_hash = b.hash
         WHERE c.session_id = ?1",
            params!["real-test-session"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("Failed to query capture with blob");

    assert_eq!(command, "nmap -sV 192.168.1.1");
    println!(
        "✓ Join query successful: {} (size: {} bytes)",
        command, blob_size
    );

    // Test storage stats
    let stats = storage.stats().expect("Failed to get stats");
    println!("\n=== Storage Statistics ===");
    println!("Sessions: {}", stats.db.session_count);
    println!("Captures: {}", stats.db.capture_count);
    println!("Blobs: {}", stats.db.blob_count);
    println!("Entities: {}", stats.db.entity_count);
    println!(
        "Machine zone size: {}",
        yinx::storage::StorageStats::format_size(stats.machine_zone_size)
    );
    println!(
        "Human zone size: {}",
        yinx::storage::StorageStats::format_size(stats.human_zone_size)
    );
    println!(
        "Total size: {}",
        yinx::storage::StorageStats::format_size(stats.total_size())
    );

    assert_eq!(stats.db.session_count, 1);
    assert_eq!(stats.db.capture_count, 1);
    assert_eq!(stats.db.blob_count, 1);

    println!("\n✅ All integration tests passed!");

    // Clean up
    std::fs::remove_dir_all(&test_dir).ok();
    println!("✓ Test directory cleaned up");
}
