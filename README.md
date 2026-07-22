# SMAC Iroh Multiplayer

Modern peer-to-peer multiplayer for **Sid Meier's Alpha Centauri** (1999), built
by replacing Wine's DirectPlay (`dplayx.dll`) with a Rust implementation that
routes the game's networking over [Iroh](https://www.iroh.computer/) — QUIC,
hole-punching, and relays included. No port forwarding, no IPX emulators, no
VPN: the host shares one ticket string and everyone connects.

**Status: experimental, but real.** Full 3-player games — lobby, faction
selection, simultaneous-turn play, diplomacy — have been played over this stack
on macOS via Wine/Whisky, with instrumented runs verifying zero message loss
and strict per-peer ordering.

## How it works

```
┌───────────────────────────────┐
│  Wine process                 │
│  ┌─────────────┐  DirectPlay  │        TCP          ┌─────────────┐
│  │ terran.exe   │◄───COM─────►│      localhost      │ smac-helper │
│  │ (the game)   │  dplayx.dll │◄───────IPC─────────►│  (native)   │
│  └─────────────┘  (this repo) │                     │    Iroh     │
└───────────────────────────────┘                     └──────┬──────┘
                                                             │ QUIC / P2P
                                                        other players
```

Two components, both in this repo:

- **`dplayx.dll`** — a 32-bit Windows DLL implementing the DirectPlay COM
  interfaces the game uses (sessions, players, sends, system messages). It is
  loaded by Wine *instead of* Wine's built-in dplayx. It contains no game
  networking itself; it forwards everything over localhost TCP to the helper.
- **`smac-helper`** — a native binary that owns the actual networking: an Iroh
  endpoint, one ordered QUIC stream per peer, session/roster state, and
  DirectPlay system-message synthesis.

The split exists because tokio (and therefore Iroh) cannot run inside Wine —
Wine's `\Device\Afd` doesn't support mio's IOCP model. See
[docs/wine-compatibility.md](docs/wine-compatibility.md) for the investigation
and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

## Requirements

- Your own copy of **Sid Meier's Alpha Centauri** (e.g. from
  [GOG](https://www.gog.com/game/sid_meiers_alpha_centauri)). This repo
  contains no game files.
- **[PRACX](https://github.com/DrazharLn/pracx)** (`terran_PRACX.exe`) — the
  community patch. The stock `terran.exe` crashes on display-mode
  initialization under Wine on macOS; PRACX fixes it.
- **Wine** — tested with [Whisky](https://getwhisky.app/) on macOS. Other Wine
  setups should work but are untested.
- **Rust** (stable) with the `i686-pc-windows-gnu` target, plus **mingw-w64**
  to link the 32-bit DLL:

  ```bash
  rustup target add i686-pc-windows-gnu
  brew install mingw-w64          # macOS
  # apt install gcc-mingw-w64-i686  # Debian/Ubuntu
  ```

## Quickstart

Build both components:

```bash
cargo build --release -p smac-helper
cargo build --release --target i686-pc-windows-gnu -p dplayx
```

Install (detailed walkthrough in [docs/INSTALL.md](docs/INSTALL.md)):

1. Copy `target/i686-pc-windows-gnu/release/dplayx.dll` into the game
   directory (next to `terran_PRACX.exe`).
2. Tell Wine to prefer it over the builtin:
   ```bash
   WINEPREFIX=/path/to/prefix wine reg add \
     "HKEY_CURRENT_USER\Software\Wine\DllOverrides" \
     /v dplayx /t REG_SZ /d native /f
   ```

Play:

```bash
# Host machine — prints a ticket string to stdout
./target/release/smac-helper host

# Each joining machine
./target/release/smac-helper join --ticket '<HOST_TICKET>'

# Then launch the game on every machine (scripts/launch-whisky.sh automates
# this on macOS/Whisky) and in-game:
#   Multiplayer → Iroh P2P → Host Game   (host)
#   Multiplayer → Iroh P2P → Join Game   (joiners)
```

The helper and the game talk over localhost TCP (default port 47624, override
with `SMAC_HELPER_PORT` — needed if you run two game instances on one
machine).

## Troubleshooting

Every layer can log:

| Variable | Component | Effect |
|---|---|---|
| `SMAC_HELPER_LOG_FILE=/path` | helper | log to file (stdout stays clean for the ticket) |
| `RUST_LOG=debug` | helper | log verbosity |
| `DPLAYX_LOG_FILE='Z:\path'` | DLL | DirectPlay call/traffic log (Wine `Z:` maps `/`) |
| `DPLAYX_RXTRACE=1` | DLL | per-message send/receive trace lines |
| `SMAC_PROBE_LOG=/path` | DLL | game-internal diagnostic probes (see ARCHITECTURE.md) |

If two machines run different builds, the helpers refuse to talk and log
`PROTOCOL VERSION MISMATCH` — rebuild and redeploy on both sides.

**Faction colors**: vanilla SMAC multiplayer colors units/flags/labels by
seat, not faction — a game bug this mod fixes so every faction wears its
classic colors (`SMAC_NO_CLASSIC_COLORS=1` reverts). See
[docs/faction-colors.md](docs/faction-colors.md).

## Repository layout

| Path | What |
|---|---|
| `crates/dplayx` | the DirectPlay replacement DLL (32-bit Windows) |
| `crates/smac-helper` | native networking helper binary |
| `crates/iroh-transport` | Iroh session/mesh/ordered-stream transport |
| `crates/dp-types` | DirectPlay structs, GUIDs, serialization |
| `crates/ipc-protocol` | DLL ↔ helper localhost protocol |
| `crates/smac-fixes` | in-memory game patches + diagnostic probe system |
| `tools/mock-dp-client` | interactive transport test client (no game needed) |

## Credits

- [PRACX](https://github.com/DrazharLn/pracx) — the community patch that makes
  SMAC viable under Wine.
- [Iroh](https://github.com/n0-computer/iroh) by n0 — the P2P layer.
- [quinn](https://github.com/quinn-rs/quinn) — the QUIC implementation.
- The Wine project — whose open dplayx source made the DirectPlay surface
  tractable to reimplement.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.

Sid Meier's Alpha Centauri is a trademark of its respective owners. This
project is an unaffiliated interoperability layer and contains no game assets.
