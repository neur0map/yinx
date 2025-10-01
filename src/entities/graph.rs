//! Host/service correlation graph for tracking relationships
//!
//! Builds a knowledge graph of hosts, services, ports, and vulnerabilities

use crate::entities::Entity;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Information about a discovered host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    /// Host identifier (IP or hostname)
    pub identifier: String,
    /// Open ports discovered
    pub ports: HashSet<u16>,
    /// Services running on the host
    pub services: HashMap<u16, String>,
    /// Vulnerabilities found
    pub vulnerabilities: HashSet<String>,
    /// Credentials discovered
    pub credentials: Vec<String>,
    /// File paths found
    pub paths: HashSet<String>,
    /// First seen timestamp
    pub first_seen: i64,
    /// Last seen timestamp
    pub last_seen: i64,
}

impl HostInfo {
    /// Create new host info
    pub fn new(identifier: String, timestamp: i64) -> Self {
        Self {
            identifier,
            ports: HashSet::new(),
            services: HashMap::new(),
            vulnerabilities: HashSet::new(),
            credentials: Vec::new(),
            paths: HashSet::new(),
            first_seen: timestamp,
            last_seen: timestamp,
        }
    }

    /// Update last seen timestamp
    pub fn update_timestamp(&mut self, timestamp: i64) {
        if timestamp > self.last_seen {
            self.last_seen = timestamp;
        }
    }

    /// Add a port
    pub fn add_port(&mut self, port: u16) {
        self.ports.insert(port);
    }

    /// Add a service
    pub fn add_service(&mut self, port: u16, service: String) {
        self.ports.insert(port);
        self.services.insert(port, service);
    }

    /// Add a vulnerability
    pub fn add_vulnerability(&mut self, vuln: String) {
        self.vulnerabilities.insert(vuln);
    }

    /// Add a credential
    pub fn add_credential(&mut self, cred: String) {
        self.credentials.push(cred);
    }

    /// Add a file path
    pub fn add_path(&mut self, path: String) {
        self.paths.insert(path);
    }
}

/// Service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Service name
    pub name: String,
    /// Hosts running this service
    pub hosts: HashSet<String>,
    /// Versions seen
    pub versions: HashSet<String>,
    /// Vulnerabilities associated with this service
    pub vulnerabilities: HashSet<String>,
}

impl ServiceInfo {
    /// Create new service info
    pub fn new(name: String) -> Self {
        Self {
            name,
            hosts: HashSet::new(),
            versions: HashSet::new(),
            vulnerabilities: HashSet::new(),
        }
    }

    /// Add a host running this service
    pub fn add_host(&mut self, host: String) {
        self.hosts.insert(host);
    }

    /// Add a version
    pub fn add_version(&mut self, version: String) {
        self.versions.insert(version);
    }

    /// Add a vulnerability
    pub fn add_vulnerability(&mut self, vuln: String) {
        self.vulnerabilities.insert(vuln);
    }
}

/// Correlation graph for tracking relationships between entities
pub struct CorrelationGraph {
    /// Host information indexed by identifier
    hosts: HashMap<String, HostInfo>,
    /// Service information indexed by name
    services: HashMap<String, ServiceInfo>,
    /// Vulnerability to hosts mapping
    vulnerabilities: HashMap<String, HashSet<String>>,
}

impl CorrelationGraph {
    /// Create new correlation graph
    pub fn new() -> Self {
        Self {
            hosts: HashMap::new(),
            services: HashMap::new(),
            vulnerabilities: HashMap::new(),
        }
    }

