# Collecting logs for an internet-speed test

This page shows how to record the logs that tell us how fast turns sync over
the internet, and (once the feature ships) whether **Early acks** help. It
works the same way on Windows and Linux. You don't need it to play.

> **Status:** Early acks are not in the released Helper yet
> ([ADR 0005](../adr/0005-internet-play-speed-helpers-answer-acks.md) plans
> them as an opt-in switch). Until then these logs give a *baseline*. When the
> feature lands, repeat the test with its switch on and compare.

## What you'll produce

| File | Made by | Contains |
|---|---|---|
| `helper.log` | the Helper (datalink-mp) | connections, routes, errors |
| `dll.log` | the game's DLL | DirectPlay calls and traffic |
| `capture/capture-*.jsonl` | the Helper | every game message with timings (one side is enough) |

## Before you start

1. Both players run the **same release** (the version is shown at the bottom of the web page).
2. Test on **different networks**, ideally with one player on a phone hotspot. A Direct connection between two home networks hides the delay we want to measure.
3. Quit datalink-mp and the game so the logs start clean.

## The quick way: the log script

The Windows zip and the Linux tarball include a script that sets every variable below, starts the Helper, then starts the game. Extract it into your Game folder with the rest of the archive.

- **Windows:** double-click `datalink-mp-collect-logs.bat`. When the game closes, quit datalink-mp with the Quit button on its page. Logs go to `%USERPROFILE%\dl-logs`.
- **Linux (Faugus):** run `./datalink-mp-collect-logs.sh` from the Game folder. It starts datalink-mp with logging on and prints one line of settings to paste into the game's Game Arguments in Faugus (the Wine override plus the DLL log switches). Press Play in Faugus, test, then press Enter in the script's terminal to stop datalink-mp. Logs go to `~/dl-logs` (set `LOG_DIR=...` to change).

The Windows script looks for `wtp.exe`, `thinker.exe`, `terran_PRACX.exe` or `terranx.exe` (in that order); to name another, pass it as an argument. Steam launches can't use the Linux script; use the manual steps.

## Linux (by hand)

Run from a terminal. Replace `<you>` with your username.

```bash
mkdir -p ~/dl-logs

# 1. Start the Helper with logging on
export SMAC_HELPER_LOG_FILE=$HOME/dl-logs/helper.log
export RUST_LOG=debug
export DATALINK_CAPTURE=$HOME/dl-logs/capture
datalink-mp          # or ./target/release/datalink-mp from a source build

# 2. Start the game (Wine) with DLL logging on
WINEDLLOVERRIDES=dplayx=n,b \
DPLAYX_LOG_FILE='Z:\home\<you>\dl-logs\dll.log' \
DPLAYX_RXTRACE=1 \
wine terranx.exe
```

- Under Wine, `Z:` is your Linux `/`, so `/home/<you>/dl-logs` is `Z:\home\<you>\dl-logs`.
- The Helper variables must be set in the shell that starts the Helper.
- **Steam:** put this in the game's launch options instead of the `wine` line:
  `WINEDLLOVERRIDES=dplayx=n,b DPLAYX_LOG_FILE='Z:\home\<you>\dl-logs\dll.log' DPLAYX_RXTRACE=1 %command%`

## Windows (by hand)

Use **one PowerShell window** for both programs so the game inherits the variables.

```powershell
mkdir $env:USERPROFILE\dl-logs

# 1. Start the Helper with logging on
$env:SMAC_HELPER_LOG_FILE = "$env:USERPROFILE\dl-logs\helper.log"
$env:RUST_LOG = "debug"
$env:DATALINK_CAPTURE = "$env:USERPROFILE\dl-logs\capture"   # optional
.\datalink-mp.exe

# 2. In the same window (or a second one with the same $env: lines),
#    start the game with DLL logging on
$env:DPLAYX_LOG_FILE = "$env:USERPROFILE\dl-logs\dll.log"
$env:DPLAYX_RXTRACE = "1"
& "C:\path\to\Game\terranx.exe"
```

- Double-clicking the exe or launching from Steam **loses** these variables. Either start both from PowerShell, or set them permanently with
  `setx SMAC_HELPER_LOG_FILE "C:\Users\<you>\dl-logs\helper.log"` and reopen your terminal.
- In `cmd.exe`, write `set NAME=value` instead of `$env:NAME = "value"`.

## During the test

1. Host or join over the internet as usual.
2. For every turn, write down:
   - the **clock time the turn ended**, and
   - roughly **how long the sync took** (the "Upkeep Phase: …" boxes), plus any phase that lingered.
3. Quit the game, then the Helper, so the last log lines are saved.

## What to send

- `helper.log` and `dll.log` from each machine.
- The newest `capture-*.jsonl`.
- Your per-turn notes, and which machine was on the hotspot.

Maintainers can analyse a capture with
`python3 .scratch/internet-play-speed/analysis/analyse_capture.py <capture.jsonl>`.

## Good to know

- `RUST_LOG=debug` logs and captures are large and contain game state. Don't commit them (the repo's `captures/` folder is gitignored).
- The Ticket isn't written to the log in web-page mode, but skim the files before posting them publicly.
- Logging slows the Helper slightly. For a pure speed comparison, rely on the capture timings from a run with logging otherwise light.

Variable reference: [building.md](../contributors/building.md#diagnosing-problems) and [traffic-capture.md](../contributors/traffic-capture.md).
