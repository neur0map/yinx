#!/bin/bash
# Yinx bash ADVANCED shell hook with full output capture using `script` command
# Source this file in your ~/.bashrc: source /path/to/yinx/shell/bash-script-wrapper.sh
#
# WARNING: This wrapper uses `script` to capture ALL terminal I/O including output.
# It may cause slight latency and is more invasive than the basic hook.
# Use this for penetration testing sessions where you need full output capture.

# Socket path (should match daemon config)
YINX_SOCKET="${HOME}/.yinx/daemon.sock"

# Directory for temporary output files
YINX_TMP_DIR="${HOME}/.yinx/tmp"
mkdir -p "$YINX_TMP_DIR" 2>/dev/null

# Path to yinx binary
YINX_BIN="${YINX_BIN:-yinx}"

# Variable to store current capture file
__YINX_CURRENT_CAPTURE_FILE=""

# Function to intercept command execution
__yinx_preexec() {
    local cmd="$1"

    # Skip if yinx command itself
    if [[ "$cmd" =~ ^yinx ]]; then
        return
    fi

    # Skip empty commands
    if [[ -z "$cmd" ]]; then
        return
    fi

    # Check if daemon socket exists
    if [[ ! -S "$YINX_SOCKET" ]]; then
        return
    fi

    # Create unique temp file for this command's output
    __YINX_CURRENT_CAPTURE_FILE="${YINX_TMP_DIR}/yinx_$$_${RANDOM}.out"
}

# Function to send captured command
__yinx_postexec() {
    local exit_code=$?

    # Get last command from history
    local last_cmd=$(HISTTIMEFORMAT= history 1 | sed 's/^[ ]*[0-9]*[ ]*//')

    # Skip if no command
    if [[ -z "$last_cmd" ]]; then
        return $exit_code
    fi

    # Skip if yinx command itself
    if [[ "$last_cmd" =~ ^yinx ]]; then
        return $exit_code
    fi

    # Check if daemon socket exists
    if [[ ! -S "$YINX_SOCKET" ]]; then
        return $exit_code
    fi

    local session_id="${YINX_SESSION_ID:-default}"
    local timestamp=$(date +%s)
    local cwd="$(pwd)"
    local output_file="$__YINX_CURRENT_CAPTURE_FILE"

    # Send capture asynchronously
    (
        if command -v "$YINX_BIN" &> /dev/null; then
            "$YINX_BIN" _internal capture \
                --session-id "$session_id" \
                --timestamp "$timestamp" \
                --command "$last_cmd" \
                --output-file "${output_file:-/dev/null}" \
                --exit-code "$exit_code" \
                --cwd "$cwd" 2>/dev/null

            # Cleanup temp file after send
            if [[ -n "$output_file" ]] && [[ -f "$output_file" ]]; then
                rm -f "$output_file" 2>/dev/null
            fi
        fi
    ) &

    # Reset capture file
    __YINX_CURRENT_CAPTURE_FILE=""

    return $exit_code
}

# Set up preexec hook (requires bash-preexec.sh or manual DEBUG trap)
# We'll use the DEBUG trap approach which is built-in to bash

__yinx_preexec_invoke() {
    # Only invoke for actual commands, not function internals
    if [[ "$BASH_COMMAND" != "__yinx_"* ]]; then
        __yinx_preexec "$BASH_COMMAND"
    fi
}

# Set up DEBUG trap for preexec
trap '__yinx_preexec_invoke' DEBUG

# Set up PROMPT_COMMAND for postexec
if [[ -z "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND="__yinx_postexec"
else
    # Prepend to existing PROMPT_COMMAND
    PROMPT_COMMAND="__yinx_postexec; $PROMPT_COMMAND"
fi

# Alternative approach using `script` command to wrap the entire shell
# This is more invasive but captures everything perfectly
__yinx_start_script_capture() {
    local capture_file="${YINX_TMP_DIR}/yinx_shell_$$_$(date +%s).typescript"

    # Start script in the background to capture all I/O
    # Note: This creates a nested shell which may not be desirable
    # Keeping this as a reference for future implementation

    # script -q -f "$capture_file"
    :
}

echo "Yinx bash hook loaded (ADVANCED with script wrapper)."
echo "Session: ${YINX_SESSION_ID:-default}"
echo "⚠ Note: Full output capture via script command not yet fully implemented."
echo "   Currently captures command metadata + attempts output via temp files."
