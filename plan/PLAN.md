# Yinx - Implementation Plan

**Author:** neur0map
**Project:** Intelligent Penetration Testing Companion
**Language:** Rust
**License:** Apache 2.0

---

## Executive Summary

Yinx is a background CLI daemon that captures all terminal activity during penetration testing sessions, intelligently filters noise from tool outputs, semantically indexes findings, and provides instant retrieval without manual note-taking. The tool works fully offline with optional cloud LLM integration for strategic assistance.
It offers the option to get a template report of your findings and it's exam version is OSCP ready.

**Core Value Proposition:**
- **Zero-friction capture**: Silent background operation, no workflow changes
- **Intelligent filtering**: 100K lines → ~100 embeddings in <500ms
- **Instant retrieval**: Semantic + keyword search without calling LLMs
- **Provenance**: Every finding traced back to original command/output
- **Exam-safe**: Works completely offline with local models

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     User Terminal (bash/zsh)                │
│  $ nmap -sV target.com                                      │
│  $ gobuster dir -u http://target.com -w wordlist.txt        │
└────────────────────┬────────────────────────────────────────┘
                     │ Shell hook (PROMPT_COMMAND)
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                  Yinx Daemon (background)                   │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  IPC Server  │→ │ Async Queue  │→ │  Processor   │      │
│  │ (Unix Socket)│  │              │  │   Pipeline   │      │
│  └──────────────┘  └──────────────┘  └──────┬───────┘      │
│                                              │              │
│  ┌───────────────────────────────────────────▼─────────┐   │
│  │          Three-Tier Filtering Pipeline             │   │
│  │  ┌──────────────────────────────────────────────┐  │   │
│  │  │ Tier 1: Anomaly Detection (Hash-based)      │  │   │
│  │  │  - 100K → 10K (90% reduction)                │  │   │
│  │  └──────────────┬───────────────────────────────┘  │   │
│  │  ┌──────────────▼───────────────────────────────┐  │   │
│  │  │ Tier 2: Statistical Scoring                  │  │   │
│  │  │  - 10K → 2K (80% reduction)                  │  │   │
│  │  └──────────────┬───────────────────────────────┘  │   │
│  │  ┌──────────────▼───────────────────────────────┐  │   │
│  │  │ Tier 3: Semantic Clustering                  │  │   │
│  │  │  - 2K → 100 representatives (95% reduction)  │  │   │
│  │  └──────────────────────────────────────────────┘  │   │
│  └────────────────────────────────────────────────────┘   │
│                          │                                 │
│  ┌───────────────────────▼────────────────────────────┐   │
│  │        Entity Extraction & Metadata                │   │
│  │  - IPs, ports, URLs, CVEs, credentials, paths      │   │
│  │  - Tool detection, host/service correlation        │   │
│  └───────────────────────┬────────────────────────────┘   │
│                          ▼                                 │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              Storage Layer                          │  │
│  │  ┌────────────────┐  ┌────────────────┐             │  │
│  │  │ Blob Storage   │  │  SQLite DB     │             │  │
│  │  │ (BLAKE3 hash)  │  │  - Sessions    │             │  │
│  │  │ Content-addr.  │  │  - Metadata    │             │  │
│  │  │ Deduplicated   │  │  - Entities    │             │  │
│  │  └────────────────┘  └────────────────┘             │  │
│  │  ┌────────────────┐  ┌────────────────┐             │  │
│  │  │ Vector Index   │  │ Keyword Index  │             │  │
│  │  │ (Qdrant/HNSW)  │  │   (tantivy)    │             │  │
│  │  └────────────────┘  └────────────────┘             │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         ▼                ▼                ▼
    ┌─────────┐    ┌──────────┐    ┌──────────┐
    │ query   │    │   ask    │    │  report  │
    │ (local) │    │ (opt LLM)│    │(markdown)│
    └─────────┘    └──────────┘    └──────────┘
