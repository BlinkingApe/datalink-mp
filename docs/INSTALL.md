# Installation

This walkthrough targets macOS with [Whisky](https://getwhisky.app/); the same
steps apply to any Wine setup if you substitute your own `WINEPREFIX` and wine
binary.

## 1. Install the game and PRACX

1. Install Sid Meier's Alpha Centauri (Planetary Pack) from GOG into a Whisky
   bottle (or Wine prefix). The default install path inside the prefix is
   `drive_c/GOG Games/Sid Meier's Alpha Centauri Planetary Pack/`.
2. Install [PRACX](https://github.com/DrazharLn/pracx) into the same
   directory. You will launch `terran_PRACX.exe`, **not** `terran.exe` — the
   stock executable crashes during display-mode initialization under Wine on
   macOS.

## 2. Build this project

```bash
rustup target add i686-pc-windows-gnu
brew install mingw-w64        # or your distro's gcc-mingw-w64-i686

cargo build --release -p smac-helper
cargo build --release --target i686-pc-windows-gnu -p dplayx
```

Artifacts:

- `target/release/smac-helper` — native helper binary
- `target/i686-pc-windows-gnu/release/dplayx.dll` — the DirectPlay DLL

## 3. Install the DLL

Copy the DLL next to the game executable:

```bash
GAME_DIR="$HOME/Library/Containers/com.isaacmarovitz.Whisky/Bottles/<BOTTLE_ID>/drive_c/GOG Games/Sid Meier's Alpha Centauri Planetary Pack"
cp target/i686-pc-windows-gnu/release/dplayx.dll "$GAME_DIR/"
```

Tell Wine to load it instead of the builtin dplayx (once per prefix):

```bash
export WINEPREFIX="$HOME/Library/Containers/com.isaacmarovitz.Whisky/Bottles/<BOTTLE_ID>"
WINE="$HOME/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin/wine64"
"$WINE" reg add "HKEY_CURRENT_USER\Software\Wine\DllOverrides" /v dplayx /t REG_SZ /d native /f
```

## 4. Start the helper

The helper must be running before the game reaches the multiplayer menu.

**Host:**

```bash
./target/release/smac-helper host
# Prints a ticket string ("smac...") to stdout — share it with the joiners.
```

**Each joiner:**

```bash
./target/release/smac-helper join --ticket '<HOST_TICKET>'
```

The helper listens for the game on localhost TCP port **47624** by default.
If you run more than one game instance on the same machine (e.g. testing),
give each helper its own port (`--port 47625`) and point the matching game at
it with `SMAC_HELPER_PORT=47625`.

To keep stdout clean (it carries the ticket), send logs to a file:

```bash
SMAC_HELPER_LOG_FILE=/tmp/helper.log RUST_LOG=info ./target/release/smac-helper host
```

## 5. Launch the game

```bash
export WINEPREFIX="<bottle path>"
export WINEDEBUG="fixme-all"
# Only if the helper is on a non-default port:
# export SMAC_HELPER_PORT=47625

cd "$GAME_DIR"
"$WINE" start /unix "$GAME_DIR/terran_PRACX.exe"
```

`scripts/launch-whisky.sh` wraps this for macOS/Whisky.

## 6. In-game

- **Host:** Multiplayer → *Iroh P2P* → Host Game. Configure the lobby as usual.
- **Joiners:** Multiplayer → *Iroh P2P* → Join Game → select the host's
  session, pick a faction.
- Host starts the game when everyone is in.

"Iroh P2P" appears in the connection list because the DLL advertises itself as
a DirectPlay service provider under that name.

## Troubleshooting

- **No "Iroh P2P" entry / multiplayer menu empty** — the DLL override isn't
  active (step 3), or the DLL isn't in the game directory.
- **Session list empty on a joiner** — the joiner's helper isn't running, was
  started without `--ticket`, or the game can't reach it (check
  `SMAC_HELPER_PORT` matches the helper's port).
- **`PROTOCOL VERSION MISMATCH` in a helper log** — the machines run different
  builds of this project. Rebuild and redeploy everywhere.
- **Faction colors** — a vanilla MP bug (units wear the wrong faction's
  colors) is fixed by this mod; set `SMAC_NO_CLASSIC_COLORS=1` for vanilla
  behavior. See [faction-colors.md](faction-colors.md).
- **Diagnosing anything else** — set `DPLAYX_LOG_FILE='Z:\tmp\dplayx.log'` (the
  `Z:` drive is Wine's mapping of `/`) and `RUST_LOG=debug` on the helper, then
  read the logs side by side; timestamps are correlated.
