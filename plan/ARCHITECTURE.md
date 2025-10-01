# Yinx - Architecture & Design Principles

**Author:** neur0map
**Last Updated:** 2025-10-01

---

## Design Principles

### 1. Configuration Over Hardcoding

**Problem:** Hardcoded values make the system inflexible and require recompilation for changes.

**Solution:** Externalize all tunable parameters, patterns, and behaviors to configuration files.

#### What Should Be Configurable

**Filtering Pipeline:**
```toml
[filtering.tier1]
max_occurrences = 3
normalization_patterns = [
    { pattern = '\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b', replacement = '__IP__' },
    { pattern = '\b\d{1,5}/(tcp|udp)\b', replacement = '__PORT__' },
    { pattern = 'https?://[^\s]+', replacement = '__URL__' },
    { pattern = '\b[0-9a-f]{32,64}\b', replacement = '__HASH__' },
    { pattern = '\b\d+\b', replacement = '__NUM__' },
]

[filtering.tier2]
entropy_weight = 0.3
uniqueness_weight = 0.3
technical_weight = 0.2
change_weight = 0.2
score_threshold_percentile = 0.8  # top 20%
technical_patterns = [
    { pattern = '\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b', weight = 1.0, name = 'ip' },
    { pattern = '\b\d{1,5}/(tcp|udp)\b', weight = 1.0, name = 'port' },
    { pattern = 'https?://', weight = 1.0, name = 'url' },
    { pattern = 'CVE-\d{4}-\d{4,}', weight = 2.0, name = 'cve' },
    { pattern = '\b[0-9a-f]{32,}\b', weight = 0.8, name = 'hash' },
]
max_technical_score = 10.0

[filtering.tier3]
cluster_min_size = 2
max_cluster_size = 1000
```

**Entity Extraction:**
```toml
[[entities]]
type = "ip_address"
pattern = '\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b'
confidence = 0.95
context_window = 50  # chars before/after

[[entities]]
type = "port"
pattern = '\b(\d{1,5})/(tcp|udp)\b'
confidence = 0.9
context_window = 30

[[entities]]
type = "cve"
pattern = 'CVE-\d{4}-\d{4,}'
confidence = 1.0
context_window = 100

[[entities]]
type = "credential"
pattern = '(password|pwd|pass)\s*[:=]\s*\S+'
confidence = 0.7
context_window = 80
redact = true  # mask in reports

[[entities]]
type = "url"
pattern = 'https?://[^\s]+'
confidence = 0.85
context_window = 40

[[entities]]
type = "email"
pattern = '\b[\w\.-]+@[\w\.-]+\.\w+\b'
confidence = 0.9
context_window = 30

[[entities]]
type = "file_path"
pattern = '(/[\w\-./]+)|([A-Z]:\\[\w\-\\]+)'
confidence = 0.75
context_window = 40

[[entities]]
type = "hash_md5"
pattern = '\b[0-9a-f]{32}\b'
confidence = 0.8
context_window = 20

[[entities]]
type = "hash_sha256"
pattern = '\b[0-9a-f]{64}\b'
confidence = 0.85
context_window = 20
```

**Tool Detection:**
```toml
[[tools]]
name = "nmap"
patterns = ['^nmap\b', '--script', '-sV', '-sC']
entity_hints = ["port", "service", "ip_address"]
output_patterns = [
    { pattern = 'PORT\s+STATE\s+SERVICE', section = 'port_scan' },
    { pattern = '\d+/tcp\s+open', section = 'open_port' },
]

[[tools]]
name = "gobuster"
patterns = ['^gobuster\b', 'gobuster dir', 'gobuster dns']
entity_hints = ["url", "file_path"]
output_patterns = [
    { pattern = 'Status:\s+200', section = 'found' },
    { pattern = 'Status:\s+403', section = 'forbidden' },
]

[[tools]]
name = "hydra"
patterns = ['^hydra\b', 'Hydra v']
entity_hints = ["credential", "ip_address", "port"]
output_patterns = [
    { pattern = '\[.*\]\[.*\] host:.*login:.*password:', section = 'valid_cred' },
]

[[tools]]
name = "sqlmap"
patterns = ['^sqlmap\b', 'sqlmap.py']
entity_hints = ["url", "parameter"]
output_patterns = [
    { pattern = 'sqlmap identified', section = 'vulnerable' },
]

# Add more tools via config without code changes
```