```

---

## ⚠️ CRITICAL: Configuration-Driven Design

**See ARCHITECTURE.md for detailed design principles.**

Throughout implementation, **avoid hardcoding:**
- ❌ Magic numbers (weights, thresholds, sizes)
- ❌ Inline regex patterns
- ❌ Tool detection if/else chains
- ❌ API endpoints and model names
- ❌ Scoring formulas and normalization rules

**Instead:**
- ✅ Load all tunable values from `config.toml`
- ✅ Compile patterns once from config into registry
- ✅ Use plugin architecture for extensibility
- ✅ Support runtime config reloading
- ✅ Provide config validation and migrations

---

## Phase 1: Foundation & CLI Structure

### Goals
Establish project structure, CLI interface, and basic session management.

### Tasks
1. **Project Setup**
   - Initialize Cargo workspace with binary and library crates
   - Configure dependencies in `Cargo.toml`
   - Set up directory structure:
     ```
     yinx/
     ├── src/
     │   ├── main.rs           # CLI entry point
     │   ├── lib.rs            # Library root
     │   ├── cli/              # Command parsing
     │   ├── config/           # Configuration management
     │   ├── daemon/           # Daemon lifecycle
     │   ├── session/          # Session state
     │   └── error.rs          # Error types
     ├── tests/
     └── Cargo.toml
     ```

2. **CLI Argument Parsing**
   - Use `clap` v4 with derive macros
   - Commands to implement:
     ```bash
     yinx start [--session <name>]
     yinx stop
     yinx status
     yinx query <text> [--limit N]
     yinx ask <text>
     yinx report [--output <path>]
     yinx export <path>
     yinx config [--show | --set <key> <value>]
     ```
   - Add global flags: `--verbose`, `--config <path>`

3. **Configuration Management**
   - TOML config file at `~/.config/yinx/config.toml`
   - **See ARCHITECTURE.md for complete config schema**
   - Schema (abbreviated):
     ```toml
     [_meta]
     schema_version = "1.0.0"

     [storage]
     data_dir = "~/.yinx"
     max_blob_size = "10MB"

     [capture]
     buffer_size = 10000
     flush_interval = "5s"

     # Load from separate files for organization
     [filtering]
     config_file = "filters.toml"  # tier1/2/3 configs

     [entities]
     config_file = "entities.toml"  # entity patterns

     [tools]
     config_file = "tools.toml"  # tool detection

     [embedding]
     model = "all-MiniLM-L6-v2"
     mode = "offline"
     batch_size = 32

     [llm]
     provider = "groq"
     api_key_env = "GROQ_API_KEY"  # read from env
     model = "llama-3.1-70b"
     enabled = false

     [indexing]
     vector_dim = 384
     hnsw_ef_construction = 200
     hnsw_m = 16

     # Profile support
     [profiles.exam]
     llm.enabled = false
     embedding.mode = "offline"
     ```

   - **Configuration Validation:**
     ```rust
     struct ConfigValidator;

     impl ConfigValidator {
         fn validate(config: &Config) -> Result<(), Vec<ValidationError>> {
             let mut errors = Vec::new();

             // Validate regex patterns compile
             for pattern in &config.entity_patterns {
                 if let Err(e) = Regex::new(&pattern.pattern) {
                     errors.push(ValidationError {
                         path: format!("entities.{}", pattern.type_name),
                         message: format!("Invalid regex: {}", e),
                     });
                 }
             }

             // Validate weights sum to 1.0
             let sum = config.filtering.tier2.entropy_weight
                 + config.filtering.tier2.uniqueness_weight
                 + config.filtering.tier2.technical_weight
                 + config.filtering.tier2.change_weight;

             if (sum - 1.0).abs() > 0.01 {
                 errors.push(ValidationError {
                     path: "filtering.tier2".into(),
                     message: format!("Weights must sum to 1.0, got {}", sum),
                 });
             }

             // Validate file paths exist
             if !config.storage.data_dir.exists() {
                 fs::create_dir_all(&config.storage.data_dir)?;
             }

             // Validate API key if LLM enabled
             if config.llm.enabled {
                 if std::env::var(&config.llm.api_key_env).is_err() {
                     errors.push(ValidationError {
                         path: "llm.api_key_env".into(),
                         message: format!("Env var {} not set", config.llm.api_key_env),
                     });
                 }
             }

             if errors.is_empty() {
                 Ok(())
             } else {
                 Err(errors)
             }
         }
     }
     ```

4. **Session Management**
   - Session struct with metadata:
     ```rust
     struct Session {
         id: Uuid,
         name: String,
         started_at: DateTime<Utc>,
         stopped_at: Option<DateTime<Utc>>,
         capture_count: u64,
         blob_count: u64,
         status: SessionStatus,  // Active, Paused, Stopped
     }
     ```
   - Session state file: `~/.yinx/sessions/<session_id>/state.json`

5. **Logging Infrastructure**
   - Use `tracing` + `tracing-subscriber`
   - Log levels: ERROR, WARN, INFO, DEBUG, TRACE
   - Log targets:
     - Daemon: `~/.yinx/daemon.log` (rotating)
     - User commands: stderr (CLI)

### Deliverables
- ✅ Working CLI that parses all commands
- ✅ Config file loading/saving
- ✅ Session CRUD operations (in-memory, no persistence yet)
- ✅ Basic logging to files

### Dependencies
```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1.8", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
thiserror = "1.0"
```

---

## Phase 2: Storage Architecture

### Goals
Implement content-addressed blob storage, SQLite database, and dual-zone directory structure.

### Tasks
1. **Content-Addressed Blob Storage**
   - Hash function: BLAKE3 (faster than SHA256, 128-bit truncated)
   - Blob structure:
     ```
     ~/.yinx/store/blobs/
     ├── ab/
     │   └── cd/
     │       └── abcdef123456... (hash as filename)
     ```
   - Two-level directory sharding (first 2 bytes) to avoid directory size limits
   - Compression: optional zstd for large blobs

2. **SQLite Schema Design**
   ```sql
   -- Sessions table
   CREATE TABLE sessions (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       started_at INTEGER NOT NULL,
       stopped_at INTEGER,
       status TEXT NOT NULL
   );

   -- Captures table (raw terminal data)
   CREATE TABLE captures (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       session_id TEXT NOT NULL,
       timestamp INTEGER NOT NULL,
       command TEXT,
       output_hash TEXT NOT NULL,  -- BLAKE3 of output
       tool TEXT,  -- detected tool name
       FOREIGN KEY (session_id) REFERENCES sessions(id)
   );

   -- Blobs table (content-addressed storage metadata)
   CREATE TABLE blobs (
       hash TEXT PRIMARY KEY,
       size INTEGER NOT NULL,
       created_at INTEGER NOT NULL,
       compressed BOOLEAN NOT NULL,
       ref_count INTEGER NOT NULL DEFAULT 1
   );

   -- Chunks table (filtered/clustered content for embedding)
   CREATE TABLE chunks (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       capture_id INTEGER NOT NULL,
       blob_hash TEXT NOT NULL,
       representative_text TEXT NOT NULL,
       cluster_size INTEGER DEFAULT 1,
       metadata JSON,  -- cluster members, scores, etc.
       FOREIGN KEY (capture_id) REFERENCES captures(id),
       FOREIGN KEY (blob_hash) REFERENCES blobs(hash)
   );

   -- Embeddings table
   CREATE TABLE embeddings (
       chunk_id INTEGER PRIMARY KEY,
       vector BLOB NOT NULL,  -- serialized float vector
       model TEXT NOT NULL,
       created_at INTEGER NOT NULL,
       FOREIGN KEY (chunk_id) REFERENCES chunks(id)
   );

   -- Entities table
   CREATE TABLE entities (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       capture_id INTEGER NOT NULL,
       type TEXT NOT NULL,  -- ip, port, url, cve, credential, path
       value TEXT NOT NULL,
       context TEXT,  -- surrounding text
       FOREIGN KEY (capture_id) REFERENCES captures(id)
   );

   -- Create indexes
   CREATE INDEX idx_captures_session ON captures(session_id);
   CREATE INDEX idx_captures_timestamp ON captures(timestamp);
   CREATE INDEX idx_entities_type ON entities(type);
   CREATE INDEX idx_entities_value ON entities(value);
   ```

3. **Dual-Zone Directory Structure**
   ```
   ~/.yinx/
   ├── config.toml
   ├── daemon.log
   ├── store/              # Machine zone (rebuildable, internal)
   │   ├── db.sqlite
   │   ├── blobs/
   │   │   └── ab/cd/abcdef...
   │   ├── vectors/        # Vector index data
   │   └── keywords/       # Tantivy index
   └── reports/            # Human zone (stable, shareable)
       └── <session_name>/
           ├── session.json
           ├── findings.md
           ├── evidence/
           │   └── <timestamp>_<tool>.txt
           └── export/
   ```

4. **Blob Operations**
   - Write: hash content → check if exists → write if new → increment ref_count
   - Read: hash → lookup path → read + decompress if needed
   - Delete: decrement ref_count → delete file if ref_count == 0
   - GC: periodic cleanup of unreferenced blobs

5. **Database Operations**
   - Connection pooling with `r2d2` or `deadpool`
   - Write-ahead logging (WAL) mode for concurrency
   - Prepared statements for all queries
   - Transaction batching for bulk inserts

### Deliverables
- ✅ Blob storage with deduplication
- ✅ SQLite database with schema
- ✅ Session persistence (save/load)
- ✅ Database migration system

### Dependencies
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
blake3 = "1.5"
zstd = "0.13"
serde_json = "1.0"
```

