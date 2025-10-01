# Yinx Quick Start

## Installation

```bash
# Build from source
cargo build --release

# Binary location
./target/release/yinx
```

## First Run - Preset Model Setup

Yinx comes with a **preset embedding model** pre-configured:

```bash
# Start yinx for the first time
yinx start

# Output you'll see:
[INFO] Initializing embedding model: all-MiniLM-L6-v2 (384D, ~90MB download if not cached)
[INFO] Downloading model to ~/.cache/huggingface/...
[####################################] 90MB/90MB
[INFO] Model loaded successfully
[INFO] Daemon started
```

**This happens only once** - subsequent runs use the cached model instantly.

## What Just Happened?

1. **Binary**: 25MB yinx binary runs
2. **Preset model**: `all-MiniLM-L6-v2` downloads automatically (~90MB)
3. **Cache location**: Model saved to `~/.cache/huggingface/hub/`
4. **Ready to use**: Semantic search enabled

## Using Yinx

```bash
# Start daemon (uses cached model - instant)
yinx start

# Your terminal is now being captured
# Work normally with your pentest tools

# Stop daemon
yinx stop

# Query findings
yinx query "CVE vulnerabilities found"

# Ask AI to analyze (optional, requires GROQ_API_KEY)
yinx ask "What are the critical findings?"
```

## Configuration (Optional)

The preset model works great for most users. If you want to customize:

```bash
# View current config
cat ~/.config/yinx/config.toml
```

```toml
[embedding]
model = "all-MiniLM-L6-v2"  # Preset - already configured
mode = "offline"             # Local models only
batch_size = 32              # Process 32 texts at once

[indexing]
vector_dim = 384             # Matches preset model
hnsw_ef_construction = 200   # Search quality
hnsw_m = 16                  # Graph connectivity
```

## Changing Models (Advanced)

Want better accuracy? Upgrade to a larger model:

```bash
# Edit config
nano ~/.config/yinx/config.toml

# Change this line:
model = "bge-small-en-v1.5"  # or "bge-base-en-v1.5"

# Also update vector_dim if using bge-base (768 dims)
vector_dim = 768  # Only for bge-base-en-v1.5
```

Next start will download the new model.

## Exam Setup

To prepare for an exam (offline environment):

```bash
# 1. Install yinx
cargo build --release

# 2. Pre-download preset model
yinx start  # Downloads 90MB preset
yinx stop

# 3. Verify offline works
# Disconnect network
yinx start  # Should start instantly with cached model
yinx stop

# Total footprint: 25MB binary + 90MB model = 115MB
```

## Storage Locations

```
~/.yinx/                    # Data directory
  ├── store/                # Machine-readable data
  │   ├── db.sqlite        # Sessions, captures, entities
  │   ├── blobs/           # Content-addressed storage
  │   ├── vectors/         # HNSW indexes
  │   └── keywords/        # Tantivy indexes
  └── reports/             # Human-readable reports

~/.cache/huggingface/      # Model cache
  └── hub/                 # Downloaded models
      └── all-MiniLM-L6-v2/  # Preset model (90MB)

~/.config/yinx/            # Configuration
  ├── config.toml          # Main config (preset defaults)
  ├── entities.toml        # Entity patterns
  ├── tools.toml           # Tool detection
  └── filters.toml         # Filtering rules
```

## Next Steps

- **Basic usage**: See main [README.md](../README.md)
- **Model options**: See [MODELS.md](./MODELS.md)
- **Architecture**: See [phase6_summary.md](./phase6_summary.md)

## Common Questions

**Q: Do I need to configure the model?**
A: No! The preset (`all-MiniLM-L6-v2`) is already configured.

**Q: Is the model bundled in the binary?**
A: No. Binary is 25MB, model downloads separately (90MB) on first use.

**Q: Can I use yinx offline?**
A: Yes! After first download, works completely offline.

**Q: Can I change the model?**
A: Yes, optionally. See "Changing Models" above.

**Q: How big is the total installation?**
A: 115MB (25MB binary + 90MB preset model)

---

**TL;DR**: Install yinx → Run `yinx start` → Preset model downloads automatically → Done!
