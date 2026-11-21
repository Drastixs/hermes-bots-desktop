#!/usr/bin/env bash
# Start Xvfb + the mock gateway + the app. Writes PIDs to $RUN_DIR. Stop with scripts/stop-headless.sh.
# Usage: scripts/run-headless.sh [display] [port]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DISP="${1:-:91}"; PORT="${2:-9311}"
RUN_DIR="${RUN_DIR:-/tmp/hbd-run}"; mkdir -p "$RUN_DIR"
BIN="${BIN_DIR:-$ROOT/target/debug}"
source "$ROOT/scripts/headless-env.sh"
if ! xdpyinfo -display "$DISP" >/dev/null 2>&1; then
  Xvfb "$DISP" -screen 0 1280x800x24 -nolisten tcp >"$RUN_DIR/xvfb.log" 2>&1 & echo $! >"$RUN_DIR/xvfb.pid"
  for _ in $(seq 50); do xdpyinfo -display "$DISP" >/dev/null 2>&1 && break; sleep 0.1; done
fi
"$BIN/mock-hermes" --addr "127.0.0.1:$PORT" --speed "${MOCK_SPEED:-1.0}" >"$RUN_DIR/mock.log" 2>&1 & echo $! >"$RUN_DIR/mock.pid"
for _ in $(seq 50); do curl -sf "http://127.0.0.1:$PORT/api/health" >/dev/null && break; sleep 0.1; done
curl -sf -XPOST "http://127.0.0.1:$PORT/__mock/auto-approve" >/dev/null
export HERMES_BOTS_CONFIG_DIR="$RUN_DIR/config"; rm -rf "$HERMES_BOTS_CONFIG_DIR"
export HERMES_GATEWAY_URL="http://127.0.0.1:$PORT" HERMES_BOTS_AUTO_SIGNIN="${AUTO_SIGNIN:-1}" DISPLAY="$DISP"
"$BIN/hermes-bots" >"$RUN_DIR/app.log" 2>&1 & echo $! >"$RUN_DIR/app.pid"
echo "started: display $DISP, mock http://127.0.0.1:$PORT"
