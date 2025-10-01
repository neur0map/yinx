# Yinx - Anti-Patterns to Avoid

**Author:** neur0map
**Purpose:** Ensure maintainable, extensible, configuration-driven implementation

---

## ❌ Hardcoding Anti-Patterns

### 1. Magic Numbers

**BAD:**
```rust
fn score_line(&self, line: &str) -> f32 {
    let entropy = self.calculate_entropy(line) * 0.3;
    let uniqueness = self.calculate_uniqueness(line) * 0.3;
    let technical = self.calculate_technical(line) * 0.2;
    let change = self.calculate_change(line) * 0.2;
    entropy + uniqueness + technical + change
}

const BATCH_SIZE: usize = 32;
const RRF_K: f32 = 60.0;
const THRESHOLD: f32 = 0.8;
```

**GOOD:**
```rust
fn score_line(&self, line: &str) -> f32 {
    let entropy = self.calculate_entropy(line) * self.config.entropy_weight;
    let uniqueness = self.calculate_uniqueness(line) * self.config.uniqueness_weight;
    let technical = self.calculate_technical(line) * self.config.technical_weight;
    let change = self.calculate_change(line) * self.config.change_weight;
    entropy + uniqueness + technical + change
}

let batch_size = config.embedding.batch_size;
let rrf_k = config.retrieval.rrf_k;
let threshold = config.filtering.tier2.score_threshold_percentile;
```

---

### 2. Inline Regex Patterns

**BAD:**
```rust
fn calculate_technical_density(&self, line: &str) -> f32 {
    let patterns = [
        r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b",
        r"\b\d{1,5}/tcp\b",
        r"https?://",
        r"CVE-\d{4}-\d{4,}",
    ];

    let matches: usize = patterns.iter()
        .map(|p| Regex::new(p).unwrap().find_iter(line).count())  // Compiles every time!
        .sum();

    matches as f32 / 10.0
}
```

**GOOD:**
```rust
// One-time compilation from config
struct PatternRegistry {
    patterns: Vec<CompiledPattern>,
}

impl PatternRegistry {
    fn from_config(config: &Config) -> Result<Self> {
        let patterns = config.technical_patterns.iter()
            .map(|p| CompiledPattern {
                name: p.name.clone(),
                regex: Regex::new(&p.pattern)?,
                weight: p.weight,
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { patterns })
    }
}

fn calculate_technical_density(&self, line: &str) -> f32 {
    let weighted_sum: f32 = self.patterns.iter()
        .map(|p| p.regex.find_iter(line).count() as f32 * p.weight)
        .sum();

    (weighted_sum / self.config.max_technical_score).min(1.0)
}
```

---

### 3. Enum-Based Entity Types

**BAD:**
```rust
enum EntityType {
    IpAddress,
    Port,
    Url,
    Cve,
    Credential,
    FilePath,
    // Adding new type = recompile!
}

fn extract_entities(text: &str) -> Vec<Entity> {
    vec![
        extract_ips(text),
        extract_ports(text),
        extract_urls(text),
        // ...
    ].into_iter().flatten().collect()
}
```

**GOOD:**
```rust
struct Entity {
    type_name: String,  // Dynamic
    value: String,
    confidence: f32,
}

struct EntityRegistry {
    extractors: Vec<EntityExtractor>,
}

impl EntityRegistry {
    fn from_config(configs: Vec<EntityConfig>) -> Result<Self> {
        let extractors = configs.iter()
            .map(|c| EntityExtractor::from_config(c))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { extractors })
    }

    fn extract_all(&self, text: &str) -> Vec<Entity> {
        self.extractors.iter()
            .flat_map(|e| e.extract(text))
            .collect()
    }
}

// Add new entity types in config.toml, no recompile!
```

---

### 4. If/Else Chains for Tool Detection

**BAD:**
```rust
fn detect_tool(command: &str) -> ToolType {
    if command.starts_with("nmap") {
        ToolType::Nmap
    } else if command.contains("gobuster") {
        ToolType::Gobuster
    } else if command.contains("hydra") {
        ToolType::Hydra
    } else if command.starts_with("sqlmap") {
        ToolType::Sqlmap
    }
    // Adding new tool = code change!
    else {
        ToolType::Unknown
    }
}
```

