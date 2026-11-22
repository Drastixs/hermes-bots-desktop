#!/usr/bin/env bash
# Install Hermes Bots Desktop for the current user (no root).
#
#   scripts/install-user.sh              build the release binary (if needed) and install it
#   scripts/install-user.sh --no-build   install target/release/hermes-bots as it is
#   scripts/install-user.sh --uninstall  remove the installed files
#
# Files:
#   ~/.local/bin/hermes-bots
#   ~/.local/share/applications/io.github.drastixs.HermesBots.desktop
#   ~/.local/share/icons/hicolor/NxN/apps/io.github.drastixs.HermesBots.png
# Settings in ~/.config/hermes-bots-desktop and tokens in the keyring are not touched.
set -euo pipefail

APP_ID=io.github.drastixs.HermesBots
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
DATA=${XDG_DATA_HOME:-$HOME/.local/share}
BIN_DIR=${BIN_DIR:-$HOME/.local/bin}
APPS=$DATA/applications
ICONS=$DATA/icons/hicolor
SIZES=(32 64 128 256 512)

refresh_caches() {
  if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$APPS" || true
  fi
  if command -v gtk-update-icon-cache >/dev/null; then
    # gtk-update-icon-cache fails when the theme has no index.theme.
    if [[ ! -f $ICONS/index.theme && -f /usr/share/icons/hicolor/index.theme ]]; then
      cp /usr/share/icons/hicolor/index.theme "$ICONS/index.theme"
    fi
    if [[ -f $ICONS/index.theme ]]; then
      gtk-update-icon-cache -q -t "$ICONS" || true
    fi
  fi
}

uninstall() {
  rm -fv "$BIN_DIR/hermes-bots" "$APPS/$APP_ID.desktop"
  for n in "${SIZES[@]}"; do
    rm -fv "$ICONS/${n}x${n}/apps/$APP_ID.png"
  done
  refresh_caches
  echo "Hermes Bots is removed. Settings and keyring entries are kept."
}

build=1
case "${1:-}" in
  --uninstall) uninstall; exit 0 ;;
  --no-build) build=0 ;;
  "") ;;
  -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
  *) echo "Unknown option: $1" >&2; exit 2 ;;
esac

BIN=$ROOT/target/release/hermes-bots
if [[ $build == 1 ]]; then
  (cd "$ROOT" && cargo build --release --locked -p hermes-bots)
fi
if [[ ! -x $BIN ]]; then
  echo "No binary at $BIN. Run: cargo build --release -p hermes-bots" >&2
  exit 1
fi

install -Dm755 "$BIN" "$BIN_DIR/hermes-bots"
for n in "${SIZES[@]}"; do
  install -Dm644 "$ROOT/assets/icons/hermes-bots-$n.png" "$ICONS/${n}x${n}/apps/$APP_ID.png"
done
# Use the absolute binary path, because ~/.local/bin is not always on the launcher PATH.
mkdir -p "$APPS"
sed "s|^Exec=hermes-bots$|Exec=$BIN_DIR/hermes-bots|" "$ROOT/packaging/$APP_ID.desktop" > "$APPS/$APP_ID.desktop"
chmod 644 "$APPS/$APP_ID.desktop"
refresh_caches

echo "Installed: $BIN_DIR/hermes-bots"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Note: $BIN_DIR is not on your PATH. The app launcher entry still works." ;;
esac