---

## Phase 3: Terminal Capture

### Goals
Implement shell integration for capturing commands and outputs in background daemon.

### Tasks
1. **Shell Hook Mechanism**
   - Bash integration:
     ```bash
     # ~/.yinx/hooks/bash_hook.sh
     __yinx_capture() {
         local cmd="$1"
         local output_file="/tmp/yinx_$$_${RANDOM}.out"

         # Execute command and capture output
         eval "$cmd" 2>&1 | tee "$output_file"
         local exit_code="${PIPESTATUS[0]}"

         # Send to yinx daemon
         yinx _internal capture --cmd "$cmd" --output "$output_file" --exit "$exit_code" &

         return $exit_code
     }

     # Hook into PROMPT_COMMAND
     PROMPT_COMMAND="__yinx_capture_last; $PROMPT_COMMAND"
     ```

   - Zsh integration similar with `preexec` and `precmd` hooks

2. **Daemon Architecture**
   - Use `daemonize` crate or manual fork/setsid
   - PID file at `~/.yinx/daemon.pid`
   - Lock file to prevent multiple instances
   - Signal handling:
     - SIGTERM/SIGINT: graceful shutdown
     - SIGHUP: reload config
     - SIGUSR1: flush buffers

3. **IPC Server (Unix Domain Socket)**
   - Socket at `/tmp/yinx_${USER}.sock`
   - Protocol: length-prefixed JSON messages
   - Message types:
     ```rust
     enum IpcMessage {
         Capture {
             command: String,
             output: Vec<u8>,
             exit_code: i32,
             timestamp: i64,
         },
         Query {
             text: String,
             limit: usize,
         },
         Status,
         Shutdown,
     }
     ```

4. **Async Processing Pipeline**
   - Use `tokio` runtime
   - Channel-based pipeline:
     ```
     IPC → mpsc::channel → Filter Pipeline → Storage
     ```
   - Bounded channels to prevent memory exhaustion
   - Backpressure handling

5. **Buffering & Flushing**
   - Configurable buffer size (default: 10,000 lines)
   - Time-based flushing (default: 5 seconds)
   - Manual flush on `SIGUSR1`

### Deliverables
- ✅ Daemon that starts/stops/status
- ✅ Shell hooks for bash/zsh
- ✅ IPC communication working
- ✅ Captured data reaches storage

### Dependencies
```toml
tokio = { version = "1.37", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"  # Fast binary serialization
nix = "0.28"  # Unix signals
```

---

## Phase 4: Three-Tier Filtering Pipeline

### Goals
Reduce 100K lines → ~100 semantic chunks while preserving all important information.

### ⚠️ Configuration-First Implementation

**Load all filtering parameters from config.toml:**
- Normalization patterns and replacements
- Scoring weights and thresholds
- Technical pattern definitions

### Tasks

#### Tier 1: Anomaly Detection (Hash-Based Deduplication)

**Algorithm:**
```rust
struct Tier1Config {
    max_occurrences: u32,
    normalization_patterns: Vec<NormalizationPattern>,
}

struct NormalizationPattern {
    regex: Regex,
    replacement: String,
    name: String,
}

struct Tier1Filter {
    config: Tier1Config,
    pattern_counts: HashMap<u64, u32>,
}

impl Tier1Filter {
    fn new(config: Tier1Config) -> Self {
        Self {
            config,
            pattern_counts: HashMap::new(),
        }
    }

    fn process_line(&mut self, line: &str) -> FilterDecision {
        let pattern = self.normalize_pattern(line);
        let hash = self.hash_pattern(&pattern);

        let count = self.pattern_counts.entry(hash).or_insert(0);
        *count += 1;

        if *count <= self.config.max_occurrences {
            FilterDecision::Keep
        } else {
            FilterDecision::Discard
        }
    }

    fn normalize_pattern(&self, line: &str) -> String {
        let mut normalized = line.to_string();

        // Apply normalization patterns from config
        for pattern in &self.config.normalization_patterns {
            normalized = pattern.regex.replace_all(&normalized, &pattern.replacement).to_string();
        }

        normalized
    }
}
```

**Performance Target:** <1ms per line
**Reduction:** 100K → ~10K (90%)

#### Tier 2: Statistical Importance Scoring

