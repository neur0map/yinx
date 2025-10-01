//! Integration tests for entity extraction with realistic tool outputs

use yinx::entities::{CorrelationGraph, EntityExtractor, MetadataEnricher};
use yinx::patterns::{
    EntitiesConfig, EntityConfig, FiltersConfig, PatternRegistry, Tier1Config, Tier2Config,
    Tier3Config, ToolsConfig,
};

/// Create test pattern registry with full entity patterns
fn create_full_registry() -> PatternRegistry {
    let entities_config = EntitiesConfig {
        entity: vec![
            EntityConfig {
                type_name: "ip_address".to_string(),
                pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                confidence: 0.95,
                context_window: 50,
                redact: false,
                description: "IPv4 address".to_string(),
            },
            EntityConfig {
                type_name: "port".to_string(),
                pattern: r"\b(\d{1,5})/(tcp|udp)\b".to_string(),
                confidence: 0.9,
                context_window: 30,
                redact: false,
                description: "Network port".to_string(),
            },
            EntityConfig {
                type_name: "hostname".to_string(),
                pattern: r"\b([a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}\b"
                    .to_string(),
                confidence: 0.75,
                context_window: 40,
                redact: false,
                description: "DNS hostname".to_string(),
            },
            EntityConfig {
                type_name: "cve".to_string(),
                pattern: r"CVE-\d{4}-\d{4,}".to_string(),
                confidence: 1.0,
                context_window: 100,
                redact: false,
                description: "CVE vulnerability".to_string(),
            },
            EntityConfig {
                type_name: "service_version".to_string(),
                pattern: r"(?i)(apache|nginx|openssh|mysql|postgresql)/[\d\.]+".to_string(),
                confidence: 0.8,
                context_window: 60,
                redact: false,
                description: "Service version".to_string(),
            },
            EntityConfig {
                type_name: "credential_password".to_string(),
                pattern: r"(?i)(password|passwd|pwd)\s*[:=]\s*\S+".to_string(),
                confidence: 0.7,
                context_window: 80,
                redact: true,
                description: "Password credential".to_string(),
            },
        ],
    };

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

    PatternRegistry::from_configs(entities_config, tools_config, filters_config).unwrap()
}

#[test]
fn test_nmap_output_entity_extraction() {
    let nmap_output = r#"
Starting Nmap 7.91 ( https://nmap.org ) at 2025-01-01 10:00 UTC
Nmap scan report for 192.168.1.1
Host is up (0.0010s latency).
Not shown: 995 closed ports
PORT     STATE SERVICE    VERSION
22/tcp   open  ssh        OpenSSH/8.2p1 Ubuntu 4ubuntu0.3 (Ubuntu Linux; protocol 2.0)
80/tcp   open  http       Apache/2.4.41 (Ubuntu)
443/tcp  open  ssl/http   nginx/1.18.0 (Ubuntu)
3306/tcp open  mysql      MySQL/5.7.35
8080/tcp open  http-proxy

Nmap scan report for 192.168.1.2
Host is up (0.0012s latency).
PORT    STATE SERVICE VERSION
22/tcp  open  ssh     OpenSSH/7.9p1 Debian 10+deb10u2 (protocol 2.0)
80/tcp  open  http    nginx/1.14.2
"#;

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let entities = extractor.extract(nmap_output);

    // Verify IP addresses extracted
    let ips: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "ip_address")
        .map(|e| e.value.as_str())
        .collect();
    assert!(ips.contains(&"192.168.1.1"));
    assert!(ips.contains(&"192.168.1.2"));

    // Verify ports extracted
    let ports: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "port")
        .map(|e| e.value.as_str())
        .collect();
    assert!(ports.contains(&"22/tcp"));
    assert!(ports.contains(&"80/tcp"));
    assert!(ports.contains(&"443/tcp"));
    assert!(ports.contains(&"3306/tcp"));

    // Verify service versions
    let services: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "service_version")
        .collect();
    assert!(!services.is_empty());

    println!("Extracted {} entities from nmap output", entities.len());
    println!("  - IPs: {}", ips.len());
    println!("  - Ports: {}", ports.len());
    println!("  - Services: {}", services.len());
}

