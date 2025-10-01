#!/bin/bash
# Yinx bash shell hook for terminal capture
# Source this file in your ~/.bashrc: source /path/to/yinx/shell/bash.sh

# Socket path (should match daemon config)
YINX_SOCKET="${HOME}/.yinx/daemon.sock"

# Directory for temporary output files
YINX_TMP_DIR="${HOME}/.yinx/tmp"
mkdir -p "$YINX_TMP_DIR" 2>/dev/null

# Path to yinx binary (customize if needed)
YINX_BIN="${YINX_BIN:-yinx}"

# Post-execution hook: Capture and send command output
__yinx_capture() {
    local exit_code=$?

    # Get last command from history
    local last_cmd=$(HISTTIMEFORMAT= history 1 | sed 's/^[ ]*[0-9]*[ ]*//')

    # Skip if yinx command itself
    if [[ "$last_cmd" =~ ^yinx ]]; then
        return $exit_code
    fi

    # Skip empty commands
    if [[ -z "$last_cmd" ]]; then
        return $exit_code
    fi

    # Check if daemon socket exists
    if [[ ! -S "$YINX_SOCKET" ]]; then
        return $exit_code
    fi

    local session_id="${YINX_SESSION_ID:-default}"
    local timestamp=$(date +%s)
    local cwd="$(pwd)"

    # Create temp file for output
    local output_file="${YINX_TMP_DIR}/yinx_$$_${RANDOM}.out"

    # We can't capture output retroactively, so we send empty output
    # Note: For full output capture, use script command wrapper
    # or source yinx-script-wrapper.sh instead

    # Send capture via yinx _internal (async, in background)
    (
        # Create empty output file
        touch "$output_file"

        # Send via yinx _internal subcommand
        if command -v "$YINX_BIN" &> /dev/null; then
            "$YINX_BIN" _internal capture \
                --session-id "$session_id" \
                --timestamp "$timestamp" \
                --command "$last_cmd" \
                --output-file "$output_file" \
                --exit-code "$exit_code" \
                --cwd "$cwd" 2>/dev/null

            # Cleanup temp file after send
            rm -f "$output_file" 2>/dev/null
        fi
    ) &

    return $exit_code
}

# Set up PROMPT_COMMAND
if [[ -z "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND="__yinx_capture"
else
    # Prepend to existing PROMPT_COMMAND
    PROMPT_COMMAND="__yinx_capture; $PROMPT_COMMAND"
fi

echo "Yinx bash hook loaded (basic). Session: ${YINX_SESSION_ID:-default}"
echo "⚠ Note: This hook captures commands but NOT output."
echo "For full output capture, use: source ~/.yinx/shell/bash-script-wrapper.sh"