**Model Configuration:**
```toml
[embedding]
provider = "fastembed"  # or "openai", "candle"
model = "all-MiniLM-L6-v2"
dimension = 384
batch_size = 32
device = "auto"  # "cpu", "cuda", "auto"

[embedding.cache]
enabled = true
max_size = "1GB"

[vector_index]
backend = "qdrant"  # or "usearch", "hnsw"
collection_name = "yinx_embeddings"
distance_metric = "cosine"
hnsw_m = 16
hnsw_ef_construction = 200
hnsw_ef_search = 100

[keyword_index]
backend = "tantivy"
writer_heap_size = 50000000
commit_interval = "30s"

[reranker]
enabled = true
model = "cross-encoder/ms-marco-MiniLM-L-6-v2"
batch_size = 16

[retrieval]
rrf_k = 60.0
semantic_weight = 0.5
keyword_weight = 0.5
rerank_top_k = 50
final_limit = 10
```

**LLM Configuration:**
```toml
[llm]
enabled = false
provider = "groq"  # or "openai", "anthropic", "ollama"
api_key_env = "GROQ_API_KEY"  # read from env
model = "llama-3.1-70b"
temperature = 0.1
max_tokens = 2048
timeout = 30

[llm.groq]
endpoint = "https://api.groq.com/openai/v1/chat/completions"
models = ["llama-3.1-70b", "mixtral-8x7b", "llama-3.2-90b"]

[llm.openai]
endpoint = "https://api.openai.com/v1/chat/completions"
models = ["gpt-4", "gpt-3.5-turbo"]

[llm.ollama]
endpoint = "http://localhost:11434/api/chat"
models = ["llama2", "mistral"]

[llm.prompts]
system = """You are a penetration testing assistant.
Use the provided terminal output context to answer questions.
Cite specific commands and outputs when possible."""

context_template = """Question: {question}

Context from terminal session:
{context}"""
```

---

### 2. Plugin Architecture for Extensibility

**Problem:** Hardcoded tool detection and entity extractors require code changes for new tools.

**Solution:** Plugin system that loads extractors dynamically.

#### Plugin Interface

```rust
// Core trait for plugins
pub trait YinxPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn init(&mut self, config: &PluginConfig) -> Result<()>;
}

// Tool detector plugin
pub trait ToolDetectorPlugin: YinxPlugin {
    fn detect(&self, command: &str, output: &str) -> Option<DetectedTool>;
    fn parse_output(&self, tool: &DetectedTool, output: &str) -> Result<ParsedOutput>;
}

// Entity extractor plugin
pub trait EntityExtractorPlugin: YinxPlugin {
    fn entity_types(&self) -> Vec<String>;
    fn extract(&self, text: &str) -> Vec<Entity>;
}

// Filter plugin
pub trait FilterPlugin: YinxPlugin {
    fn filter(&self, lines: Vec<String>, context: &FilterContext) -> Vec<String>;
}

// Output formatter plugin
pub trait FormatterPlugin: YinxPlugin {
    fn format_output(&self, results: &[ScoredChunk]) -> String;
    fn format_report(&self, report: &Report) -> String;
}
```

#### Plugin Discovery

```toml
[plugins]
enabled = true
directory = "~/.yinx/plugins"
auto_load = true

[[plugins.tool_detectors]]
name = "nmap-detector"
path = "~/.yinx/plugins/libnmap_detector.so"
enabled = true

[[plugins.entity_extractors]]
name = "custom-credential-extractor"
path = "~/.yinx/plugins/libcred_extractor.so"
config = { patterns = [...], sensitivity = "high" }
enabled = true

[[plugins.filters]]
name = "ml-based-filter"
path = "~/.yinx/plugins/libml_filter.so"
enabled = false  # optional advanced filter

[[plugins.formatters]]
name = "json-formatter"
path = "~/.yinx/plugins/libjson_formatter.so"
enabled = true
```

#### Built-in vs Plugin

**Built-in (compiled):**
- Core filtering (Tier 1-3)
- Basic entity types (IP, port, URL, CVE)
- Common tools (nmap, gobuster, hydra)
- Standard formatters (CLI, markdown)

**Plugin (loadable):**
- Custom tool parsers
- Domain-specific entities
- ML-based filters
- Alternative output formats
- Integration with external tools

---

### 3. Data-Driven Pattern Matching

**Problem:** Inline regex compilation is inefficient and hard to maintain.

**Solution:** Compile patterns once, store in registry, reference by name.

