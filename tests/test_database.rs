use rusqlite::params;
use std::path::PathBuf;
use yinx::storage::StorageManager;

fn main() {
    let test_dir = PathBuf::from("/tmp/yinx-storage-demo");

    // Clean up if exists
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).ok();
    }

    println!("📦 Creating storage at: {:?}\n", test_dir);
    let storage = StorageManager::new(test_dir.clone()).expect("Failed to create storage");

    println!("✓ Storage structure created:");
    println!("  - Machine zone: {:?}", test_dir.join("store"));
    println!("  - Human zone: {:?}", test_dir.join("reports"));
    println!("  - Database: {:?}\n", test_dir.join("store/db.sqlite"));

    // Write test data
    let test_captures = [
        ("nmap -sV 192.168.1.1", "Starting Nmap 7.80\nNmap scan report for 192.168.1.1\nHost is up (0.0012s latency).\nPORT   STATE SERVICE VERSION\n22/tcp open  ssh     OpenSSH 8.2p1\n80/tcp open  http    nginx 1.18.0", "nmap"),
        ("gobuster dir -u http://192.168.1.1 -w wordlist.txt", "===============================================================\nGobuster v3.1.0\nby OJ Reeves (@TheColonial) & Christian Mehlmauer (@firefart)\n===============================================================\n[+] Url:            http://192.168.1.1\n[+] Wordlist:       wordlist.txt\n===============================================================\n/admin (Status: 200)\n/login (Status: 200)\n/api (Status: 401)", "gobuster"),
        ("hydra -l admin -P passwords.txt 192.168.1.1 ssh", "Hydra v9.1 (c) 2020 by van Hauser/THC\n[DATA] max 16 tasks per 1 server\n[DATA] attacking ssh://192.168.1.1:22/\n[22][ssh] host: 192.168.1.1   login: admin   password: admin123\n1 of 1 target successfully completed, 1 valid password found", "hydra"),
    ];

    let conn = storage
        .database
        .get_conn()
        .expect("Failed to get connection");

    // Insert session
    conn.execute(
        "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            "demo-pentest-session",
            "Demo Penetration Test",
            1727812765,
            "active",
            3,
            3
        ],
    )
    .expect("Failed to insert session");

    println!("✓ Session created: Demo Penetration Test\n");

    // Insert captures and blobs
    for (idx, (command, output, tool)) in test_captures.iter().enumerate() {
        let (hash, compressed, _) = storage
            .blob_store
            .write(output.as_bytes())
            .expect("Failed to write blob");

        conn.execute(
            "INSERT INTO captures (session_id, timestamp, command, output_hash, tool, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "demo-pentest-session",
                1727812765 + idx as i64,
                command,
                &hash,
                tool,
                0
            ],
        )
        .expect("Failed to insert capture");

        conn.execute(
            "INSERT INTO blobs (hash, size, created_at, compressed, ref_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&hash, output.len() as i64, 1727812765, compressed, 1],
        )
        .expect("Failed to insert blob");

        let shard1 = &hash[0..2];
        let shard2 = &hash[2..4];
        println!("✓ Capture {}: {} (hash: {})", idx + 1, tool, hash);
        println!("  Command: {}", command);
        println!(
            "  Blob location: store/blobs/{}/{}/{}",
            shard1, shard2, hash
        );
        println!("  Compressed: {}", compressed);
        println!();
    }

    // Get stats
    let stats = storage.stats().expect("Failed to get stats");
    println!("=== Storage Statistics ===");
    println!("Sessions: {}", stats.db.session_count);
    println!("Captures: {}", stats.db.capture_count);
    println!("Blobs: {}", stats.db.blob_count);
    println!("Chunks: {}", stats.db.chunk_count);
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

    println!("\n✅ Test database created successfully!");
    println!("\n📍 Inspect with:");
    println!("   sqlite3 /tmp/yinx-storage-demo/store/db.sqlite");
    println!("\n📍 Example queries:");
    println!("   SELECT * FROM sessions;");
    println!("   SELECT command, tool, output_hash FROM captures;");
    println!("   SELECT hash, size, compressed FROM blobs;");
}
