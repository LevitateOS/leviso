# Auto-launch tmux with docs-tui on live ISO boot
# Only runs on tty1, only if not already in tmux
# This script only exists in live-overlay, so if it's running, we're in live mode

# Skip if already in tmux
[ -n "$TMUX" ] && return

# Skip if not on tty1 (allow SSH and other ttys to be normal shells)
[ "$(tty)" != "/dev/tty1" ] && return

# Skip if levitate-docs not available
command -v levitate-docs >/dev/null 2>&1 || return

# Skip if tmux not available
command -v tmux >/dev/null 2>&1 || return

# Launch tmux with shell on left, docs on right (50/50 split)
exec tmux new-session -d -s live \; \
    set-option -g prefix None \; \
    set-option -g mouse on \; \
    set-option -g status-style 'bg=black,fg=white' \; \
    set-option -g status-left '' \; \
    set-option -g status-right ' Alt+Tab: switch | Ctrl+Left/Right: resize | F1: help ' \; \
    set-option -g status-right-length 60 \; \
    bind-key -n M-Tab select-pane -t :.+ \; \
    bind-key -n BTab select-pane -t :.+ \; \
    bind-key -n F2 select-pane -t :.+ \; \
    bind-key -n C-Left resize-pane -L 5 \; \
    bind-key -n C-Right resize-pane -R 5 \; \
    bind-key -n F1 display-message 'Alt+Tab: switch panes | Ctrl+Left/Right: resize | In docs: Up/Down navigate, j/k scroll, q quit' \; \
    split-window -h levitate-docs \; \
    select-pane -t 0 \; \
    attach-session -t live
