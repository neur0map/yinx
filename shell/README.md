# Yinx Shell Integration

Yinx provides shell hooks to automatically capture terminal command execution during penetration testing sessions.

## Overview

There are two capture modes available:

1. **Basic Mode** (recommended): Captures command metadata (command text, exit code, working directory)
2. **Script Mode** (experimental): Attempts full output capture using temp files

## Installation

### Bash

**Basic Mode:**
```bash
# Add to ~/.bashrc
source /path/to/yinx/shell/bash.sh
```

**Script Mode (experimental):**
```bash
# Add to ~/.bashrc
source /path/to/yinx/shell/bash-script-wrapper.sh
```

### Zsh

**Basic Mode:**
```bash
# Add to ~/.zshrc
source /path/to/yinx/shell/zsh.sh
```

## Usage

1. **Start the yinx daemon:**
   ```bash
   yinx start --session "my-pentest"
   ```

2. **Run your commands normally:**
   ```bash
   nmap -sV 192.168.1.1
   gobuster dir -u http://target.com -w /usr/share/wordlists/dirb/common.txt
   hydra -l admin -P passwords.txt ssh://192.168.1.1
   ```

3. **Commands are automatically captured in the background** - no wrapping needed!

4. **Stop the daemon when done:**
   ```bash
   yinx stop
   ```

## How It Works

### Basic Mode

- Uses `PROMPT_COMMAND` (bash) or `precmd` (zsh) hooks
- Captures after command execution
- Sends data asynchronously via `yinx _internal capture`
- Zero-latency, minimal overhead

### Script Mode (Experimental)

- Uses `DEBUG` trap to intercept command execution
- Attempts to capture stdout/stderr via temp files
- Higher overhead, may cause slight latency
- Still under development

## Output Capture Limitations

⚠️ **Current Limitation**: Shell hooks can only capture command metadata, not actual stdout/stderr output retroactively.

**Why?** Once a command executes in bash/zsh, its output is already printed to the terminal. We can't capture it after the fact without:
- Wrapping command execution (complex, fragile)
- Using PTY capture with `script` command (experimental)
- Using a terminal multiplexer integration (future)

### Workarounds

1. **Manual Redirection** (for specific commands):
   ```bash
   nmap -sV target.com | tee >(yinx _internal capture ...)
   ```

2. **Script Command** (captures everything):
   ```bash
   script -q -c "bash" /tmp/yinx-session.log
   # Your pentest session here
   # Then: yinx import /tmp/yinx-session.log
   ```

3. **Wait for Phase 4+** (intelligent filtering will work with full logs)

## Environment Variables

- `YINX_SESSION_ID`: Override session ID (default: "default")
- `YINX_BIN`: Path to yinx binary (default: "yinx")
- `YINX_SOCKET`: Unix socket path (default: "~/.yinx/daemon.sock")

## Troubleshooting

### Hook not working?

1. Check if yinx daemon is running:
   ```bash
   yinx status
   ```

2. Verify socket exists:
   ```bash
   ls -l ~/.yinx/daemon.sock
   ```

3. Check if hook is loaded:
   ```bash
   # For bash
   echo $PROMPT_COMMAND | grep yinx

   # For zsh
   echo $precmd_functions | grep yinx
   ```

### Permission denied on socket?

```bash
chmod 600 ~/.yinx/daemon.sock
```

### Commands not being captured?

- Make sure you sourced the hook file in the current shell
- Restart your shell after adding to ~/.bashrc or ~/.zshrc
- Check logs: `tail -f ~/.yinx/logs/daemon.log`

## Future Enhancements

- **Phase 3.1**: Full PTY capture using `script` wrapper
- **Phase 3.2**: Terminal multiplexer integration (tmux, screen)
- **Phase 3.3**: Real-time streaming capture
- **Phase 3.4**: Intelligent output filtering at capture time

## Security Notes

- All captured data stays local in `~/.yinx/`
- No data sent over network (unless you configure cloud LLM for Phase 8)
- Socket permissions restrict access to your user only
- Sensitive data (credentials) will be redacted in Phase 5

## Performance

- **Basic Mode**: <1ms overhead per command
- **Script Mode**: ~5-10ms overhead per command
- Async sending doesn't block your terminal
- Background processing in daemon

---

For more information, see the main project documentation.
