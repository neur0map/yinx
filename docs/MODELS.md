# Yinx Embedding Models

## Overview

Yinx uses **local embedding models** for semantic search - **no API calls, fully offline**.

### Preset Model

**Yinx comes with a preset model**: `all-MiniLM-L6-v2`
- Pre-configured in all installations
- Automatically downloads on first use (~90MB)
- Recommended for most users and exam scenarios
- **Users can optionally change** to more powerful models (see below)

### Binary Size

- **Yinx binary**: ~25MB (ONNX runtime, no models bundled)
- **Preset model**: ~90MB (downloads to `~/.cache/huggingface/` on first use)
- **Total**: 115MB after first run

## Available Models

Yinx supports three models. The preset is recommended, but you can switch:

### ✅ all-MiniLM-L6-v2 (PRESET - Recommended)
- **Size**: ~90MB
- **Dimensions**: 384
- **Speed**: Fast (100 texts/sec)
- **Accuracy**: Good
- **Use case**: General purpose, exam-friendly, **default preset**
- **Status**: **Pre-configured in yinx** - no config change needed
- **Change to this**: Not needed (already default)

### 🔧 bge-small-en-v1.5 (Optional Upgrade)
- **Size**: ~130MB
- **Dimensions**: 384
- **Speed**: Fast (80 texts/sec)
- **Accuracy**: Better than preset
- **Use case**: When you need higher quality embeddings
- **Status**: Optional - requires config change
- **Change to this**: Set `embedding.model = "bge-small-en-v1.5"` in config

### 🔧 bge-base-en-v1.5 (Optional - Maximum Accuracy)
- **Size**: ~440MB
- **Dimensions**: 768
- **Speed**: Slower (40 texts/sec)
- **Accuracy**: Best available
- **Use case**: Maximum accuracy, research, not recommended for exams
- **Status**: Optional - requires config change
- **Change to this**: Set `embedding.model = "bge-base-en-v1.5"` in config

## Model Download Behavior

### First Use (Per Model)
```bash
# First time you use embedding features
$ yinx start
[INFO] Initializing embedding model: all-MiniLM-L6-v2 (384D, ~90MB download if not cached)
[INFO] Downloading model to ~/.cache/huggingface/...
# Progress bar shows download
[INFO] Model loaded successfully
```

### Subsequent Uses
```bash
# Model already cached
$ yinx start
[INFO] Initializing embedding model: all-MiniLM-L6-v2 (384D, ~90MB download if not cached)
[INFO] Model loaded successfully (cached)
```

## Pre-downloading Models

To pre-download models before an exam:

```bash
# Option 1: Start daemon once (downloads default model)
yinx start
yinx stop

# Option 2: Use Python to download specific models
pip install fastembed
python -c "from fastembed import TextEmbedding; TextEmbedding('all-MiniLM-L6-v2')"

# Option 3: Manual download (advanced)
# Models are stored in ~/.cache/huggingface/hub/
```

## Exam Compliance

### Recommended Setup
1. **Before exam**: Pre-download the model
2. **Model choice**: Use `all-MiniLM-L6-v2` (90MB, fast)
3. **Binary size**: 25MB binary + 90MB model = 115MB total
4. **Offline**: Works completely offline once downloaded

### Switching Models (Optional)

**The preset model (all-MiniLM-L6-v2) is already configured** - no action needed.

If you want to upgrade to a more powerful model:

**Option 1: Edit config file** (`~/.config/yinx/config.toml`):
```toml
[embedding]
model = "bge-small-en-v1.5"  # Change from preset to this
mode = "offline"
batch_size = 32
```

**Option 2: Use environment variable** (temporary):
```bash
export YINX_EMBEDDING__MODEL="bge-small-en-v1.5"
yinx start
```

**Note**: Changing models will trigger a new download on next use.

## Cache Location

Models are cached in:
- **Linux/macOS**: `~/.cache/huggingface/hub/`
- **Windows**: `%USERPROFILE%\.cache\huggingface\hub\`

To clear cache:
```bash
rm -rf ~/.cache/huggingface/hub/
```

## Disk Space Requirements

| Configuration | Total Size |
|---------------|------------|
| Binary only | 25MB |
| Binary + default model | 115MB |
| Binary + all models | 685MB |
| With indexes (1GB data) | +500MB |

## Performance Characteristics

| Model | Embedding Speed | Search Speed | Memory |
|-------|----------------|--------------|--------|
| all-MiniLM-L6-v2 | 100 texts/sec | <2ms | ~500MB |
| bge-small-en-v1.5 | 80 texts/sec | <2ms | ~600MB |
| bge-base-en-v1.5 | 40 texts/sec | <3ms | ~1.2GB |

*Benchmarks on MacBook Pro M1, release build*

## Troubleshooting

### Model Download Fails
```bash
# Check internet connection
# Verify HuggingFace is accessible
curl -I https://huggingface.co

# Check disk space
df -h ~/.cache

# Clear cache and retry
rm -rf ~/.cache/huggingface/hub/
```

### Model Takes Too Long
```bash
# Use smaller model
export YINX_EMBEDDING__MODEL="all-MiniLM-L6-v2"

# Or disable embeddings temporarily
export YINX_EMBEDDING__MODE="disabled"
```

## Future Support

Planned model backends:
- [ ] Candle (Rust-native, no ONNX dependency)
- [ ] Quantized models (smaller, faster)
- [ ] Custom models via ONNX
- [ ] API-based models (OpenAI, Cohere) for online mode

---

## Key Takeaways

✅ **Yinx has a preset model**: `all-MiniLM-L6-v2` (pre-configured, recommended)
✅ **Binary size**: 25MB (lightweight, no models bundled)
✅ **Preset model download**: ~90MB on first use to `~/.cache/huggingface/`
✅ **Total footprint**: 115MB (binary + preset model)
✅ **Exam-friendly**: Pre-download once, use offline indefinitely
✅ **Optional upgrades**: Users can switch to more powerful models if needed

**For most users**: Just install and run - the preset model works great out of the box!
