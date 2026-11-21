#!/usr/bin/env bash
# End-to-end test: drive the real binary on Xvfb against the mock gateway.
# Checks the mock's state after each step and saves screenshots to $OUT (default docs/screenshots).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
OUT="${OUT:-$ROOT/docs/screenshots}"; mkdir -p "$OUT"
DISP="${DISP:-:91}"; PORT="${PORT:-9311}"; M="http://127.0.0.1:$PORT"
export DISPLAY="$DISP"
fail() { echo "E2E FAIL: $*" >&2; scripts/shot.sh "$OUT/../e2e-failure.png" "$DISP" || true; exit 1; }
state() { curl -sf "$M/__mock/state"; }
click() { xdotool mousemove "$1" "$2" click 1; sleep "${3:-0.4}"; }
wait_for() { # wait_for "desc" cmd...
  local d="$1"; shift; for _ in $(seq 60); do "$@" >/dev/null 2>&1 && return 0; sleep 0.25; done; fail "$d"; }
trap 'scripts/stop-headless.sh' EXIT

AUTO_SIGNIN=0 scripts/run-headless.sh "$DISP" "$PORT" >/dev/null
sleep 1; kill -0 "$(cat ${RUN_DIR:-/tmp/hbd-run}/app.pid)" 2>/dev/null || { tail -5 "${RUN_DIR:-/tmp/hbd-run}/app.log"; fail "app did not start"; }
scripts/focus.sh "$DISP"; sleep 1.5
scripts/shot.sh "$OUT/01-sign-in.png"
# 1. Sign in (the URL comes from HERMES_GATEWAY_URL; the mock auto-approves the browser step).
click 640 483 0.5
sleep 4
scripts/shot.sh "$OUT/02-roster.png"
# 2. Open Scout and send a message; wait for the streamed reply.
click 130 182 1
click 700 755 0.2; xdotool type --delay 10 "Give me an update"; xdotool key Return
wait_for "prompt reached the gateway" sh -c "curl -sf $M/__mock/state | grep -q 'Give me an update'"
sleep 2.4; scripts/shot.sh "$OUT/03-chat-streaming.png"
sleep 5; scripts/shot.sh "$OUT/04-chat-reply.png"
# 3. Ask for a login; the Bot asks to take over its screen.
click 700 755 0.2; xdotool type --delay 10 "Please log in to the job site"; xdotool key Return
sleep 8; scripts/shot.sh "$OUT/05-login-handoff.png"
# 4. Details pane with the computer preview.
click 1190 32 3; scripts/shot.sh "$OUT/06-details-computer.png"; xdotool key Escape; sleep 0.8
scripts/shot.sh /tmp/hbd-before-take.png
# 5. Take over from the card, type on the Bot screen, hand back.
TAKE=$(python3 "$ROOT/scripts/find_button.py" /tmp/hbd-before-take.png) || fail "no Take over button"
click $TAKE 3
wait_for "lease held by human" sh -c "curl -sf $M/__mock/state | grep -q '\"scout\":\"human\"'"
scripts/shot.sh "$OUT/07-screen-control-empty.png"
click 775 388 0.3; xdotool type --delay 25 "me@example.com"; click 775 457 0.3; xdotool type --delay 25 "secret"; sleep 1.5
scripts/shot.sh "$OUT/08-screen-control.png"
state | grep -q '"email":"me@example.com"' || fail "typed email did not reach the Bot screen: $(state)"
click 779 518 1.2
state | grep -q '"signed_in":true' || fail "sign-in click did not reach the Bot screen"
click 905 745 1.5   # I'm done
wait_for "lease back to agent" sh -c "curl -sf $M/__mock/state | grep -q '\"scout\":\"agent\"'"
scripts/shot.sh "$OUT/09-screen-watching.png"
state | grep -q 'handed control back' || fail "continue message was not sent after hand-back"
xdotool key Escape; sleep 1
# 6. Server drop: the app shows reconnecting and recovers.
curl -sf -XPOST "$M/__mock/offline?secs=6" >/dev/null; sleep 1.2
scripts/shot.sh "$OUT/10-offline.png"
sleep 9
click 700 755 0.2; xdotool type --delay 10 "Still there?"; xdotool key Return
wait_for "message after reconnect" sh -c "curl -sf $M/__mock/state | grep -q 'Still there?'"
# 7. Settings and create Bot dialogs.
xdotool key ctrl+comma; sleep 1; scripts/shot.sh "$OUT/11-settings.png"; xdotool key Escape; sleep 1
xdotool key ctrl+n; sleep 1.5; xdotool type --delay 30 "Research Buddy"; sleep 0.3; xdotool key Tab; sleep 0.3; xdotool type --delay 10 "Finds papers and sums them up"; sleep 1.2
scripts/shot.sh "$OUT/12-create-bot.png"; xdotool key Return; sleep 2.5
curl -sf -H "Authorization: Bearer x" "$M/api/profiles" >/dev/null 2>&1 || true
scripts/shot.sh "$OUT/13-new-bot.png"
grep -q "research-buddy" <(xdotool getwindowname "$(xdotool search --name 'Hermes Bots' | head -1)" | tr 'A-Z ' 'a-z-') || fail "new Bot was not selected"
echo "E2E OK"
