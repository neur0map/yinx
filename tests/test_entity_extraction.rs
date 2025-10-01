//! Interactive entity extraction test
//!
//! Run with: cargo run --example test_entity_extraction

use std::io::{self, Write};
use std::path::PathBuf;
use tempfile::TempDir;
use yinx::entities::{EntityExtractor, MetadataEnricher};
use yinx::patterns::PatternRegistry;
use yinx::storage::StorageManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║         Phase 5: Entity Extraction - Live Demo              ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Setup
    println!("⚙️  Setting up test environment...");
    let temp_dir = TempDir::new()?;
    let storage = StorageManager::new(temp_dir.path().to_path_buf())?;

    // Load patterns from config
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config-templates");
    let patterns = PatternRegistry::from_config_files(
        &config_dir.join("entities.toml"),
        &config_dir.join("tools.toml"),
        &config_dir.join("filters.toml"),
    )?;

    let extractor = EntityExtractor::new(patterns.clone());
    let mut enricher = MetadataEnricher::new();

    println!("✓ Loaded {} entity patterns", patterns.entities.len());
    println!("✓ Database ready at: {:?}\n", temp_dir.path());

    // Create test session
    let conn = storage.database.get_conn()?;
    conn.execute(
        "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params!["live-test", "Live Test Session", 1000, "active", 0, 0],
    )?;
    println!("✓ Created test session\n");

    // Interactive menu
    loop {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                    TESTING OPTIONS                           ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  1. Test with Nmap scan output                              ║");
        println!("║  2. Test with vulnerability scan                            ║");
        println!("║  3. Test with credential discovery                          ║");
        println!("║  4. Enter your own text to analyze                          ║");
        println!("║  5. View correlation graph                                  ║");
        println!("║  6. Query database entities                                 ║");
        println!("║  7. Show storage statistics                                 ║");
        println!("║  0. Exit                                                    ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        print!("\nSelect option: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => test_nmap(&extractor, &mut enricher, &storage)?,
            "2" => test_vuln_scan(&extractor, &mut enricher, &storage)?,
            "3" => test_credentials(&extractor, &mut enricher, &storage)?,
            "4" => test_custom_input(&extractor, &mut enricher, &storage)?,
            "5" => show_correlation_graph(&enricher)?,
            "6" => query_database(&storage)?,
            "7" => show_statistics(&storage)?,
            "0" => {
                println!("\n👋 Goodbye!");
                break;
            }
            _ => println!("❌ Invalid option, try again"),
        }
    }

    Ok(())
}

fn test_nmap(
    extractor: &EntityExtractor,
    enricher: &mut MetadataEnricher,
    storage: &StorageManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Testing with Nmap output...\n");

    let nmap_output = r#"
Starting Nmap 7.91 at 2025-01-01 10:00
Nmap scan report for target.example.com (192.168.1.100)
Host is up (0.0010s latency).
PORT     STATE SERVICE    VERSION
22/tcp   open  ssh        OpenSSH/8.2p1 Ubuntu
80/tcp   open  http       Apache/2.4.41 (Ubuntu)
443/tcp  open  ssl/https  nginx/1.18.0
3306/tcp open  mysql      MySQL/5.7.35
"#;

    println!("📄 Input:\n{}", nmap_output);
    analyze_and_store(nmap_output, "nmap", extractor, enricher, storage)?;

    Ok(())
}

fn test_vuln_scan(
    extractor: &EntityExtractor,
    enricher: &mut MetadataEnricher,
    storage: &StorageManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Testing with vulnerability scan...\n");

    let vuln_output = r#"
Vulnerability Report for 192.168.1.100

[CRITICAL] CVE-2021-44228 - Apache Log4j RCE
  Affected: Apache/2.4.41 on port 80/tcp

[HIGH] CVE-2021-3156 - Sudo Buffer Overflow
  Affected: OpenSSH/8.2p1

[MEDIUM] CVE-2020-11984 - mod_proxy_uwsgi
  Affected: Apache/2.4.41
"#;

    println!("📄 Input:\n{}", vuln_output);
    analyze_and_store(vuln_output, "nessus", extractor, enricher, storage)?;

    Ok(())
}

fn test_credentials(
    extractor: &EntityExtractor,
    enricher: &mut MetadataEnricher,
    storage: &StorageManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Testing with credential discovery...\n");

    let cred_output = r#"
Hydra v9.1 - Credential Discovery
Target: 192.168.1.100:22

[SUCCESS] login: admin   password: Welcome123!
[SUCCESS] login: backup  password: Backup2024

Config file /etc/mysql/my.cnf:
  user=root
  password=MyS3cr3tP@ss

AWS Key Found: AKIAIOSFODNN7EXAMPLE
"#;

    println!("📄 Input:\n{}", cred_output);
    analyze_and_store(cred_output, "hydra", extractor, enricher, storage)?;

    Ok(())
}

fn test_custom_input(
    extractor: &EntityExtractor,
    enricher: &mut MetadataEnricher,
    storage: &StorageManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n✏️  Enter your text (end with empty line):\n");

    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        lines.push(line);
    }

    let input = lines.join("");
    if input.is_empty() {
        println!("❌ No input provided");
        return Ok(());
    }

    analyze_and_store(&input, "custom", extractor, enricher, storage)?;

    Ok(())
}

