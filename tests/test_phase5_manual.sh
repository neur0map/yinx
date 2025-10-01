#!/bin/bash
# Manual testing script for Phase 5: Entity Extraction & Metadata

echo "═══════════════════════════════════════════════════════════════"
echo "Phase 5 Manual Testing Script"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Build the project
echo -e "${BLUE}Step 1: Building project...${NC}"
cargo build --release
if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi
echo -e "${GREEN}✓ Build successful${NC}\n"

# Create test directory
TEST_DIR="/tmp/yinx_phase5_test"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

echo -e "${BLUE}Step 2: Creating test tool outputs...${NC}"

# Create nmap output
cat > nmap_scan.txt <<'EOF'
Starting Nmap 7.91 ( https://nmap.org ) at 2025-01-01 10:00 UTC
Nmap scan report for target.example.com (192.168.1.100)
Host is up (0.0010s latency).
Not shown: 995 closed ports
PORT     STATE SERVICE    VERSION
22/tcp   open  ssh        OpenSSH/8.2p1 Ubuntu 4ubuntu0.3
80/tcp   open  http       Apache/2.4.41 (Ubuntu)
443/tcp  open  ssl/https  nginx/1.18.0
3306/tcp open  mysql      MySQL/5.7.35
8080/tcp open  http-proxy

Nmap scan report for 192.168.1.101
Host is up (0.0012s latency).
PORT   STATE SERVICE VERSION
22/tcp open  ssh     OpenSSH/7.9p1

Service detection performed.
Nmap done: 2 IP addresses (2 hosts up) scanned in 12.34 seconds
EOF

# Create vulnerability scan output
cat > vuln_scan.txt <<'EOF'
Vulnerability Assessment Report
Target: 192.168.1.100

[CRITICAL] CVE-2021-44228 - Apache Log4j Remote Code Execution
Affected Service: Apache/2.4.41 on port 80/tcp
Description: Remote code execution vulnerability in Log4j

[HIGH] CVE-2021-3156 - Sudo Buffer Overflow
Affected Service: sudo on OpenSSH/8.2p1

[MEDIUM] CVE-2020-11984 - Apache HTTP Server mod_proxy_uwsgi
Affected Service: Apache/2.4.41
EOF

# Create credential discovery output
cat > creds_found.txt <<'EOF'
Hydra v9.1 starting at 2025-01-01 11:00:00
[DATA] attacking ssh://192.168.1.100:22/
[22][ssh] host: 192.168.1.100   login: admin   password: Welcome123!
[22][ssh] host: 192.168.1.100   login: backup  password: Backup2024

Configuration file found at /etc/mysql/my.cnf:
  user=root
  password=MyS3cr3tP@ss

AWS credentials discovered:
  AKIAIOSFODNN7EXAMPLE
EOF

echo -e "${GREEN}✓ Created test files:${NC}"
echo "  - nmap_scan.txt (nmap output with IPs, ports, services)"
echo "  - vuln_scan.txt (vulnerability scan with CVEs)"
echo "  - creds_found.txt (credential discovery with sensitive data)"
echo ""

# Show what entities should be extracted
echo -e "${BLUE}Step 3: Expected entity extraction results:${NC}"
echo ""
echo -e "${YELLOW}From nmap_scan.txt:${NC}"
echo "  • IP Addresses: 192.168.1.100, 192.168.1.101"
echo "  • Hostnames: target.example.com"
echo "  • Ports: 22/tcp, 80/tcp, 443/tcp, 3306/tcp, 8080/tcp"
echo "  • Service Versions: OpenSSH/8.2p1, Apache/2.4.41, nginx/1.18.0, MySQL/5.7.35"
echo ""
echo -e "${YELLOW}From vuln_scan.txt:${NC}"
echo "  • CVEs: CVE-2021-44228, CVE-2021-3156, CVE-2020-11984"
echo "  • IP Address: 192.168.1.100"
echo "  • Service Versions: Apache/2.4.41, OpenSSH/8.2p1"
echo ""
echo -e "${YELLOW}From creds_found.txt:${NC}"
echo "  • IP Address: 192.168.1.100"
echo "  • Credentials (REDACTED): password=Welcome123!, password=Backup2024, password=MyS3cr3tP@ss"
echo "  • AWS Access Key: AKIAIOSFODNN7EXAMPLE"
echo "  • File Paths: /etc/mysql/my.cnf"
echo ""

# Interactive testing
echo -e "${BLUE}Step 4: Run unit tests to see entity extraction in action${NC}"
echo "Run this command to see detailed test output:"
echo ""
echo -e "${GREEN}  cargo test --test test_entity_integration -- --nocapture${NC}"
echo ""

# Show where test files are
echo -e "${BLUE}Step 5: Test files location${NC}"
echo "Test files created in: $TEST_DIR"
echo ""
echo "You can manually inspect entity extraction by:"
echo "  1. Looking at the test outputs above"
echo "  2. Running the cargo test command"
echo "  3. Checking src/entities/ module implementation"
echo ""

# Database inspection
echo -e "${BLUE}Step 6: Inspecting database storage (after daemon runs)${NC}"
echo "When the daemon is running, entities are stored in:"
echo "  ~/.yinx/store/db.sqlite"
echo ""
echo "You can query with sqlite3:"
echo -e "${GREEN}  sqlite3 ~/.yinx/store/db.sqlite 'SELECT * FROM entities LIMIT 10;'${NC}"
echo ""

# Show correlation graph
echo -e "${BLUE}Step 7: Correlation Graph Features${NC}"
echo "The correlation graph tracks:"
echo "  • Host → Ports → Services relationships"
echo "  • Vulnerability → Affected hosts mapping"
echo "  • Credentials → Host associations"
echo "  • First/last seen timestamps"
echo ""

echo -e "${GREEN}════════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}Manual testing setup complete!${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Next steps:"
echo "  1. Run: cargo test --test test_entity_integration -- --nocapture"
echo "  2. Inspect test files in: $TEST_DIR"
echo "  3. Review src/entities/ implementation"
echo "  4. Check database schema: src/storage/database.rs:232-244"
echo ""