**GOOD:**
```rust
struct ToolDetector {
    matchers: Vec<ToolMatcher>,
}

impl ToolDetector {
    fn from_config(configs: Vec<ToolConfig>) -> Result<Self> {
        let matchers = configs.iter()
            .map(|c| ToolMatcher {
                name: c.name.clone(),
                patterns: c.patterns.iter()
                    .map(|p| Regex::new(p))
                    .collect::<Result<Vec<_>, _>>()?,
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { matchers })
    }

    fn detect(&self, command: &str) -> Option<&str> {
        self.matchers.iter()
            .find(|m| m.matches(command))
            .map(|m| m.name.as_str())
    }
}

// Add new tools in tools.toml, no recompile!
```

---

### 5. Hardcoded API Endpoints

**BAD:**
```rust
async fn call_llm(question: &str) -> Result<String> {
    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .json(&json!({
            "model": "llama-3.1-70b",
            "messages": [...],
            "temperature": 0.1,
        }))
        .send()
        .await?;

    // ...
}
```

**GOOD:**
```rust
trait LlmProvider {
    async fn ask(&self, question: &str, context: &str) -> Result<String>;
}

struct GroqProvider {
    config: LlmConfig,
    client: Client,
}

impl LlmProvider for GroqProvider {
    async fn ask(&self, question: &str, context: &str) -> Result<String> {
        let response = self.client
            .post(&self.config.endpoint)
            .json(&json!({
                "model": self.config.model,
                "messages": [...],
                "temperature": self.config.temperature,
            }))
            .send()
            .await?;

        // ...
    }
}

// Support multiple providers via config
fn create_provider(config: &LlmConfig) -> Box<dyn LlmProvider> {
    match config.provider.as_str() {
        "groq" => Box::new(GroqProvider::new(config)),
        "openai" => Box::new(OpenAIProvider::new(config)),
        "ollama" => Box::new(OllamaProvider::new(config)),
        _ => panic!("Unknown provider"),
    }
}
```

---

### 6. Hardcoded Prompts

**BAD:**
```rust
let system_prompt = "You are a penetration testing assistant. \
    Use the provided terminal output context to answer questions. \
    Cite specific commands and outputs when possible.";

let user_prompt = format!(
    "Question: {}\n\nContext from terminal session:\n{}",
    question, context
);
```

**GOOD:**
```rust
struct PromptTemplate {
    system: String,
    user: String,
}

impl PromptTemplate {
    fn from_config(config: &LlmConfig) -> Result<Self> {
        // Load from config or external files
        Ok(Self {
            system: config.prompts.system.clone(),
            user: config.prompts.user_template.clone(),
        })
    }

    fn render_user(&self, question: &str, context: &str) -> String {
        self.user
            .replace("{question}", question)
            .replace("{context}", context)
    }
}

// Or use a template engine
use tera::Tera;

let mut tera = Tera::new("prompts/**/*.txt")?;
let rendered = tera.render("user_query.txt", &context! {
    question: question,
    context: context,
})?;
```

---

### 7. Hardcoded Model Names

**BAD:**
```rust
let model = TextEmbedding::try_new(InitOptions {
    model_name: "all-MiniLM-L6-v2",  // Hardcoded
    ..Default::default()
})?;

let dimension = 384;  // Must match model
```

**GOOD:**
```rust
let model = TextEmbedding::try_new(InitOptions {
    model_name: config.embedding.model.as_str(),
    ..Default::default()
})?;

let dimension = config.embedding.dimension;

// Validate model exists at startup
if !SUPPORTED_MODELS.contains(&config.embedding.model.as_str()) {
    return Err(anyhow!("Unsupported model: {}", config.embedding.model));
}
```

---

### 8. Hardcoded Normalization Rules

**BAD:**
```rust
fn normalize_pattern(line: &str) -> String {
    line.replace_regex(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}", "__IP__")
        .replace_regex(r"\d{1,5}/(tcp|udp)", "__PORT__")
        .replace_regex(r"https?://[^\s]+", "__URL__")
        .replace_regex(r"\d+", "__NUM__")
}
```

**GOOD:**
```rust
struct NormalizationEngine {
    rules: Vec<NormalizationRule>,
}

struct NormalizationRule {
    name: String,
    pattern: Regex,
    replacement: String,
    priority: u8,  // Apply in order
}

impl NormalizationEngine {
    fn from_config(config: &FilterConfig) -> Result<Self> {
        let mut rules = config.normalization_patterns.iter()
            .map(|p| Ok(NormalizationRule {
                name: p.name.clone(),
                pattern: Regex::new(&p.pattern)?,
                replacement: p.replacement.clone(),
                priority: p.priority.unwrap_or(0),
            }))
            .collect::<Result<Vec<_>, _>>()?;

        rules.sort_by_key(|r| r.priority);
        Ok(Self { rules })
    }

    fn normalize(&self, text: &str) -> String {
        let mut result = text.to_string();
        for rule in &self.rules {
            result = rule.pattern.replace_all(&result, &rule.replacement).to_string();
        }
        result
    }
}
```

