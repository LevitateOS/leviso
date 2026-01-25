#!/bin/bash
# LevitateOS test mode instrumentation
# Activates ONLY on serial console (ttyS0) - test harness environment
# Users on tty1 see normal behavior (+ docs-tui from live-docs.sh)

# Only run in interactive shells
[[ $- != *i* ]] && return

# Detect test mode: serial console = test mode
if [[ $(tty) == /dev/ttyS0 ]]; then
    export LEVITATE_TEST_MODE=1
else
    # Not test mode - exit early, let live-docs.sh handle normal UX
    return
fi

# ═══════════════════════════════════════════════════════════════════
# TEST MODE ACTIVE - Emit structured markers for install-tests harness
# ═══════════════════════════════════════════════════════════════════

_LEV_CMD_ID=""

# Called BEFORE each command (DEBUG trap)
_levitate_pre_cmd() {
    # Skip for PROMPT_COMMAND itself and tab completion
    [[ -n "$COMP_LINE" ]] && return
    [[ "$BASH_COMMAND" == "$PROMPT_COMMAND" ]] && return
    [[ "$BASH_COMMAND" == "_levitate_post_cmd" ]] && return

    _LEV_CMD_ID=$(date +%s%3N)
    echo "___CMD_START_${_LEV_CMD_ID}_${BASH_COMMAND}___"
}

# Called AFTER each command (PROMPT_COMMAND)
_levitate_post_cmd() {
    local exit_code=$?

    if [[ -n "$_LEV_CMD_ID" ]]; then
        echo "___CMD_END_${_LEV_CMD_ID}_${exit_code}___"
        _LEV_CMD_ID=""
    fi

    # Emit prompt marker - tells test harness shell is ready for next command
    echo "___PROMPT___"
}

# Set up traps
trap '_levitate_pre_cmd' DEBUG
PROMPT_COMMAND='_levitate_post_cmd'

# Signal shell is ready - test harness waits for this instead of warmup
echo "___SHELL_READY___"
# Emit initial prompt marker for first command
# PROMPT_COMMAND hasn't run yet (shell hasn't displayed prompt), so we emit it here
# Subsequent commands will get ___PROMPT___ from PROMPT_COMMAND
echo "___PROMPT___"