**Algorithm:**
```rust
struct Tier2Config {
    entropy_weight: f32,
    uniqueness_weight: f32,
    technical_weight: f32,
    change_weight: f32,
    score_threshold_percentile: f32,
    technical_patterns: Vec<TechnicalPattern>,
    max_technical_score: f32,
}

struct TechnicalPattern {
    name: String,
    regex: Regex,
    weight: f32,
}

struct Tier2Filter {
    config: Tier2Config,
    line_frequencies: HashMap<String, u32>,
    total_lines: u32,
}

impl Tier2Filter {
    fn new(config: Tier2Config) -> Self {
        Self {
            config,
            line_frequencies: HashMap::new(),
            total_lines: 0,
        }
    }

    fn score_line(&self, line: &str, prev_line: Option<&str>) -> f32 {
        let entropy_score = self.calculate_entropy(line) * self.config.entropy_weight;
        let uniqueness_score = self.calculate_uniqueness(line) * self.config.uniqueness_weight;
        let technical_score = self.calculate_technical_density(line) * self.config.technical_weight;
        let change_score = self.calculate_change(line, prev_line) * self.config.change_weight;

        entropy_score + uniqueness_score + technical_score + change_score
    }

    fn calculate_entropy(&self, line: &str) -> f32 {
        // Shannon entropy of character distribution
        let mut freq: HashMap<char, u32> = HashMap::new();
        for c in line.chars() {
            *freq.entry(c).or_insert(0) += 1;
        }

        let len = line.len() as f32;
        -freq.values().map(|&count| {
            let p = count as f32 / len;
            p * p.log2()
        }).sum::<f32>()
    }

    fn calculate_uniqueness(&self, line: &str) -> f32 {
        let freq = self.line_frequencies.get(line).unwrap_or(&1);
        1.0 - (*freq as f32 / self.total_lines as f32)
    }

    fn calculate_technical_density(&self, line: &str) -> f32 {
        // Use patterns from config
        let weighted_matches: f32 = self.config.technical_patterns.iter()
            .map(|p| p.regex.find_iter(line).count() as f32 * p.weight)
            .sum();

        (weighted_matches / self.config.max_technical_score).min(1.0)
    }

    fn calculate_change(&self, line: &str, prev: Option<&str>) -> f32 {
        match prev {
            Some(p) => 1.0 - self.string_similarity(line, p),
            None => 1.0,
        }
    }
}
```

**Filter:** Keep top 20% by score
**Performance Target:** ~0.1ms per line
**Reduction:** 10K → ~2K (80%)

#### Tier 3: Semantic Clustering

**Algorithm:**
```rust
struct Tier3Filter;

impl Tier3Filter {
    fn cluster_lines(&self, lines: Vec<String>) -> Vec<Cluster> {
        // Group by normalized pattern
        let mut clusters: HashMap<String, Vec<String>> = HashMap::new();

        for line in lines {
            let pattern = self.normalize_for_clustering(&line);
            clusters.entry(pattern).or_insert_with(Vec::new).push(line);
        }

        // Create representatives
        clusters.into_iter().map(|(pattern, members)| {
            Cluster {
                pattern,
                representative: members[0].clone(),
                members,
                metadata: serde_json::json!({
                    "count": members.len(),
                }),
            }
        }).collect()
    }

    fn normalize_for_clustering(&self, line: &str) -> String {
        // More aggressive normalization than Tier 1
        // Keep semantic structure, replace specific values
    }
}

struct Cluster {
    pattern: String,
    representative: String,
    members: Vec<String>,
    metadata: serde_json::Value,
}
```

**Output:** ~100 cluster representatives
**Performance Target:** ~50ms total
**Reduction:** 2K → ~100 (95%)

### Pipeline Orchestration
```rust
async fn process_capture(data: CaptureData) -> Vec<Chunk> {
    let tier1 = Tier1Filter::new();
    let tier2 = Tier2Filter::new();
    let tier3 = Tier3Filter::new();

    // Tier 1: streaming
    let tier1_output: Vec<String> = data.lines()
        .filter(|line| tier1.process_line(line).is_keep())
        .collect();

    // Tier 2: batch scoring
    let scores: Vec<f32> = tier1_output.iter()
        .enumerate()
        .map(|(i, line)| {
            let prev = if i > 0 { Some(&tier1_output[i-1]) } else { None };
            tier2.score_line(line, prev.map(|s| s.as_str()))
        })
        .collect();

    let threshold = percentile(&scores, 0.8);  // top 20%
    let tier2_output: Vec<String> = tier1_output.into_iter()
        .zip(scores)
        .filter(|(_, score)| *score >= threshold)
        .map(|(line, _)| line)
        .collect();

    // Tier 3: clustering
    tier3.cluster_lines(tier2_output)
}
```

### Deliverables
- ✅ Three-tier filtering implemented
- ✅ Configurable thresholds
- ✅ Performance benchmarks (100K lines in <500ms)
- ✅ Unit tests with real tool outputs

### Dependencies
```toml
regex = "1.10"
ahash = "0.8"  # Fast hashing
```

---

## Phase 5: Entity Extraction & Metadata

### Goals
Extract structured entities from filtered text and enrich with metadata.

### ⚠️ Configuration-First Implementation

**Load entity patterns and tool detection from config:**
- Entity definitions with patterns, confidence, context windows
- Tool detection patterns and output parsers
- Plugin support for custom extractors

### Tasks

1. **Entity Registry (Config-Based)**
   ```rust
   struct EntityConfig {
       type_name: String,
       pattern: String,
       confidence: f32,
       context_window: usize,
       redact: bool,  // mask in outputs
       metadata: HashMap<String, Value>,
   }

   struct EntityRegistry {
       extractors: Vec<EntityExtractor>,
   }

   impl EntityRegistry {
       fn from_config(configs: Vec<EntityConfig>) -> Result<Self> {
           let mut extractors = Vec::new();

           for config in configs {
               let regex = Regex::new(&config.pattern)?;
               extractors.push(EntityExtractor {
                   type_name: config.type_name,
                   regex,
                   confidence: config.confidence,
                   context_window: config.context_window,
                   redact: config.redact,
               });
           }

           Ok(Self { extractors })
       }

       fn extract_all(&self, text: &str) -> Vec<Entity> {
           self.extractors.iter()
               .flat_map(|e| e.extract(text))
               .collect()
       }
   }

   struct EntityExtractor {
       type_name: String,
       regex: Regex,
       confidence: f32,
       context_window: usize,
       redact: bool,
   }

   impl EntityExtractor {
       fn extract(&self, text: &str) -> Vec<Entity> {
           self.regex.find_iter(text)
               .map(|m| {
                   let context = Self::get_context(text, m.start(), m.end(), self.context_window);
                   Entity {
                       type_name: self.type_name.clone(),
                       value: m.as_str().to_string(),
                       context,
                       confidence: self.confidence,
                       redact: self.redact,
                   }
               })
               .collect()
       }
   }

   struct Entity {
       type_name: String,  // Dynamic, not enum
       value: String,
       context: String,
       confidence: f32,
       redact: bool,
   }
   ```

