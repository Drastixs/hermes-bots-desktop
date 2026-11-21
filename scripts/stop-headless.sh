#!/usr/bin/env bash
RUN_DIR="${RUN_DIR:-/tmp/hbd-run}"
for p in app mock xvfb; do
  [ -f "$RUN_DIR/$p.pid" ] && kill "$(cat "$RUN_DIR/$p.pid")" 2>/dev/null; rm -f "$RUN_DIR/$p.pid"
done
true
