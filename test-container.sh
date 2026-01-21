#!/bin/bash
# Quick test helper for LevitateOS container
# Usage: ./test-container.sh "command to run"

MACHINE_DIR="/var/lib/machines/levitateos"

if [ ! -d "$MACHINE_DIR" ]; then
    echo "Setting up container from tarball..."
    sudo mkdir -p "$MACHINE_DIR"
    sudo tar -xJf output/levitateos-base.tar.xz -C "$MACHINE_DIR"
fi

if [ -z "$1" ]; then
    # Interactive shell
    exec sudo systemd-nspawn -D "$MACHINE_DIR" /bin/bash
else
    # Run command
    sudo systemd-nspawn -D "$MACHINE_DIR" --pipe /bin/bash -c "$1"
fi