2. **Tool Detection Registry (Config-Based)**
   ```rust
   struct ToolConfig {
       name: String,
       patterns: Vec<String>,  // command patterns
       entity_hints: Vec<String>,  // likely entity types
       output_patterns: Vec<OutputPattern>,
   }

   struct OutputPattern {
       pattern: String,
       section: String,  // categorize output
   }

   struct ToolDetector {
       tools: Vec<ToolMatcher>,
   }

   impl ToolDetector {
       fn from_config(configs: Vec<ToolConfig>) -> Result<Self> {
           let mut tools = Vec::new();

           for config in configs {
               let command_regexes: Vec<Regex> = config.patterns.iter()
                   .map(|p| Regex::new(p))
                   .collect::<Result<Vec<_>, _>>()?;

               let output_regexes: Vec<(Regex, String)> = config.output_patterns.iter()
                   .map(|op| Ok((Regex::new(&op.pattern)?, op.section.clone())))
                   .collect::<Result<Vec<_>, _>>()?;

               tools.push(ToolMatcher {
                   name: config.name,
                   command_patterns: command_regexes,
                   entity_hints: config.entity_hints,
                   output_patterns: output_regexes,
               });
           }

           Ok(Self { tools })
       }

       fn detect(&self, command: &str) -> Option<&ToolMatcher> {
           self.tools.iter()
               .find(|tool| tool.matches_command(command))
       }
   }

   struct ToolMatcher {
       name: String,
       command_patterns: Vec<Regex>,
       entity_hints: Vec<String>,
       output_patterns: Vec<(Regex, String)>,
   }

   impl ToolMatcher {
       fn matches_command(&self, command: &str) -> bool {
           self.command_patterns.iter()
               .any(|p| p.is_match(command))
       }

       fn categorize_output(&self, output: &str) -> HashMap<String, Vec<String>> {
           let mut sections: HashMap<String, Vec<String>> = HashMap::new();

           for line in output.lines() {
               for (pattern, section) in &self.output_patterns {
                   if pattern.is_match(line) {
                       sections.entry(section.clone())
                           .or_insert_with(Vec::new)
                           .push(line.to_string());
                   }
               }
           }

           sections
       }
   }
   ```

4. **Host/Service Correlation**
   ```rust
   struct ContextGraph {
       hosts: HashMap<IpAddr, HostInfo>,
   }

   struct HostInfo {
       ip: IpAddr,
       hostnames: Vec<String>,
       ports: HashMap<u16, ServiceInfo>,
       first_seen: DateTime<Utc>,
       last_seen: DateTime<Utc>,
   }

   struct ServiceInfo {
       port: u16,
       protocol: String,
       service: String,
       version: Option<String>,
       vulnerabilities: Vec<String>,
   }
   ```

5. **Metadata Enrichment**
   - Capture-level metadata:
     - Tool name, command, exit code
     - Timestamp, duration
     - Entity counts by type
   - Chunk-level metadata:
     - Cluster size, pattern
     - Score components (entropy, uniqueness, etc.)
     - Related entities

### Deliverables
- ✅ Entity extraction for all types
- ✅ Tool detection working
- ✅ Host/service graph populated
- ✅ Metadata stored in database

### Dependencies
```toml
regex = "1.10"
lazy_static = "1.4"
```

---

## Phase 6: Embedding & Indexing

### Goals
Generate embeddings for semantic search and build keyword index for exact matches.

### Tasks

1. **Embedding Model Integration**
   - **Primary**: `fastembed-rs` (ONNX runtime, fast)
   - **Fallback**: `candle` with sentence-transformers
   - Model: `all-MiniLM-L6-v2` (384 dims, 80MB, offline-capable)
   - Alternative for online: `text-embedding-3-small` via OpenAI API

   ```rust
   trait EmbeddingProvider {
       fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
       fn dimension(&self) -> usize;
   }

   struct FastEmbedProvider {
       model: TextEmbedding,
   }

   impl EmbeddingProvider for FastEmbedProvider {
       fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
           self.model.embed(texts, None)
       }

       fn dimension(&self) -> usize {
           384  // all-MiniLM-L6-v2
       }
   }
   ```

2. **Vector Database Setup**
   - **Option A**: Qdrant embedded mode (simplest)
   - **Option B**: Custom HNSW with `usearch` or `instant-distance`

   ```rust
   // Qdrant approach
   use qdrant_client::Qdrant;

   struct VectorIndex {
       client: Qdrant,
       collection: String,
   }

   impl VectorIndex {
       async fn new(path: &Path) -> Result<Self> {
           let client = Qdrant::from_url("http://localhost:6334").build()?;

           // Create collection
           client.create_collection(
               CreateCollection {
                   collection_name: "yinx_embeddings".into(),
                   vectors_config: Some(VectorsConfig {
                       size: 384,
                       distance: Distance::Cosine,
                       ..Default::default()
                   }),
                   ..Default::default()
               }
           ).await?;

           Ok(Self {
               client,
               collection: "yinx_embeddings".into(),
           })
       }

       async fn insert(&self, chunk_id: i64, vector: Vec<f32>) -> Result<()> {
           self.client.upsert_points(
               self.collection.clone(),
               vec![PointStruct::new(
                   chunk_id as u64,
                   vector,
                   serde_json::json!({ "chunk_id": chunk_id }),
               )],
               None,
           ).await?;
           Ok(())
       }

       async fn search(&self, query: Vec<f32>, limit: usize) -> Result<Vec<ScoredPoint>> {
           let results = self.client.search_points(
               SearchPoints {
                   collection_name: self.collection.clone(),
                   vector: query,
                   limit: limit as u64,
                   with_payload: Some(true.into()),
                   ..Default::default()
               }
           ).await?;
           Ok(results.result)
       }
   }
   ```