---

## ✅ Best Practices

### 1. Centralized Configuration

```rust
// Load once at startup
pub struct YinxContext {
    config: Arc<Config>,
    pattern_registry: Arc<PatternRegistry>,
    entity_registry: Arc<EntityRegistry>,
    tool_detector: Arc<ToolDetector>,
}

impl YinxContext {
    pub fn new(config_path: &Path) -> Result<Self> {
        let config = Config::load(config_path)?;

        // Validate before proceeding
        ConfigValidator::validate(&config)?;

        // Pre-compile all patterns
        let pattern_registry = Arc::new(PatternRegistry::from_config(&config)?);
        let entity_registry = Arc::new(EntityRegistry::from_config(&config.entities)?);
        let tool_detector = Arc::new(ToolDetector::from_config(&config.tools)?);

        Ok(Self {
            config: Arc::new(config),
            pattern_registry,
            entity_registry,
            tool_detector,
        })
    }
}
```

### 2. Hot-Reloadable Configuration

```rust
pub async fn watch_config(
    config_path: PathBuf,
    reload_tx: mpsc::Sender<YinxContext>,
) -> Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        if config_changed(&config_path)? {
            match YinxContext::new(&config_path) {
                Ok(new_context) => {
                    tracing::info!("Config reloaded successfully");
                    reload_tx.send(new_context).await?;
                }
                Err(e) => {
                    tracing::error!("Failed to reload config: {}", e);
                    // Keep using old config
                }
            }
        }
    }
}
```

### 3. Profile-Based Overrides

```rust
impl Config {
    pub fn load_with_profile(path: &Path, profile: &str) -> Result<Self> {
        let mut config = Self::load(path)?;

        if let Some(profile_config) = config.profiles.get(profile) {
            config.merge_profile(profile_config)?;
        }

        Ok(config)
    }
}

// Usage
let config = Config::load_with_profile(path, "exam")?;
```

### 4. Environment Variable Overrides

```rust
impl Config {
    pub fn apply_env_overrides(&mut self) {
        for (key, value) in std::env::vars() {
            if let Some(config_key) = key.strip_prefix("YINX_") {
                let path = config_key.replace("__", ".");
                if let Err(e) = self.set_value(&path, &value) {
                    tracing::warn!("Failed to apply env override {}: {}", key, e);
                }
            }
        }
    }
}
```

### 5. Config Documentation

```toml
# Example: filters.toml

[tier2]

# Entropy weight (0.0-1.0)
# Higher = prioritize information-dense lines
# Default: 0.3
# Range: [0.0, 1.0]
# Note: All weights must sum to 1.0
entropy_weight = 0.3

# Uniqueness weight (0.0-1.0)
# Higher = prioritize rare lines
# Default: 0.3
uniqueness_weight = 0.3

# Technical content weight (0.0-1.0)
# Higher = prioritize lines with IPs, CVEs, etc.
# Default: 0.2
technical_weight = 0.2

# Change detection weight (0.0-1.0)
# Higher = prioritize lines different from previous
# Default: 0.2
change_weight = 0.2
```

---

## Summary

| Area | Anti-Pattern | Best Practice |
|------|-------------|---------------|
| **Scoring weights** | `0.3, 0.2` in code | Load from config |
| **Regex patterns** | Inline `Regex::new()` | Pre-compiled registry |
| **Entity types** | Enum | Dynamic string types |
| **Tool detection** | if/else chains | Config-based matchers |
| **API endpoints** | Hardcoded URLs | Provider trait + config |
| **Prompts** | String literals | Template files |
| **Model names** | `"all-MiniLM-L6-v2"` | Config with validation |
| **Normalization** | Fixed replacements | Configurable rules |
| **Batch sizes** | Constants | Config values |
| **Thresholds** | Magic numbers | Named config values |

---

## Pre-Implementation Checklist

Before writing code, ask:

- [ ] Can this value change between deployments? → Config
- [ ] Will users want to customize this? → Config
- [ ] Does this vary by environment (dev/exam/prod)? → Profile
- [ ] Is this a regex or pattern? → Compile once from config
- [ ] Is this tool-specific? → Plugin/config, not hardcode
- [ ] Is this an entity type? → Dynamic registry
- [ ] Will this need A/B testing? → Config
- [ ] Could this be user-contributed? → Plugin architecture

**If you answer YES to any, avoid hardcoding.**

---

**Remember: Configuration-driven = Maintainable, Extensible, User-Friendly**
