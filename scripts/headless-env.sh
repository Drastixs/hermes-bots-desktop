#!/usr/bin/env bash
# Shared env for headless runs (Xvfb + software renderer). Source this file.
export WAYLAND_DISPLAY=
export ICED_BACKEND=tiny-skia
export HERMES_BOTS_NO_KEYRING=1
export HERMES_BOTS_NO_BROWSER=1
# Optional local copy of libxkbcommon-x11 for hosts that lack it (CI installs the package).
if [ -n "${HBD_EXTRA_LIBS:-}" ]; then export LD_LIBRARY_PATH="$HBD_EXTRA_LIBS:${LD_LIBRARY_PATH:-}"; fi
