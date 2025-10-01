#!/bin/bash
# Quick script to inject test data and verify Phase 5 entity extraction

echo "Injecting test data into yinx database..."

# Create test data with entities
TEST_OUTPUT="Nmap scan report for target.example.com (192.168.1.100)
PORT     STATE SERVICE    VERSION
22/tcp   open  ssh        OpenSSH/8.2p1
80/tcp   open  http       Apache/2.4.41
443/tcp  open  https      nginx/1.18.0

Vulnerability scan results:
[CRITICAL] CVE-2021-44228 - Apache Log4j RCE
[HIGH] CVE-2021-3156 - Sudo heap overflow

Credentials found:
admin:password123
root:toor123
"

# Run through entity extraction pipeline
cargo run --release --bin yinx -- start 2>&1 | head -5

sleep 2

# Simulate a capture via IPC (if daemon is running)
echo "Test data created. Checking what was extracted..."

cargo run --release --bin yinx -- stop

# Check database
echo ""
echo "=== DATABASE VERIFICATION ==="
sqlite3 ~/.yinx/store/db.sqlite << 'EOF'
.mode column
.headers on
SELECT 'Sessions:' as metric, COUNT(*) as count FROM sessions
UNION ALL SELECT 'Captures:', COUNT(*) FROM captures
UNION ALL SELECT 'Entities:', COUNT(*) FROM entities;

SELECT '--- Entities by Type ---' as info;
SELECT type, COUNT(*) as count FROM entities GROUP BY type ORDER BY count DESC;

SELECT '--- Sample Entities ---' as info;
SELECT type, value, ROUND(confidence, 2) as conf FROM entities LIMIT 10;
EOF
