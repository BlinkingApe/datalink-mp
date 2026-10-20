#!/usr/bin/env bash
# Starts datalink-mp with logging on, and prints the settings that switch on
# the game's DLL logging, for an internet-speed test. You start the game
# yourself from Faugus. See docs/players/collecting-logs.md.
#
# Run from the Game folder (the one holding datalink-mp and dplayx.dll):
#   ./datalink-mp-collect-logs.sh
#
# Optional environment:
#   LOG_DIR=<folder>   where logs go (default: ~/dl-logs)
set -u

cd "$(dirname "$(readlink -f "$0")")" || exit 1

LOG_DIR="${LOG_DIR:-$HOME/dl-logs}"

if [[ ! -x ./datalink-mp ]]; then
  echo "datalink-mp is not in $PWD. Extract the whole archive into your Game folder." >&2
  exit 1
fi
if [[ ! -f ./dplayx.dll ]]; then
  echo "dplayx.dll is not in $PWD. Extract the whole archive into your Game folder." >&2
  exit 1
fi

mkdir -p "$LOG_DIR"
# Under Wine and Proton, Z: is the Linux root.
WIN_DLL_LOG="Z:${LOG_DIR//\//\\}\\dll.log"

SMAC_HELPER_LOG_FILE="$LOG_DIR/helper.log" \
RUST_LOG=debug \
DATALINK_CAPTURE="$LOG_DIR/capture" \
  ./datalink-mp &
HELPER_PID=$!

stop_helper() {
  kill "$HELPER_PID" 2>/dev/null && wait "$HELPER_PID" 2>/dev/null
}
trap stop_helper EXIT
trap 'exit 130' INT TERM

sleep 2
if ! kill -0 "$HELPER_PID" 2>/dev/null; then
  echo "datalink-mp exited at once; see $LOG_DIR/helper.log" >&2
  exit 1
fi

cat <<TEXT

datalink-mp is running with logging on (pid $HELPER_PID).

In Faugus, open the game's settings and put this in Game Arguments / Launch
Arguments (one line), then press Play:

  WINEDLLOVERRIDES=dplayx=n,b DPLAYX_LOG_FILE='$WIN_DLL_LOG' DPLAYX_RXTRACE=1

Play your test. When the game has closed, come back here and press Enter (or
Ctrl-C) to stop datalink-mp so its log is saved.
TEXT

read -r _ || true

echo "Stopping datalink-mp ..."
stop_helper
trap - EXIT
echo "Logs are in $LOG_DIR:"
ls -l "$LOG_DIR" "$LOG_DIR/capture" 2>/dev/null
exit 0
