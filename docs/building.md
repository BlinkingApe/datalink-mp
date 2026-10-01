# Building from source

This document is for people who want to build datalink-mp themselves. If you
just want to play, download a release and follow the quickstart in the
[README](../README.md).

Nothing here is needed to play, and everything below the build step describes
how to assemble by hand what a release archive already contains. The
walkthrough was written on macOS with [Whisky](https://getwhisky.app/) and has
not been re-tested there; macOS is untested, as in the README. The same steps
apply to any Wine setup if you substitute your own `WINEPREFIX` and Wine
binary.

## 1. Install the game and Thinker or PRACX

1. Install Sid Meier's Alpha Centauri (Planetary Pack) into a Wine bottle or
   prefix (or natively on Windows). With GOG under Whisky the default Game
   folder inside the prefix is
   `drive_c/GOG Games/Sid Meier's Alpha Centauri Planetary Pack/`.
2. Install [Thinker](https://github.com/induktio/thinker) or
   [PRACX](https://github.com/DrazharLn/pracx) into the same Game folder. You
   will start `thinker.exe` or `terran_PRACX.exe`, not the stock `terran.exe`,
   which crashed during display-mode initialization in earlier testing under
   Wine on macOS.

## 2. Build this project

You need stable Rust, the `i686-pc-windows-gnu` target and mingw-w64 to link
the 32-bit DLL:

```bash
rustup target add i686-pc-windows-gnu
brew install mingw-w64        # macOS
# apt install gcc-mingw-w64-i686  # Debian/Ubuntu

cargo build --release -p datalink-mp
cargo build --release --target i686-pc-windows-gnu -p dplayx
```

Artifacts:

- `target/release/datalink-mp` (`datalink-mp.exe` on Windows): the Helper
- `target/i686-pc-windows-gnu/release/dplayx.dll`: the DLL

Both Windows binaries carry a VERSIONINFO resource with the workspace
version (Properties → Details on Windows). Their `build.rs` compiles it with
mingw's `windres`, which comes with the mingw-w64 packages above; native
Linux and macOS builds skip it.

The DLL links with mingw's own `_Unwind_Resume` from `libgcc_eh` (linked
with Fedora's GCC 15.1; Ubuntu 24.04's GCC 13.2, which CI uses, ships the
same DWARF `libgcc_eh`). Older versions of this
project compiled a C stub for it, which clashed with `libgcc_eh` on newer
mingw with `multiple definition of '_Unwind_Resume'`. If a mingw toolchain
ever reports that again, add the linker flag per-target, so
`.cargo/config.toml`'s `control-flow-guard=no` and `-lws2_32` stay in effect —
bare `RUSTFLAGS` replaces them instead of adding to them:

```bash
CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition" \
  cargo build --release --target i686-pc-windows-gnu -p dplayx
```

### Cross-building the Helper for Windows

To produce a Windows `datalink-mp.exe` without a Windows machine (for example
to stage a manual test), add the 64-bit mingw target and cross-build:

```bash
rustup target add x86_64-pc-windows-gnu
brew install mingw-w64              # macOS
# apt install gcc-mingw-w64-x86-64  # Debian/Ubuntu

cargo build --release --target x86_64-pc-windows-gnu -p datalink-mp
```

Artifact: `target/x86_64-pc-windows-gnu/release/datalink-mp.exe`. It links no
mingw runtime DLLs (libgcc/winpthread are statically linked).

### Building the static Linux Helper

Releases ship a static musl Helper, so it runs on any x86_64 distro whatever
its glibc. `ring` compiles C, so it needs a musl C compiler:

```bash
rustup target add x86_64-unknown-linux-musl
# apt install musl-tools  # Debian/Ubuntu

CC_x86_64_unknown_linux_musl=musl-gcc \
  cargo build --release --target x86_64-unknown-linux-musl -p datalink-mp
```

These cross-builds are for experiments. The archives players download are
built by CI from a tag; see [releasing.md](releasing.md).

## 3. Assemble your Game folder

Copy both artifacts into your Game folder, next to the game executable:

```bash
GAME_DIR="$HOME/Library/Containers/com.isaacmarovitz.Whisky/Bottles/<BOTTLE_ID>/drive_c/GOG Games/Sid Meier's Alpha Centauri Planetary Pack"
cp target/i686-pc-windows-gnu/release/dplayx.dll "$GAME_DIR/"
cp target/release/datalink-mp "$GAME_DIR/"
```

On Linux and macOS, Wine must be told to load the DLL instead of its own
`dplayx.dll`. As in the README, add `WINEDLLOVERRIDES="dplayx=n,b"` to your
launcher's environment (Faugus: unquoted, in Game Arguments). If you drive Wine
from your own scripts, you can instead set the override once per prefix:

```bash
export WINEPREFIX="$HOME/Library/Containers/com.isaacmarovitz.Whisky/Bottles/<BOTTLE_ID>"
WINE="$HOME/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin/wine64"
"$WINE" reg add "HKEY_CURRENT_USER\Software\Wine\DllOverrides" /v dplayx /t REG_SZ /d native /f
```

`scripts/launch-whisky.sh` relies on that registry setting; it starts the game
from a Whisky bottle and is a maintainer convenience, not part of a release.

## 4. Start datalink-mp, then the game

Double-click `datalink-mp` in the Game folder, as a player would. Your browser
opens the page, which checks the Game folder for you and shows your Ticket.
Start it before the game reaches the multiplayer menu.

The Helper listens for the game on localhost TCP port **47624** by default.
If you run more than one game on the same machine (for example when testing),
give each Helper its own port and point the matching game at it with
`SMAC_HELPER_PORT`; see "For power users" in the [README](../README.md).

## 5. In the game

- **Host:** Multiplayer → *Iroh P2P* → Host Game. Configure the lobby as usual.
- **Joiners:** Multiplayer → *Iroh P2P* → Join Game → select the host's
  session, pick a faction.
- The host starts the game when everyone is in.

"Iroh P2P" appears in the connection list because the DLL advertises itself as
a DirectPlay service provider under that name.

## Repository layout

| Path | What |
|---|---|
| `crates/dplayx` | the DirectPlay replacement DLL (32-bit Windows) |
| `crates/datalink-mp` | the Helper: native networking, session state and web page |
| `crates/iroh-transport` | Iroh session/mesh/ordered-stream transport |
| `crates/dp-types` | DirectPlay structs, GUIDs, serialization |
| `crates/ipc-protocol` | DLL ↔ Helper localhost protocol |
| `crates/smac-fixes` | in-memory game patches + diagnostic probe system |
| `tools/mock-dp-client` | interactive transport test client (no game needed) |

The design is in [ARCHITECTURE.md](ARCHITECTURE.md).

## Diagnosing problems

Every layer can log:

| Variable | Component | Effect |
|---|---|---|
| `SMAC_HELPER_LOG_FILE=/path` | Helper | log to file (standard output stays clean for the Ticket in `host` mode) |
| `RUST_LOG=debug` | Helper | log verbosity |
| `DPLAYX_LOG_FILE='Z:\path'` | DLL | DirectPlay call/traffic log (Wine's `Z:` drive maps `/`) |
| `DPLAYX_RXTRACE=1` | DLL | per-message send/receive trace lines |
| `SMAC_PROBE_LOG=/path` | DLL | game-internal diagnostic probes (see [ARCHITECTURE.md](ARCHITECTURE.md)) |

Set `DPLAYX_LOG_FILE` and `RUST_LOG=debug`, then read the two logs side by
side; timestamps are correlated.

Symptoms and their usual causes:

- **No "Iroh P2P" entry / multiplayer menu empty:** the DLL override isn't
  active (step 3), or the DLL isn't in the Game folder.
- **Session list empty on a joiner:** the joiner's Helper isn't connected to
  the host's Ticket, or the game can't reach the Helper (check that
  `SMAC_HELPER_PORT` matches the Helper's port).
- **A mismatch in a Helper log:** the machines run different Release versions.
  Build and deploy the same one everywhere.
- **Faction colors:** vanilla multiplayer colors units by seat, not faction;
  this project fixes that. Set `SMAC_NO_CLASSIC_COLORS=1` for vanilla behavior.
  See [faction-colors.md](faction-colors.md).