3. **Keyword Index (Tantivy)**
   ```rust
   use tantivy::*;

   struct KeywordIndex {
       index: Index,
       schema: Schema,
   }

   impl KeywordIndex {
       fn new(path: &Path) -> Result<Self> {
           let mut schema_builder = Schema::builder();
           schema_builder.add_i64_field("chunk_id", STORED | INDEXED);
           schema_builder.add_text_field("text", TEXT | STORED);
           let schema = schema_builder.build();

           let index = Index::create_in_dir(path, schema.clone())?;

           Ok(Self { index, schema })
       }

       fn insert(&self, chunk_id: i64, text: &str) -> Result<()> {
           let mut index_writer = self.index.writer(50_000_000)?;

           let chunk_id_field = self.schema.get_field("chunk_id").unwrap();
           let text_field = self.schema.get_field("text").unwrap();

           index_writer.add_document(doc!(
               chunk_id_field => chunk_id,
               text_field => text,
           ))?;

           index_writer.commit()?;
           Ok(())
       }

       fn search(&self, query: &str, limit: usize) -> Result<Vec<(i64, f32)>> {
           let reader = self.index.reader()?;
           let searcher = reader.searcher();

           let text_field = self.schema.get_field("text").unwrap();
           let query_parser = QueryParser::for_index(&self.index, vec![text_field]);
           let query = query_parser.parse_query(query)?;

           let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

           let chunk_id_field = self.schema.get_field("chunk_id").unwrap();
           let results = top_docs.iter().map(|(score, doc_addr)| {
               let doc = searcher.doc(*doc_addr).unwrap();
               let chunk_id = doc.get_first(chunk_id_field).unwrap().as_i64().unwrap();
               (chunk_id, *score)
           }).collect();

           Ok(results)
       }
   }
   ```

4. **Batch Processing**
   ```rust
   async fn index_chunks(chunks: Vec<Chunk>) -> Result<()> {
       const BATCH_SIZE: usize = 32;

       for batch in chunks.chunks(BATCH_SIZE) {
           let texts: Vec<String> = batch.iter()
               .map(|c| c.representative_text.clone())
               .collect();

           let embeddings = embedding_provider.embed(&texts)?;

           for (chunk, embedding) in batch.iter().zip(embeddings) {
               vector_index.insert(chunk.id, embedding).await?;
               keyword_index.insert(chunk.id, &chunk.representative_text)?;
           }
       }

       Ok(())
   }
   ```

### Deliverables
- ✅ Embedding generation working (local model)
- ✅ Vector index operational
- ✅ Keyword index operational
- ✅ Batch processing pipeline

### Dependencies
```toml
fastembed = "3.6"
qdrant-client = "1.9"
tantivy = "0.22"
```

---

## Phase 7: Hybrid Retrieval & Reranking

### Goals
Combine semantic and keyword search, then rerank for precision.

### Tasks

1. **Parallel Search**
   ```rust
   async fn hybrid_search(query: &str, limit: usize) -> Result<Vec<ScoredChunk>> {
       // Parallel execution
       let (semantic_results, keyword_results) = tokio::join!(
           semantic_search(query, limit * 2),
           keyword_search(query, limit * 2),
       );

       let semantic = semantic_results?;
       let keyword = keyword_results?;

       // Fusion
       let fused = reciprocal_rank_fusion(semantic, keyword);

       // Rerank
       let reranked = rerank(query, fused, limit).await?;

       Ok(reranked)
   }
   ```

2. **Reciprocal Rank Fusion (RRF)**
   ```rust
   fn reciprocal_rank_fusion(
       semantic: Vec<(i64, f32)>,
       keyword: Vec<(i64, f32)>,
   ) -> Vec<(i64, f32)> {
       const K: f32 = 60.0;
       let mut scores: HashMap<i64, f32> = HashMap::new();

       // Semantic results
       for (rank, (chunk_id, _score)) in semantic.iter().enumerate() {
           *scores.entry(*chunk_id).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
       }

       // Keyword results
       for (rank, (chunk_id, _score)) in keyword.iter().enumerate() {
           *scores.entry(*chunk_id).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
       }

       // Sort by fused score
       let mut results: Vec<_> = scores.into_iter().collect();
       results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

       results
   }
   ```

3. **Cross-Encoder Reranker**
   ```rust
   use fastembed::TextRerank;

   struct Reranker {
       model: TextRerank,
   }

   impl Reranker {
       fn new() -> Result<Self> {
           let model = TextRerank::try_new(Default::default())?;
           Ok(Self { model })
       }

       async fn rerank(
           &self,
           query: &str,
           candidates: Vec<ScoredChunk>,
           limit: usize,
       ) -> Result<Vec<ScoredChunk>> {
           let texts: Vec<String> = candidates.iter()
               .map(|c| c.text.clone())
               .collect();

           let scores = self.model.rerank(query, &texts, true, None)?;

           let mut reranked: Vec<_> = candidates.into_iter()
               .zip(scores)
               .map(|(chunk, score)| ScoredChunk {
                   score: score.score,
                   ..chunk
               })
               .collect();

           reranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
           reranked.truncate(limit);

           Ok(reranked)
       }
   }
   ```

4. **Deduplication**
   ```rust
   fn deduplicate(results: Vec<ScoredChunk>) -> Vec<ScoredChunk> {
       let mut seen: HashSet<i64> = HashSet::new();
       results.into_iter()
           .filter(|chunk| seen.insert(chunk.id))
           .collect()
   }
   ```

5. **Citation Tracking**
   ```rust
   struct ScoredChunk {
       id: i64,
       text: String,
       score: f32,
       metadata: ChunkMetadata,
       provenance: Provenance,
   }

   struct Provenance {
       capture_id: i64,
       blob_hash: String,
       command: String,
       timestamp: DateTime<Utc>,
       tool: String,
   }
   ```

### Deliverables
- ✅ Hybrid search working
- ✅ RRF fusion implemented
- ✅ Reranking operational
- ✅ Provenance tracking complete

### Dependencies
```toml
fastembed = "3.6"  # includes reranker
```

---

## Phase 8: Query Interface

### Goals
Implement user-facing query commands with clean output.

### Tasks

