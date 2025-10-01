// Integration test for filtering pipeline with realistic data
use std::sync::Arc;
use yinx::filtering::FilterPipeline;
use yinx::patterns::{
    EntitiesConfig, FiltersConfig, NormalizationPattern, PatternRegistry, TechnicalPattern,
    Tier1Config, Tier2Config, Tier3Config, ToolsConfig,
};

fn create_patterns() -> Arc<PatternRegistry> {
    let entities = EntitiesConfig { entity: vec![] };
    let tools = ToolsConfig { tool: vec![] };

    let filters = FiltersConfig {
        tier1: Tier1Config {
            max_occurrences: 3,
            normalization_patterns: vec![
                NormalizationPattern {
                    name: "ip_address".to_string(),
                    pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                    replacement: "__IP__".to_string(),
                    priority: 1,
                },
                NormalizationPattern {
                    name: "port".to_string(),
                    pattern: r"\b\d{1,5}/(tcp|udp)".to_string(),
                    replacement: "__PORT__".to_string(),
                    priority: 2,
                },
            ],
        },
        tier2: Tier2Config {
            entropy_weight: 0.25,
            uniqueness_weight: 0.25,
            technical_weight: 0.25,
            change_weight: 0.25,
            score_threshold_percentile: 0.7, // Keep top 30%
            max_technical_score: 10.0,
            technical_patterns: vec![
                TechnicalPattern {
                    name: "cve".to_string(),
                    pattern: r"CVE-\d{4}-\d{4,}".to_string(),
                    weight: 3.0,
                },
                TechnicalPattern {
                    name: "service_version".to_string(),
                    pattern: r"v\d+\.\d+".to_string(),
                    weight: 1.5,
                },
            ],
        },
        tier3: Tier3Config {
            cluster_min_size: 2,
            max_cluster_size: 100,
            representative_strategy: "highest_entropy".to_string(),
            cluster_patterns: vec![NormalizationPattern {
                name: "numbers".to_string(),
                pattern: r"\b\d+\b".to_string(),
                replacement: "__NUM__".to_string(),
                priority: 1,
            }],
            preserve_metadata: vec![],
        },
    };

    Arc::new(PatternRegistry::from_configs(entities, tools, filters).unwrap())
}

