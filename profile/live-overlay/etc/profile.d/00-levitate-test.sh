#!/bin/bash
# LevitateOS test mode instrumentation
# Activates ONLY on serial console (ttyS0) - test harness environment

# Only run in interactive shells
[[ $- != *i* ]] && return

# Detect test mode: serial console = test mode
if [[ $(tty) == /dev/ttyS0 ]]; then
    export LEVITATE_TEST_MODE=1
else
    # Not test mode - exit early
    return
fi

# ═══════════════════════════════════════════════════════════════════
# TEST MODE ACTIVE - Minimal instrumentation for test harness
# ═══════════════════════════════════════════════════════════════════

# Signal shell is ready - test harness waits for this
# recqemu's exec.rs has its own command marker protocol, so we don't need
# DEBUG traps here. Just emit the readiness marker.
echo "___SHELL_READY___"

# Emit prompt marker for first command
# Subsequent commands get ___PROMPT___ from PROMPT_COMMAND
echo "___PROMPT___"

# Simple PROMPT_COMMAND to emit prompt marker after each command
PROMPT_COMMAND='echo "___PROMPT___"'