    /// Process entities and update graph
    ///
    /// Correlates entities extracted from the same context
    pub fn process_entities(&mut self, entities: &[Entity], timestamp: i64) {
        // Extract hosts (IPs and hostnames)
        let hosts: Vec<&Entity> = entities
            .iter()
            .filter(|e| e.entity_type == "ip_address" || e.entity_type == "hostname")
            .collect();

        // Extract ports
        let ports: Vec<&Entity> = entities
            .iter()
            .filter(|e| e.entity_type == "port")
            .collect();

        // Extract services
        let services: Vec<&Entity> = entities
            .iter()
            .filter(|e| e.entity_type == "service_version")
            .collect();

        // Extract vulnerabilities
        let vulnerabilities: Vec<&Entity> =
            entities.iter().filter(|e| e.entity_type == "cve").collect();

        // Extract credentials
        let credentials: Vec<&Entity> = entities
            .iter()
            .filter(|e| e.entity_type.starts_with("credential_"))
            .collect();

        // Extract file paths
        let paths: Vec<&Entity> = entities
            .iter()
            .filter(|e| e.entity_type == "file_path_unix" || e.entity_type == "file_path_windows")
            .collect();

        // Process each host
        for host_entity in &hosts {
            let host_id = &host_entity.value;
            let host_info = self
                .hosts
                .entry(host_id.clone())
                .or_insert_with(|| HostInfo::new(host_id.clone(), timestamp));

            host_info.update_timestamp(timestamp);

            // Add ports
            for port_entity in &ports {
                if let Some(port) = Self::parse_port(&port_entity.value) {
                    host_info.add_port(port);
                }
            }

            // Add services
            for service_entity in &services {
                if let Some((service_name, version)) = Self::parse_service(&service_entity.value) {
                    // Add to host
                    if let Some(first_port) = host_info.ports.iter().next().copied() {
                        host_info.add_service(first_port, service_name.clone());
                    }

                    // Add to services graph
                    let service_info = self
                        .services
                        .entry(service_name.clone())
                        .or_insert_with(|| ServiceInfo::new(service_name));
                    service_info.add_host(host_id.clone());
                    service_info.add_version(version);
                }
            }

            // Add vulnerabilities
            for vuln_entity in &vulnerabilities {
                let vuln_id = &vuln_entity.value;
                host_info.add_vulnerability(vuln_id.clone());

                // Update vulnerability index
                self.vulnerabilities
                    .entry(vuln_id.clone())
                    .or_default()
                    .insert(host_id.clone());

                // Add to services if applicable
                for service_info in self.services.values_mut() {
                    if service_info.hosts.contains(host_id) {
                        service_info.add_vulnerability(vuln_id.clone());
                    }
                }
            }

            // Add credentials
            for cred_entity in &credentials {
                host_info.add_credential(cred_entity.value.clone());
            }

            // Add paths
            for path_entity in &paths {
                host_info.add_path(path_entity.value.clone());
            }
        }
    }

    /// Get host information
    pub fn get_host(&self, identifier: &str) -> Option<&HostInfo> {
        self.hosts.get(identifier)
    }

    /// Get all hosts
    pub fn get_all_hosts(&self) -> Vec<&HostInfo> {
        self.hosts.values().collect()
    }

    /// Get service information
    pub fn get_service(&self, name: &str) -> Option<&ServiceInfo> {
        self.services.get(name)
    }

    /// Get all services
    pub fn get_all_services(&self) -> Vec<&ServiceInfo> {
        self.services.values().collect()
    }

    /// Get hosts affected by a vulnerability
    pub fn get_vulnerable_hosts(&self, cve: &str) -> Vec<&HostInfo> {
        self.vulnerabilities
            .get(cve)
            .map(|host_ids| {
                host_ids
                    .iter()
                    .filter_map(|id| self.hosts.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all unique vulnerabilities
    pub fn get_all_vulnerabilities(&self) -> Vec<String> {
        let mut vulns: Vec<String> = self.vulnerabilities.keys().cloned().collect();
        vulns.sort();
        vulns
    }

    /// Get statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            host_count: self.hosts.len(),
            service_count: self.services.len(),
            vulnerability_count: self.vulnerabilities.len(),
            total_ports: self.hosts.values().map(|h| h.ports.len()).sum(),
            total_credentials: self.hosts.values().map(|h| h.credentials.len()).sum(),
        }
    }

    /// Parse port from entity value (e.g., "22/tcp" -> Some(22))
    fn parse_port(value: &str) -> Option<u16> {
        value.split('/').next()?.parse().ok()
    }

    /// Parse service from entity value (e.g., "Apache/2.4.41" -> Some(("Apache", "2.4.41")))
    fn parse_service(value: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = value.split('/').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }
}

impl Default for CorrelationGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Correlation graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub host_count: usize,
    pub service_count: usize,
    pub vulnerability_count: usize,
    pub total_ports: usize,
    pub total_credentials: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entity(entity_type: &str, value: &str) -> Entity {
        Entity {
            entity_type: entity_type.to_string(),
            value: value.to_string(),
            context: format!("Context for {}", value),
            confidence: 0.9,
            should_redact: false,
        }
    }

    #[test]
    fn test_host_creation() {
        let host = HostInfo::new("192.168.1.1".to_string(), 1000);
        assert_eq!(host.identifier, "192.168.1.1");
        assert_eq!(host.first_seen, 1000);
        assert_eq!(host.last_seen, 1000);
    }

    #[test]
    fn test_host_update() {
        let mut host = HostInfo::new("192.168.1.1".to_string(), 1000);
        host.add_port(22);
        host.add_port(80);
        host.add_service(22, "ssh".to_string());
        host.add_vulnerability("CVE-2021-44228".to_string());

        assert_eq!(host.ports.len(), 2);
        assert!(host.ports.contains(&22));
        assert!(host.services.contains_key(&22));
        assert_eq!(host.vulnerabilities.len(), 1);
    }

    #[test]
    fn test_graph_basic() {
        let mut graph = CorrelationGraph::new();
        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1"),
            create_test_entity("port", "22/tcp"),
            create_test_entity("cve", "CVE-2021-44228"),
        ];

        graph.process_entities(&entities, 1000);

        let host = graph.get_host("192.168.1.1").unwrap();
        assert_eq!(host.ports.len(), 1);
        assert!(host.ports.contains(&22));
        assert_eq!(host.vulnerabilities.len(), 1);
    }