#[test]
fn test_nmap_output_filtering() {
    let patterns = create_patterns();
    let pipeline = FilterPipeline::new(patterns);

    // Realistic nmap output with many repeated patterns
    let nmap_output = r#"
Starting Nmap 7.94 ( https://nmap.org ) at 2024-01-15 10:30 UTC
Nmap scan report for target1.local (192.168.1.10)
Host is up (0.0012s latency).
Not shown: 995 closed ports
PORT     STATE SERVICE VERSION
22/tcp   open  ssh     OpenSSH 8.9p1 Ubuntu 3ubuntu0.1 (Ubuntu Linux; protocol 2.0)
80/tcp   open  http    nginx 1.18.0
443/tcp  open  ssl/http nginx 1.18.0
3306/tcp open  mysql   MySQL 8.0.32
8080/tcp open  http    Apache httpd 2.4.52

Nmap scan report for target2.local (192.168.1.11)
Host is up (0.0015s latency).
Not shown: 996 closed ports
PORT     STATE SERVICE VERSION
22/tcp   open  ssh     OpenSSH 8.9p1 Ubuntu 3ubuntu0.1 (Ubuntu Linux; protocol 2.0)
80/tcp   open  http    nginx 1.18.0
443/tcp  open  ssl/http nginx 1.18.0
3306/tcp open  mysql   MySQL 8.0.32

Nmap scan report for target3.local (192.168.1.12)
Host is up (0.0010s latency).
Not shown: 997 closed ports
PORT     STATE SERVICE VERSION
22/tcp   open  ssh     OpenSSH 8.9p1 Ubuntu 3ubuntu0.1 (Ubuntu Linux; protocol 2.0)
80/tcp   open  http    nginx 1.18.0
443/tcp  open  ssl/http nginx 1.18.0

Nmap done: 3 IP addresses (3 hosts up) scanned in 15.23 seconds
"#;

    let (clusters, stats) = pipeline
        .process_capture("test-nmap-session", nmap_output)
        .unwrap();

    // Verify filtering effectiveness
    println!("Nmap filtering stats:");
    println!("  Input lines: {}", stats.input_lines);
    println!("  After Tier 1: {}", stats.tier1_output);
    println!("  After Tier 2: {}", stats.tier2_output);
    println!("  Final clusters: {}", stats.tier3_clusters);
    println!("  Processing time: {}ms", stats.processing_time_ms);

    // Should have significant reduction due to repeated patterns
    assert!(stats.input_lines > 20);
    assert!(stats.tier3_clusters < stats.input_lines);

    // Verify clusters were created
    assert!(!clusters.is_empty());

    // Processing should be fast
    assert!(stats.processing_time_ms < 100);
}

#[test]
fn test_gobuster_output_filtering() {
    let patterns = create_patterns();
    let pipeline = FilterPipeline::new(patterns);

    // Realistic gobuster output with many 404s
    let gobuster_output = r#"
===============================================================
Gobuster v3.6
by OJ Reeves (@TheColonial) & Christian Mehlmauer (@firefart)
===============================================================
[+] Url:                     http://192.168.1.10
[+] Method:                  GET
[+] Threads:                 10
[+] Wordlist:                /usr/share/wordlists/dirb/common.txt
===============================================================
Starting gobuster in directory enumeration mode
===============================================================
/admin                (Status: 200) [Size: 1234]
/login                (Status: 200) [Size: 2345]
/api                  (Status: 401) [Size: 123]
/config               (Status: 403) [Size: 456]
/test                 (Status: 404) [Size: 789]
/backup               (Status: 404) [Size: 789]
/old                  (Status: 404) [Size: 789]
/tmp                  (Status: 404) [Size: 789]
/debug                (Status: 404) [Size: 789]
Progress: 4614 / 4615 (99.98%)
===============================================================
Finished
===============================================================
"#;

    let (clusters, stats) = pipeline
        .process_capture("test-gobuster-session", gobuster_output)
        .unwrap();

    println!("Gobuster filtering stats:");
    println!("  Input lines: {}", stats.input_lines);
    println!("  After Tier 1: {}", stats.tier1_output);
    println!("  After Tier 2: {}", stats.tier2_output);
    println!("  Final clusters: {}", stats.tier3_clusters);

    // Should cluster similar 404 responses together
    assert!(stats.tier3_clusters < stats.input_lines);
    assert!(!clusters.is_empty());
}

#[test]
fn test_large_output_performance() {
    let patterns = create_patterns();
    let pipeline = FilterPipeline::new(patterns);

    // Generate 10K lines of varied output
    let mut lines = Vec::new();
    for i in 0..10000 {
        if i % 10 == 0 {
            lines.push(format!(
                "Found vulnerability CVE-2024-{} on 192.168.1.{}",
                1000 + i % 100,
                i % 255
            ));
        } else {
            lines.push(format!(
                "Scanning port {} on host 192.168.1.{}",
                i % 65535,
                i % 255
            ));
        }
    }
    let output = lines.join("\n");

    let (clusters, stats) = pipeline
        .process_capture("test-performance", &output)
        .unwrap();

    println!("Performance test (10K lines):");
    println!("  Input lines: {}", stats.input_lines);
    println!("  After Tier 1: {}", stats.tier1_output);
    println!("  After Tier 2: {}", stats.tier2_output);
    println!("  Final clusters: {}", stats.tier3_clusters);
    println!("  Processing time: {}ms", stats.processing_time_ms);
    println!(
        "  Throughput: {:.0} lines/ms",
        stats.input_lines as f32 / stats.processing_time_ms as f32
    );

    // Verify performance target
    assert_eq!(stats.input_lines, 10000);
    assert!(stats.processing_time_ms <= 600); // Should be under 600ms for 10K lines (allows variance)

    // Verify significant reduction
    let reduction = 1.0 - (stats.tier3_clusters as f32 / stats.input_lines as f32);
    println!("  Overall reduction: {:.1}%", reduction * 100.0);
    assert!(reduction > 0.5); // At least 50% reduction

    assert!(!clusters.is_empty());
}

#[test]
fn test_session_isolation() {
    let patterns = create_patterns();
    let pipeline = FilterPipeline::new(patterns);

    let repeated_line = "Test line that will be repeated\n".repeat(5);

    // Session 1: All lines should pass (first occurrence)
    let (_, stats1) = pipeline
        .process_capture("session-1", &repeated_line)
        .unwrap();
    assert!(stats1.tier1_output >= 3); // max_occurrences = 3

    // Session 2: Should have independent state
    let (_, stats2) = pipeline
        .process_capture("session-2", &repeated_line)
        .unwrap();
    assert!(stats2.tier1_output >= 3); // Independent deduplication

    // Session 1 again: Should remember previous state
    let (_, stats3) = pipeline
        .process_capture("session-1", &repeated_line)
        .unwrap();
    assert!(stats3.tier1_output < stats1.tier1_output); // Already seen some lines

    // Cleanup
    pipeline.clear_session("session-1");
    pipeline.clear_session("session-2");
    assert_eq!(pipeline.active_sessions(), 0);
}