fn analyze_and_store(
    text: &str,
    tool: &str,
    extractor: &EntityExtractor,
    enricher: &mut MetadataEnricher,
    storage: &StorageManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⚡ Extracting entities...");

    let start = std::time::Instant::now();
    let entities = extractor.extract(text);
    let duration = start.elapsed();

    println!("✓ Extracted {} entities in {:?}", entities.len(), duration);

    if entities.is_empty() {
        println!("ℹ️  No entities found in input");
        return Ok(());
    }

    // Show entities by type
    println!("\n📊 Entities found:");
    let entity_types = extractor.get_entity_types(text);
    for entity_type in &entity_types {
        let count = entities
            .iter()
            .filter(|e| e.entity_type == *entity_type)
            .count();
        let sample: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == *entity_type)
            .take(3)
            .map(|e| {
                if e.should_redact {
                    "[REDACTED]".to_string()
                } else {
                    e.value.clone()
                }
            })
            .collect();
        println!(
            "  • {}: {} (e.g., {})",
            entity_type,
            count,
            sample.join(", ")
        );
    }

    // Enrich metadata
    let metadata = enricher.enrich_capture(
        entities.clone(),
        Some(tool.to_string()),
        chrono::Utc::now().timestamp(),
    );

    println!("\n📋 Metadata:");
    println!("  • Tool: {:?}", metadata.tool);
    println!("  • Hosts: {:?}", metadata.hosts);
    println!("  • Vulnerabilities: {:?}", metadata.vulnerabilities);
    println!("  • Has sensitive data: {}", metadata.has_sensitive_data);

    // Store in database
    let (hash, _, _) = storage.blob_store.write(text.as_bytes())?;
    let conn = storage.database.get_conn()?;

    conn.execute(
        "INSERT INTO captures (session_id, timestamp, command, output_hash, tool, exit_code, cwd)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            "live-test",
            chrono::Utc::now().timestamp(),
            format!("{} scan", tool),
            &hash,
            tool,
            0,
            "/tmp"
        ],
    )?;
    let capture_id = conn.last_insert_rowid();

    // Store entities
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

    let inserted = storage
        .database
        .insert_entities(capture_id, &entity_records)?;
    println!(
        "\n💾 Stored {} entities in database (capture_id: {})",
        inserted, capture_id
    );

    Ok(())
}

fn show_correlation_graph(enricher: &MetadataEnricher) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🕸️  Correlation Graph\n");

    let stats = enricher.graph().stats();
    println!("📊 Statistics:");
    println!("  • Total hosts: {}", stats.host_count);
    println!("  • Total services: {}", stats.service_count);
    println!("  • Total vulnerabilities: {}", stats.vulnerability_count);
    println!("  • Open ports: {}", stats.total_ports);
    println!("  • Credentials found: {}", stats.total_credentials);

    if stats.host_count > 0 {
        println!("\n🖥️  Hosts:");
        for host_info in enricher.graph().get_all_hosts() {
            println!("\n  Host: {}", host_info.identifier);
            println!(
                "    • Ports: {:?}",
                host_info.ports.iter().collect::<Vec<_>>()
            );
            if !host_info.vulnerabilities.is_empty() {
                println!(
                    "    • Vulnerabilities: {:?}",
                    host_info.vulnerabilities.iter().take(3).collect::<Vec<_>>()
                );
            }
            println!("    • First seen: {}", host_info.first_seen);
            println!("    • Last seen: {}", host_info.last_seen);
        }
    }

    Ok(())
}

fn query_database(storage: &StorageManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Database Query\n");

    let conn = storage.database.get_conn()?;

    // Count entities by type
    println!("📊 Entities by type:");
    let mut stmt = conn.prepare(
        "SELECT type, COUNT(*) as count FROM entities GROUP BY type ORDER BY count DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows {
        let (entity_type, count) = row?;
        println!("  • {}: {}", entity_type, count);
    }

    // Show recent entities
    println!("\n📝 Recent entities (last 10):");
    let mut stmt =
        conn.prepare("SELECT type, value, confidence FROM entities ORDER BY id DESC LIMIT 10")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f32>(2)?,
        ))
    })?;

    for (i, row) in rows.enumerate() {
        let (entity_type, value, confidence) = row?;
        println!(
            "  {}. [{}] {} (confidence: {:.2})",
            i + 1,
            entity_type,
            value,
            confidence
        );
    }

    Ok(())
}

fn show_statistics(storage: &StorageManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📈 Storage Statistics\n");

    let stats = storage.stats()?;
    println!("🗄️  Database:");
    println!("  • Sessions: {}", stats.db.session_count);
    println!("  • Captures: {}", stats.db.capture_count);
    println!("  • Entities: {}", stats.db.entity_count);
    println!("  • Blobs: {}", stats.db.blob_count);
    println!("  • Chunks: {}", stats.db.chunk_count);

    println!("\n💾 Storage:");
    println!(
        "  • Machine zone: {}",
        yinx::storage::StorageStats::format_size(stats.machine_zone_size)
    );
    println!(
        "  • Human zone: {}",
        yinx::storage::StorageStats::format_size(stats.human_zone_size)
    );
    println!(
        "  • Total: {}",
        yinx::storage::StorageStats::format_size(stats.total_size())
    );

    Ok(())
}
