#!/usr/bin/env bash
# Give the app window keyboard focus (Xvfb has no window manager).
export DISPLAY="${1:-:91}"
for _ in $(seq 50); do W=$(xdotool search --name "Hermes Bots" 2>/dev/null | head -1); [ -n "$W" ] && break; sleep 0.2; done
xdotool windowfocus --sync "$W"