    #[test]
    fn test_service_correlation() {
        let mut graph = CorrelationGraph::new();
        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1"),
            create_test_entity("port", "80/tcp"),
            create_test_entity("service_version", "Apache/2.4.41"),
        ];

        graph.process_entities(&entities, 1000);

        let service = graph.get_service("Apache").unwrap();
        assert!(service.hosts.contains("192.168.1.1"));
        assert!(service.versions.contains("2.4.41"));
    }

    #[test]
    fn test_vulnerability_mapping() {
        let mut graph = CorrelationGraph::new();
        let entities1 = vec![
            create_test_entity("ip_address", "192.168.1.1"),
            create_test_entity("cve", "CVE-2021-44228"),
        ];
        let entities2 = vec![
            create_test_entity("ip_address", "192.168.1.2"),
            create_test_entity("cve", "CVE-2021-44228"),
        ];

        graph.process_entities(&entities1, 1000);
        graph.process_entities(&entities2, 2000);

        let affected = graph.get_vulnerable_hosts("CVE-2021-44228");
        assert_eq!(affected.len(), 2);
    }

    #[test]
    fn test_credential_tracking() {
        let mut graph = CorrelationGraph::new();
        let mut cred_entity = create_test_entity("credential_password", "admin:password123");
        cred_entity.should_redact = true;

        let entities = vec![create_test_entity("ip_address", "192.168.1.1"), cred_entity];

        graph.process_entities(&entities, 1000);

        let host = graph.get_host("192.168.1.1").unwrap();
        assert_eq!(host.credentials.len(), 1);
    }

    #[test]
    fn test_graph_stats() {
        let mut graph = CorrelationGraph::new();
        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1"),
            create_test_entity("port", "22/tcp"),
            create_test_entity("port", "80/tcp"),
            create_test_entity("service_version", "Apache/2.4.41"),
            create_test_entity("cve", "CVE-2021-44228"),
        ];

        graph.process_entities(&entities, 1000);

        let stats = graph.stats();
        assert_eq!(stats.host_count, 1);
        assert_eq!(stats.vulnerability_count, 1);
        assert_eq!(stats.total_ports, 2);
    }

    #[test]
    fn test_multiple_hosts() {
        let mut graph = CorrelationGraph::new();

        for i in 1..=5 {
            let entities = vec![
                create_test_entity("ip_address", &format!("192.168.1.{}", i)),
                create_test_entity("port", "22/tcp"),
            ];
            graph.process_entities(&entities, 1000 + i as i64);
        }

        let stats = graph.stats();
        assert_eq!(stats.host_count, 5);
        assert_eq!(stats.total_ports, 5);
    }

    #[test]
    fn test_timestamp_updates() {
        let mut graph = CorrelationGraph::new();
        let entities = vec![create_test_entity("ip_address", "192.168.1.1")];

        graph.process_entities(&entities, 1000);
        graph.process_entities(&entities, 2000);

        let host = graph.get_host("192.168.1.1").unwrap();
        assert_eq!(host.first_seen, 1000);
        assert_eq!(host.last_seen, 2000);
    }
}