```rust
pub struct PatternRegistry {
    patterns: HashMap<String, CompiledPattern>,
}

pub struct CompiledPattern {
    name: String,
    regex: Regex,
    weight: f32,
    context_window: usize,
    metadata: HashMap<String, serde_json::Value>,
}

impl PatternRegistry {
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut patterns = HashMap::new();

        for pattern_config in &config.patterns {
            let regex = Regex::new(&pattern_config.pattern)?;
            patterns.insert(
                pattern_config.name.clone(),
                CompiledPattern {
                    name: pattern_config.name.clone(),
                    regex,
                    weight: pattern_config.weight,
                    context_window: pattern_config.context_window,
                    metadata: pattern_config.metadata.clone(),
                },
            );
        }

        Ok(Self { patterns })
    }

    pub fn get(&self, name: &str) -> Option<&CompiledPattern> {
        self.patterns.get(name)
    }

    pub fn match_all(&self, text: &str) -> Vec<Match> {
        self.patterns.values()
            .flat_map(|p| p.regex.find_iter(text).map(|m| Match {
                pattern_name: p.name.clone(),
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
                weight: p.weight,
            }))
            .collect()
    }
}
```

---

### 4. Runtime Configuration Validation

**Problem:** Invalid config values cause runtime errors.

**Solution:** Validate config on load with clear error messages.

```rust
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(config: &Config) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate filtering weights sum to 1.0
        let weights_sum = config.filtering.tier2.entropy_weight
            + config.filtering.tier2.uniqueness_weight
            + config.filtering.tier2.technical_weight
            + config.filtering.tier2.change_weight;

        if (weights_sum - 1.0).abs() > 0.01 {
            errors.push(ValidationError::new(
                "filtering.tier2",
                format!("Weights must sum to 1.0, got {}", weights_sum),
            ));
        }

        // Validate regex patterns compile
        for pattern in &config.entities {
            if let Err(e) = Regex::new(&pattern.pattern) {
                errors.push(ValidationError::new(
                    &format!("entities.{}", pattern.type_name),
                    format!("Invalid regex: {}", e),
                ));
            }
        }

        // Validate model exists
        if config.embedding.provider == "fastembed" {
            if !SUPPORTED_MODELS.contains(&config.embedding.model.as_str()) {
                errors.push(ValidationError::new(
                    "embedding.model",
                    format!("Unsupported model: {}", config.embedding.model),
                ));
            }
        }

        // Validate API keys are set if LLM enabled
        if config.llm.enabled {
            if let Ok(key) = std::env::var(&config.llm.api_key_env) {
                if key.is_empty() {
                    errors.push(ValidationError::new(
                        "llm.api_key_env",
                        format!("Environment variable {} is empty", config.llm.api_key_env),
                    ));
                }
            } else {
                errors.push(ValidationError::new(
                    "llm.api_key_env",
                    format!("Environment variable {} not set", config.llm.api_key_env),
                ));
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

---

### 5. Hot-Reloading Configuration

**Problem:** Config changes require daemon restart.

**Solution:** Watch config file and reload on change (SIGHUP or inotify).

```rust
pub struct ConfigWatcher {
    config_path: PathBuf,
    last_modified: SystemTime,
}

impl ConfigWatcher {
    pub async fn watch(
        config_path: PathBuf,
        reload_tx: mpsc::Sender<Config>,
    ) -> Result<()> {
        let mut watcher = Self {
            config_path: config_path.clone(),
            last_modified: Self::get_modified_time(&config_path)?,
        };

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let current_modified = Self::get_modified_time(&watcher.config_path)?;
            if current_modified > watcher.last_modified {
                tracing::info!("Config file changed, reloading...");

                match Config::load(&watcher.config_path) {
                    Ok(config) => {
                        if let Err(errors) = ConfigValidator::validate(&config) {
                            tracing::error!("Invalid config: {:?}", errors);
                            continue;
                        }

                        if reload_tx.send(config).await.is_ok() {
                            watcher.last_modified = current_modified;
                            tracing::info!("Config reloaded successfully");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load config: {}", e);
                    }
                }
            }
        }
    }
}
```

---

### 6. Profile-Based Configuration

**Problem:** Different use cases need different settings (exam vs normal, fast vs accurate).

**Solution:** Predefined profiles that override base config.

```toml
# Base config
[base]
# ... all settings ...

# Exam mode: offline, fast, minimal
[profiles.exam]
embedding.mode = "offline"
embedding.model = "all-MiniLM-L6-v2"  # small, fast
llm.enabled = false
filtering.tier2.score_threshold_percentile = 0.9  # keep more
reranker.enabled = false  # skip for speed
capture.buffer_size = 5000
capture.flush_interval = "10s"

# Accuracy mode: online, slow, thorough
[profiles.accuracy]
embedding.mode = "online"
embedding.model = "text-embedding-3-large"
llm.enabled = true
llm.model = "gpt-4"
filtering.tier2.score_threshold_percentile = 0.7  # more aggressive
reranker.enabled = true
reranker.model = "cross-encoder/ms-marco-TinyBERT-L-6"