1. **`yinx query` Command**
   ```rust
   async fn cmd_query(query: &str, limit: usize) -> Result<()> {
       let results = hybrid_search(query, limit).await?;

       println!("Found {} results:\n", results.len());

       for (i, result) in results.iter().enumerate() {
           println!("{}. [Score: {:.3}] {}",
               i + 1,
               result.score,
               result.text.lines().next().unwrap_or(""));

           println!("   Tool: {} | Time: {}",
               result.provenance.tool,
               result.provenance.timestamp.format("%Y-%m-%d %H:%M:%S"));

           println!("   Command: {}\n", result.provenance.command);
       }

       Ok(())
   }
   ```

2. **`yinx ask` Command**
   ```rust
   async fn cmd_ask(question: &str) -> Result<()> {
       // Retrieve context
       let context_chunks = hybrid_search(question, 20).await?;

       // Format context
       let context = context_chunks.iter()
           .map(|c| format!("[{}] {}\nCommand: {}\n",
               c.provenance.timestamp.format("%H:%M:%S"),
               c.text,
               c.provenance.command))
           .collect::<Vec<_>>()
           .join("\n---\n");

       // Check if LLM enabled
       let config = Config::load()?;
       if !config.llm.enabled {
           println!("LLM not enabled. Showing retrieved context:\n");
           println!("{}", context);
           return Ok(());
       }

       // Call LLM
       let response = llm_client.ask(question, &context).await?;

       println!("{}\n", response);
       println!("---");
       println!("Sources: {} chunks from session", context_chunks.len());

       Ok(())
   }
   ```

3. **LLM Client (Groq)**
   ```rust
   struct GroqClient {
       api_key: String,
       model: String,
       client: reqwest::Client,
   }

   impl GroqClient {
       async fn ask(&self, question: &str, context: &str) -> Result<String> {
           let system_prompt = "You are a penetration testing assistant. \
               Use the provided terminal output context to answer questions. \
               Cite specific commands and outputs when possible.";

           let user_prompt = format!(
               "Question: {}\n\nContext from terminal session:\n{}",
               question, context
           );

           let response = self.client
               .post("https://api.groq.com/openai/v1/chat/completions")
               .header("Authorization", format!("Bearer {}", self.api_key))
               .json(&serde_json::json!({
                   "model": self.model,
                   "messages": [
                       {"role": "system", "content": system_prompt},
                       {"role": "user", "content": user_prompt},
                   ],
                   "temperature": 0.1,
               }))
               .send()
               .await?;

           let data: serde_json::Value = response.json().await?;
           let text = data["choices"][0]["message"]["content"]
               .as_str()
               .ok_or_else(|| anyhow::anyhow!("No response"))?;

           Ok(text.to_string())
       }
   }
   ```

4. **Configuration Management**
   - API key storage (secure, not in plain config)
   - Mode switching (offline/online)
   - Model selection

### Deliverables
- ✅ `yinx query` working with formatted output
- ✅ `yinx ask` working (with/without LLM)
- ✅ Groq integration operational
- ✅ Clean, readable output

### Dependencies
```toml
reqwest = { version = "0.12", features = ["json"] }
```

---

## Phase 9: Report Generation

### Goals
Generate structured markdown reports with evidence citations.

### Tasks

1. **Report Template**
   ```markdown
   # Penetration Test Report
   **Session:** {session_name}
   **Date:** {date_range}
   **Duration:** {duration}

   ## Executive Summary
   {auto_generated_summary}

   ## Discovered Assets
   ### Hosts
   - {ip} ({hostname})
     - Ports: {ports}
     - Services: {services}

   ## Findings
   ### {severity}: {title}
   **Affected Asset:** {ip}:{port}
   **Description:** {description}
   **Evidence:**
   ```
   {command_output}
   ```
   **Source:** Command executed at {timestamp}

   ## Credentials Discovered
   | Type | Value | Source |
   |------|-------|--------|
   | {type} | {value} | {command} |

   ## Appendix: Full Evidence
   - {timestamp}_{tool}.txt
   ```

2. **Report Generator**
   ```rust
   struct ReportGenerator {
       session: Session,
       db: Database,
   }

   impl ReportGenerator {
       async fn generate(&self) -> Result<Report> {
           let captures = self.db.get_captures(self.session.id).await?;
           let entities = self.db.get_entities(self.session.id).await?;

           // Group by hosts
           let hosts = self.group_by_host(&entities);

           // Generate findings
           let findings = self.extract_findings(&captures, &entities);

           // Generate summary (optional LLM)
           let summary = self.generate_summary(&findings).await?;

           Ok(Report {
               session: self.session.clone(),
               summary,
               hosts,
               findings,
               entities,
           })
       }

       fn render_markdown(&self, report: &Report) -> String {
           // Use template engine or manual formatting
           format!("# Penetration Test Report\n...")
       }
   }
   ```

3. **Evidence Export**
   ```rust
   async fn export_evidence(session_id: &str, output_dir: &Path) -> Result<()> {
       let captures = db.get_captures(session_id).await?;

       for capture in captures {
           let blob = blob_store.read(&capture.output_hash)?;

           let filename = format!(
               "{}_{}_{}.txt",
               capture.timestamp.format("%Y%m%d_%H%M%S"),
               capture.tool,
               capture.id,
           );

           let path = output_dir.join("evidence").join(filename);
           fs::write(path, blob)?;
       }

       Ok(())
   }
   ```

4. **Export Bundle**
   ```rust
   async fn export_bundle(session_id: &str, output_path: &Path) -> Result<()> {
       let temp_dir = tempdir()?;

       // Copy database
       fs::copy(
           db_path(),
           temp_dir.path().join("db.sqlite"),
       )?;

       // Copy referenced blobs
       let blobs = db.get_blob_hashes(session_id).await?;
       for hash in blobs {
           let blob = blob_store.read(&hash)?;
           let blob_path = temp_dir.path().join("blobs").join(&hash);
           fs::create_dir_all(blob_path.parent().unwrap())?;
           fs::write(blob_path, blob)?;
       }

       // Generate checksums
       let checksums = generate_checksums(&temp_dir)?;
       fs::write(temp_dir.path().join("checksums.txt"), checksums)?;

       // Create tarball
       create_tarball(temp_dir.path(), output_path)?;

       Ok(())
   }
   ```

