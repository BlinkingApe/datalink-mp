#!/bin/bash
# Launch SMAC from a Whisky bottle with the Iroh multiplayer DLL active.
#
# Usage:
#   scripts/launch-whisky.sh <bottle-path> [helper-port]
#
# Example:
#   scripts/launch-whisky.sh \
#     "$HOME/Library/Containers/com.isaacmarovitz.Whisky/Bottles/<BOTTLE_ID>" 47624
#
# Optional environment:
#   DPLAYX_LOG_FILE   Wine path for the DLL log (e.g. 'Z:\tmp\dplayx.log')
#   RUST_LOG          DLL log verbosity (default: info)
set -euo pipefail

BOTTLE="${1:?usage: launch-whisky.sh <bottle-path> [helper-port]}"
HELPER_PORT="${2:-47624}"

GAME_DIR="$BOTTLE/drive_c/GOG Games/Sid Meier's Alpha Centauri Planetary Pack"
WHISKY_WINE="$HOME/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin/wine64"
EXE="$GAME_DIR/terran_PRACX.exe"

[ -d "$BOTTLE" ]      || { echo "error: bottle not found: $BOTTLE" >&2; exit 1; }
[ -x "$WHISKY_WINE" ] || { echo "error: Whisky wine64 not found: $WHISKY_WINE" >&2; exit 1; }
[ -f "$EXE" ]         || { echo "error: terran_PRACX.exe not found in $GAME_DIR (install PRACX)" >&2; exit 1; }
[ -f "$GAME_DIR/dplayx.dll" ] || { echo "error: dplayx.dll not installed in $GAME_DIR (see docs/building.md)" >&2; exit 1; }

export WINEPREFIX="$BOTTLE"
export WINEDEBUG="${WINEDEBUG:-fixme-all}"
export SMAC_HELPER_PORT="$HELPER_PORT"
export RUST_LOG="${RUST_LOG:-info}"

echo "Launching SMAC (helper port $HELPER_PORT)..."
cd "$GAME_DIR"
exec "$WHISKY_WINE" start /unix "$EXE"