# Fast mode: minimal processing
[profiles.fast]
filtering.tier1.max_occurrences = 1
filtering.tier2.score_threshold_percentile = 0.95
embedding.batch_size = 64
reranker.enabled = false
```

Usage:
```bash
yinx start --profile exam
yinx start --profile accuracy
yinx config set-profile fast
```

---

### 7. Schema Versioning

**Problem:** Config format changes break existing configs.

**Solution:** Version config schema and provide migrations.

```toml
[_meta]
schema_version = "2.0.0"
created_at = "2025-10-01T00:00:00Z"
last_modified = "2025-10-01T12:00:00Z"

# Rest of config...
```

```rust
pub struct ConfigMigrator;

impl ConfigMigrator {
    pub fn migrate(config: &mut Config) -> Result<()> {
        let current = semver::Version::parse(&config.meta.schema_version)?;
        let target = semver::Version::parse(CURRENT_SCHEMA_VERSION)?;

        if current < target {
            tracing::info!("Migrating config from {} to {}", current, target);

            // Apply migrations in order
            if current < semver::Version::parse("1.1.0")? {
                Self::migrate_1_0_to_1_1(config)?;
            }
            if current < semver::Version::parse("2.0.0")? {
                Self::migrate_1_1_to_2_0(config)?;
            }

            config.meta.schema_version = CURRENT_SCHEMA_VERSION.to_string();
        }

        Ok(())
    }

    fn migrate_1_0_to_1_1(config: &mut Config) -> Result<()> {
        // Add new fields with defaults
        config.retrieval.rerank_top_k = 50;
        Ok(())
    }
}
```

---

### 8. Environment Variable Overrides

**Problem:** Container/cloud deployments need runtime config.

**Solution:** Allow env vars to override config values.

```bash
# Override any config value via env var
YINX_LLM__ENABLED=true
YINX_LLM__API_KEY=xxx
YINX_EMBEDDING__MODE=online
YINX_FILTERING__TIER2__SCORE_THRESHOLD_PERCENTILE=0.75
```

```rust
impl Config {
    pub fn load_with_overrides(path: &Path) -> Result<Self> {
        let mut config = Self::load(path)?;

        // Apply env var overrides
        for (key, value) in std::env::vars() {
            if let Some(config_key) = key.strip_prefix("YINX_") {
                let path = config_key.replace("__", ".");
                config.set_value(&path, &value)?;
            }
        }

        Ok(config)
    }
}
```

---

### 9. Configuration Documentation

Every config value should have inline documentation:

```toml
# Filtering pipeline configuration
[filtering.tier2]

# Weight for entropy score (0.0-1.0)
# Higher values prioritize information-dense lines
# Default: 0.3
entropy_weight = 0.3

# Weight for uniqueness score (0.0-1.0)
# Higher values prioritize rare/unique lines
# Default: 0.3
uniqueness_weight = 0.3

# Percentile threshold for keeping lines (0.0-1.0)
# 0.8 = keep top 20%, 0.9 = keep top 10%
# Default: 0.8
score_threshold_percentile = 0.8
```

---

## Summary of Architectural Improvements

| Area | Before (Hardcoded) | After (Configurable) |
|------|-------------------|---------------------|
| **Filter weights** | `0.3, 0.3, 0.2, 0.2` in code | `config.toml` with validation |
| **Entity patterns** | Inline `Regex::new()` | Pattern registry from config |
| **Tool detection** | if/else chains | Plugin system + config |
| **Model names** | `"all-MiniLM-L6-v2"` | Config with validation |
| **API endpoints** | Hardcoded URLs | Provider-specific config |
| **Scoring logic** | Fixed formula | Weights from config |
| **Normalization** | `__IP__`, `__PORT__` | Configurable replacements |
| **Batch sizes** | `32`, `50_000_000` | Config with profiles |
| **Thresholds** | `0.8`, `60.0` | Tunable via config |
| **LLM prompts** | String literals | Template files |

---

## Benefits

1. **No recompilation needed** - Tune performance without rebuilding
2. **Environment-specific configs** - Different settings for dev/prod/exam
3. **User extensibility** - Add tools/entities without code changes
4. **A/B testing** - Compare filter strategies easily
5. **Runtime adaptation** - Hot-reload config on the fly
6. **Clear documentation** - Config file is self-documenting
7. **Validation** - Catch errors before runtime
8. **Versioning** - Migrate old configs automatically

---

**This architecture ensures yinx remains flexible, maintainable, and extensible without sacrificing performance.**