#[test]
fn test_gobuster_output_entity_extraction() {
    let gobuster_output = r#"
===============================================================
Gobuster v3.1.0
by OJ Reeves (@TheColonial) & Christian Mehlmauer (@firefart)
===============================================================
[+] Url:                     https://example.com
[+] Method:                  GET
[+] Threads:                 10
[+] Wordlist:                /usr/share/wordlists/dirb/common.txt
===============================================================
2025/01/01 10:00:00 Starting gobuster in directory enumeration mode
===============================================================
/.git                 (Status: 301) [Size: 310] [--> https://example.com/.git/]
/admin                (Status: 200) [Size: 4523]
/api                  (Status: 301) [Size: 309] [--> https://example.com/api/]
/backup               (Status: 403) [Size: 280]
/config               (Status: 200) [Size: 1523]
/dashboard            (Status: 302) [Size: 0] [--> https://example.com/login]
/index.php            (Status: 200) [Size: 10234]
"#;

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let entities = extractor.extract(gobuster_output);

    // Verify hostname extracted
    let hostnames: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "hostname")
        .map(|e| e.value.as_str())
        .collect();
    assert!(hostnames.contains(&"example.com"));

    println!("Extracted {} entities from gobuster output", entities.len());
    println!("  - Hostnames: {}", hostnames.len());
}

#[test]
fn test_hydra_output_with_credentials() {
    let hydra_output = r#"
Hydra v9.1 (c) 2020 by van Hauser/THC & David Maciejak - Please do not use in military or secret service organizations, or for illegal purposes.

Hydra (https://github.com/vanhauser-thc/thc-hydra) starting at 2025-01-01 10:00:00
[DATA] max 16 tasks per 1 server, overall 16 tasks, 14344399 login tries (l:1/p:14344399), ~896525 tries per task
[DATA] attacking ssh://192.168.1.10:22/
[22][ssh] host: 192.168.1.10   login: admin   password: admin123
[22][ssh] host: 192.168.1.10   login: root    password: toor
1 of 1 target successfully completed, 2 valid passwords found
"#;

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let entities = extractor.extract(hydra_output);

    // Verify IP extracted
    let ips: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "ip_address")
        .collect();
    assert!(!ips.is_empty());

    // Verify credentials extracted (should be marked for redaction)
    let creds: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "credential_password")
        .collect();
    assert!(!creds.is_empty());
    assert!(creds.iter().all(|c| c.should_redact));

    println!("Extracted {} entities from hydra output", entities.len());
    println!("  - IPs: {}", ips.len());
    println!("  - Credentials (redacted): {}", creds.len());
}

#[test]
fn test_correlation_graph_integration() {
    let nmap_output = r#"
Nmap scan report for 192.168.1.100
PORT    STATE SERVICE VERSION
22/tcp  open  ssh     OpenSSH/8.2p1
80/tcp  open  http    Apache/2.4.41
CVE-2021-44228 detected in Apache Log4j
"#;

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let entities = extractor.extract(nmap_output);

    // Build correlation graph
    let mut graph = CorrelationGraph::new();
    graph.process_entities(&entities, 1000);

    // Verify host was added
    let host = graph.get_host("192.168.1.100").unwrap();
    assert_eq!(host.identifier, "192.168.1.100");
    assert!(host.ports.contains(&22));
    assert!(host.ports.contains(&80));

    // Verify CVE was correlated
    let vulns: Vec<_> = host.vulnerabilities.iter().collect();
    assert!(!vulns.is_empty());

    println!("Correlation graph stats: {:?}", graph.stats());
}

#[test]
fn test_metadata_enrichment_workflow() {
    let scan_output = r#"
Nmap scan report for target.example.com (10.0.0.50)
PORT     STATE SERVICE    VERSION
22/tcp   open  ssh        OpenSSH/8.2p1
3306/tcp open  mysql      MySQL/5.7.35
Found vulnerability: CVE-2021-3156 in sudo
"#;

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let entities = extractor.extract(scan_output);

    // Create metadata enricher
    let mut enricher = MetadataEnricher::new();

    // Enrich capture
    let metadata = enricher.enrich_capture(entities, Some("nmap".to_string()), 1000);

    // Verify metadata
    assert_eq!(metadata.tool, Some("nmap".to_string()));
    assert!(!metadata.hosts.is_empty());
    assert!(!metadata.vulnerabilities.is_empty());

    // Verify correlation graph was updated
    let stats = enricher.graph().stats();
    assert!(stats.host_count > 0);
    assert!(stats.vulnerability_count > 0);

    println!("Capture metadata:");
    println!("  - Tool: {:?}", metadata.tool);
    println!("  - Hosts: {:?}", metadata.hosts);
    println!("  - Vulnerabilities: {:?}", metadata.vulnerabilities);
    println!("  - Entity types: {:?}", metadata.entity_types);
}

#[test]
fn test_multiple_hosts_correlation() {
    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let mut graph = CorrelationGraph::new();

    // Scan 1: Host 192.168.1.100
    let scan1 = r#"
Nmap scan report for 192.168.1.100
PORT   STATE SERVICE VERSION
22/tcp open  ssh     OpenSSH/8.2p1
CVE-2021-44228 detected
"#;
    let entities1 = extractor.extract(scan1);
    graph.process_entities(&entities1, 1000);

    // Scan 2: Host 192.168.1.101
    let scan2 = r#"
Nmap scan report for 192.168.1.101
PORT   STATE SERVICE VERSION
80/tcp open  http    Apache/2.4.41
CVE-2021-44228 detected
"#;
    let entities2 = extractor.extract(scan2);
    graph.process_entities(&entities2, 2000);

    // Verify both hosts affected by same CVE
    let affected = graph.get_vulnerable_hosts("CVE-2021-44228");
    assert_eq!(affected.len(), 2);

    let stats = graph.stats();
    assert_eq!(stats.host_count, 2);
    assert_eq!(stats.vulnerability_count, 1);

    println!("Multi-host correlation:");
    println!("  - Total hosts: {}", stats.host_count);
    println!("  - Shared vulnerabilities: {}", stats.vulnerability_count);
}

#[test]
fn test_performance_large_output() {
    use std::time::Instant;

    // Generate large realistic output (1000 lines)
    let mut large_output = String::new();
    for i in 1..=250 {
        large_output.push_str(&format!("Nmap scan report for 192.168.1.{}\n", i % 255 + 1));
        large_output.push_str("PORT   STATE SERVICE\n");
        large_output.push_str("22/tcp open  ssh\n");
        large_output.push_str("80/tcp open  http\n");
    }

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);

    let start = Instant::now();
    let entities = extractor.extract(&large_output);
    let duration = start.elapsed();

    println!("Performance test (1000 lines):");
    println!("  - Extracted {} entities", entities.len());
    println!("  - Time: {:?}", duration);
    println!(
        "  - Rate: {:.0} entities/sec",
        entities.len() as f64 / duration.as_secs_f64()
    );

    // Should complete in reasonable time (< 100ms for 1000 lines)
    assert!(
        duration.as_millis() < 100,
        "Entity extraction too slow: {:?}",
        duration
    );
}

#[test]
fn test_entity_context_extraction() {
    let output = r#"
Critical vulnerability CVE-2021-44228 found in Apache Log4j on host 192.168.1.50
Severity: CRITICAL (CVSS: 10.0)
"#;

    let registry = create_full_registry();
    let extractor = EntityExtractor::new(registry);
    let entities = extractor.extract(output);

    // Find CVE entity and verify context
    let cve = entities
        .iter()
        .find(|e| e.entity_type == "cve")
        .expect("CVE should be extracted");

    assert!(cve.context.contains("CVE-2021-44228"));
    assert!(
        cve.context.len() > "CVE-2021-44228".len(),
        "Context should include surrounding text"
    );

    println!("Entity with context:");
    println!("  - Type: {}", cve.entity_type);
    println!("  - Value: {}", cve.value);
    println!("  - Context: {}", cve.context);
}
