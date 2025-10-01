# Yinx

**Author:** neur0map

## What It Does

Yinx captures everything you do in the terminal during penetration tests and lets you search through it instantly.

**Core features:**
- **Automatic capture** - Runs in the background, captures all commands and outputs
- **Intelligent filtering** - Reduces gigabytes of noise to just the important findings
- **Instant search** - Find IPs, credentials, vulnerabilities, tool outputs in milliseconds
- **AI assistance** - Ask questions about your pentest: "What services are running on 192.168.1.1?"
- **Report generation** - Export findings organized by host with evidence
- **Offline-first** - Works without internet, optional LLM integration when online

**Usage:**
```bash
# Start capturing
yinx start --session "client-pentest"

# Work normally - yinx captures everything
nmap -sV 192.168.1.0/24
gobuster dir -u http://target.com
hydra -l admin -P passwords.txt ssh://target.com

# Search your findings
yinx query "credentials"
yinx query "CVE-" --tool sqlmap

# Ask questions (with AI)
yinx ask "What ports are open on 192.168.1.5?"
yinx ask "Show me all password hashes found"

# Generate report
yinx report --output client-report.md

# Stop capturing
yinx stop
```

**Status:** In development (30% complete - Phase 3/10 finished)