### Deliverables
- ✅ Report generation working
- ✅ Evidence export functional
- ✅ Export bundle creation
- ✅ Markdown templates customizable

### Dependencies
```toml
tera = "1.19"  # Template engine
tar = "0.4"
```

---

## Phase 10: Polish & Documentation

### Goals
Production-ready tool with comprehensive documentation.

### Tasks

1. **README.md**
   - Project overview
   - Quick start guide
   - Installation instructions
   - Usage examples
   - Architecture diagram
   - FAQ

2. **ARCHITECTURE.md**
   - System design
   - Data flow diagrams
   - Component descriptions
   - Technology choices and rationale
   - Performance characteristics

3. **INSTALLATION.md**
   - Cargo install
   - Binary releases
   - Shell integration setup
   - Configuration guide
   - Troubleshooting

4. **Shell Integration Scripts**
   ```bash
   # install.sh
   #!/bin/bash

   # Detect shell
   SHELL_NAME=$(basename "$SHELL")

   case "$SHELL_NAME" in
       bash)
           echo "source ~/.yinx/hooks/bash_hook.sh" >> ~/.bashrc
           ;;
       zsh)
           echo "source ~/.yinx/hooks/zsh_hook.sh" >> ~/.zshrc
           ;;
       *)
           echo "Unsupported shell: $SHELL_NAME"
           exit 1
           ;;
   esac

   echo "Yinx installed. Restart your shell."
   ```

5. **CI/CD Setup**
   - GitHub Actions workflow
   - Cross-platform builds (Linux, macOS)
   - Automated testing
   - Release automation with binaries

6. **Testing Suite**
   ```rust
   // Unit tests
   #[cfg(test)]
   mod tests {
       #[test]
       fn test_tier1_filter() {
           // Test with synthetic data
       }

       #[tokio::test]
       async fn test_hybrid_search() {
           // Test retrieval pipeline
       }
   }

   // Integration tests
   #[tokio::test]
   async fn test_end_to_end_capture() {
       // Simulate capture → filter → index → query
   }
   ```

7. **Security Considerations Doc**
   - Credential handling (detection, storage, redaction)
   - Local-only data by default
   - API key security
   - Encrypted storage options
   - Exam compliance (offline mode)

### Deliverables
- ✅ Comprehensive documentation
- ✅ Installation scripts
- ✅ CI/CD pipeline
- ✅ Test coverage >80%
- ✅ Security audit checklist

---

## Technology Stack Summary

### Core
- **Language:** Rust (stable)
- **Async Runtime:** tokio
- **CLI:** clap v4
- **Config:** serde + toml

### Storage
- **Database:** SQLite (rusqlite)
- **Hashing:** BLAKE3
- **Compression:** zstd

### Indexing
- **Embeddings:** fastembed-rs (ONNX)
- **Vector DB:** Qdrant (embedded mode)
- **Keyword Search:** tantivy
- **Reranker:** fastembed (cross-encoder)

### External Services
- **LLM:** Groq API (optional, llama-3.1-70b)

### Utilities
- **Regex:** regex
- **Hashing:** ahash
- **IPC:** Unix domain sockets (nix)
- **Templates:** tera
- **HTTP:** reqwest

---

## Performance Targets

| Operation | Target | Rationale |
|-----------|--------|-----------|
| Capture latency | <10ms | Zero impact on user workflow |
| Filter 100K lines | <500ms | Real-time processing |
| Embed 100 chunks | <2s | Batch processing acceptable |
| Query response | <200ms | Interactive feel |
| Index 1GB session | <5min | Background acceptable |

---

## Critical Success Factors

1. **Zero-latency capture:** Async queue, no blocking I/O
2. **Configuration-driven:** No hardcoded patterns, weights, or thresholds
3. **Adaptive filtering:** Works for any tool via config/plugins
4. **Offline capability:** Full functionality without internet
5. **Provenance integrity:** Every result traceable to source
6. **Security:** No credential leakage, safe for exams
7. **Extensibility:** Plugin architecture for custom extractors
8. **Validation:** Config validated on load with clear errors

---

## Future Enhancements (Post-MVP)

- Multi-session correlation
- Graph visualization of attack paths
- Plugin system for custom extractors
- Web UI for browsing sessions
- Collaborative features (shared sessions)
- MITRE ATT&CK mapping
- Automated exploitation suggestions
- Integration with other tools (Metasploit, Burp)

---

## Implementation Timeline Estimate

| Phase | Estimated Effort | Dependencies |
|-------|-----------------|--------------|
| 1. Foundation | 3-5 days | None |
| 2. Storage | 4-6 days | Phase 1 |
| 3. Capture | 5-7 days | Phase 1, 2 |
| 4. Filtering | 7-10 days | Phase 2 |
| 5. Entities | 4-6 days | Phase 4 |
| 6. Indexing | 6-8 days | Phase 4, 5 |
| 7. Retrieval | 5-7 days | Phase 6 |
| 8. Query UI | 3-5 days | Phase 7 |
| 9. Reports | 4-6 days | Phase 5, 7 |
| 10. Polish | 5-7 days | All |
| **Total** | **46-67 days** | **~2-3 months** |

*Assumes single developer, full-time work. Adjust based on available time.*

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Embedding model size | Use quantized models, lazy loading |
| Vector DB memory | Disk-backed index, configurable cache |
| Shell hook compatibility | Test on multiple shells/OSes |
| Credential leakage | Regex detection + redaction |
| Performance degradation | Benchmarks + profiling |
| IPC security | Unix socket with file permissions |

---

## Success Metrics

- [ ] Captures 100K lines without dropping data
- [ ] Filters to <1% of original volume while retaining all findings
- [ ] Retrieves relevant results in <200ms
- [ ] Works completely offline
- [ ] Zero false negatives for important entities (CVEs, credentials)
- [ ] User completes pentest without manual notes

---

**End of Plan**

This plan provides a comprehensive roadmap for implementing yinx from foundation to production-ready tool. Each phase builds on the previous, with clear deliverables and dependencies.
