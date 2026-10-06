# ADR-0001: Embedded local web UI as the primary front door for datalink-mp

- **Status:** Accepted (2026-09-30; revised from the Proposed draft, see [Changes from the draft](#changes-from-the-draft))
- **Applies to:** `datalink-mp`, a standalone project seeded from `hdevalence/smac-iroh` ([ADR-0003](0003-standalone-project.md)); Windows x86_64, Linux x86_64 and macOS aarch64 releases
- **Audience:** implementing agent working in the repo
- **Related:** [ADR-0002](0002-releases-versioning-pipeline-trust.md) (versioning, release pipeline, trust posture), [ADR-0003](0003-standalone-project.md) (standalone project). Decision trail: `.scratch/archive/web-ui-v1/map.md`.

## Context

The seed project, smac-iroh, replaces Wine's `dplayx.dll` with a Rust DirectPlay implementation (`crates/dplayx`, 32-bit Windows DLL). The DLL forwards all game networking over localhost TCP (default port 47624, override `SMAC_HELPER_PORT`) to the Helper (`crates/smac-helper`, native), which owns the Iroh endpoint, session state and the Ticket.

- The DLL and Helper have been built and tested on Windows, and Linux ↔ Windows multiplayer works.
- Today's usage needs a terminal: `smac-helper host` prints a Ticket, `smac-helper join --ticket '<TICKET>'` joins, then the game's Multiplayer → Iroh P2P → Host/Join Game.
- On Wine/Proton, putting `dplayx.dll` in the game folder is **not** enough: the launcher must set `WINEDLLOVERRIDES=dplayx=n,b` (tested under GE-Proton11-7 via Faugus and the shell).

### The v1 promise

A player with SMAC plus Thinker or PRACX extracts one archive into the game folder and double-clicks the Helper, then plays multiplayer with friends on Windows, Linux or Mac **without typing any commands**. Linux/Mac players paste the Wine override into their launcher themselves; Mac players also do a one-time Gatekeeper "Open Anyway", and on Mac a Terminal window opens by itself and must stay open.

## Decision

Make **an embedded, local-only web UI served by the Helper itself** the default way to use the tool.

- The Helper binary is renamed to `datalink-mp` (ADR-0003). Running it with **no subcommand** starts the UI (no documented usage relies on the old default of `host`). `host` and `join` keep smac-helper's behaviour, for power users and scripts.
- The UI is a single static HTML/JS page embedded in the binary (`include_str!`), talking to a small JSON API on the Helper's own HTTP server (axum). No frontend framework, no npm, no build step.
- The Helper opens the default browser at `http://127.0.0.1:<ui-port>/?t=<token>` on startup and always prints the URL too.
- The **game folder is the Helper's own folder** (found via `current_exe()`, never the working directory). No auto-detection, no config file, no install step.

## Requirements

### 1. Startup

- UI port: a fixed default different from 47624, configurable via `--ui-port` and `SMAC_UI_PORT`; if taken, try the next free ports. Exact default, fallback range and the single-instance mechanism (a second launch opens the browser to the running instance) are left to the spec.
- Bind `127.0.0.1` only. Random token (≥128 bits, OS CSPRNG) in the launch URL, required on every API call.
- Browser opening:
  - **Windows:** do **not** use the `open` crate (5.4.x spawns a hidden `powershell.exe`, a classic AV detection pattern). Use the `webbrowser` crate or `ShellExecuteW` via `windows-sys`, implementer's choice.
  - **Linux:** `xdg-open` (or `webbrowser`). **macOS:** `/usr/bin/open` (untested).
- **Startup self-check** of the game folder: `dplayx.dll` present, and at least one of `thinker.exe` / `terran_PRACX.exe` / `wtp.exe`. Failing that, the UI shows the "not your game folder" banner; if only the DLL is missing, the banner adds that antivirus may have quarantined it.
- Windows keeps its console window. macOS double-click runs the Helper in a Terminal window (closing it quits); do not wrap it in a `.app` (that would trigger macOS 15's local-network prompt). Linux: the user double-clicks → Run.

### 2. Process structure

- `main` stays sync. axum gets its own runtime owned by `main`.
- The IPC listener is bound **once** at startup and runs on a plain thread; it looks up the **current Transport** per request, via a controller that owns it.
- Every blocking Transport call, and every drop of a Transport, runs inside `spawn_blocking` (a nested-runtime panic otherwise; release builds use `panic = "abort"`). The last `Arc<Transport>` must never be dropped on an async thread.
- The UI and the CLI drive the **same** session code; extract `host`/`join` into the controller rather than duplicating networking logic.

### 3. Session model: Ticket from startup, Stop in-process

- The Helper has a Ticket from the moment it starts; there is no "start hosting" action. Hosting is simply "a peer dialled us".
- **Stop** is done in-process, not by re-exec (a re-exec would orphan the running game's IPC connection, since the DLL never reconnects after an IPC error):
  - Add `Transport::shutdown()` (`disconnect_all` + `endpoint.close()`, ≤3 s).
  - The controller replaces it with a fresh Transport, which means a **new Ticket**. The UI shows "your Ticket changed, share it again".
- **Quit** exits the Helper (also available via the console / Terminal window).
- Joining has a visible "Joining" state and a dial timeout shorter than QUIC's 30 s default.

### 4. UI

One page, usable in a narrow window, no external assets. Four numbered steps that tick off from live status (reference: prototype variant C on branch `prototype/ui-flow`, `6399164`):

1. **Game folder**: the self-check result. On Linux/macOS the Wine override setup text sits here (section 6).
2. **Share or paste a Ticket**: your Ticket (always shown, Copy button with select-all fallback) beside the friend's Ticket box and Connect. Once connected: "Connected to your friend".
3. **Start the game**: "In the game: Multiplayer → Iroh P2P → Host Game / Join Game", with a game-connected pill.
4. **Play**: peer count with short IDs, and the per-OS "keep this open" line.

- Header: Stop (only while in a session) and Quit. Banners go under the header. Footer: Release version (IPC and Peer protocol versions as small print) and state.
- **Status:** the page polls `GET /api/status` about once a second. It shows the Release version, state, whether the DLL is connected, and the peer count with short IDs. No player names.
- **Banners** (five, plus the self-check one):
  - IPC version mismatch: re-extract the archive.
  - Peer protocol version mismatch: your friend has a different release; both need the same one. Driven by the ALPN rejection (ADR-0002).
  - Invalid Ticket.
  - Can't reach the host: ask them for their current Ticket, since it changes after Stop.
  - IPC port in use: set on `AddrInUse` at bind; don't exit.
  - Not your game folder (maybe quarantined, see section 1).

### 5. HTTP API (suggested shape; the spec may adjust)

All endpoints require the token (header `X-Token`; the page moves it from the launch URL into the header). Reject with 403 otherwise.

- `GET /api/status`: state, versions, peers, DLL-connected flag, self-check result, the Ticket, active banner
- `POST /api/join`: body `{ "ticket": "..." }`
- `POST /api/stop`: leave the session, new Transport, new Ticket
- `POST /api/quit`

**Ticket validation:** `iroh_transport::Ticket::parse` (`ticket.rs`), with no I/O. **Trim whitespace first** (parse doesn't). Also reject our own Ticket, and warn when a Ticket carries no addresses.

**Observability:** keep one shared status struct, written at the existing sites:
- DLL connect/disconnect in `main.rs`
- IPC version mismatch at the handshake, where a decode failure on the first message also counts as a likely mismatch
- a new `Transport::connected_peers()` for the peer list
- the dial error for "can't reach host", including the DLL-driven join error
- the ALPN rejection for a Peer protocol mismatch

### 6. Setup text (Linux / macOS)

- The Wine override string `WINEDLLOVERRIDES=dplayx=n,b`, with **Copy** buttons for:
  - **Generic** (tested): `WINEDLLOVERRIDES="dplayx=n,b"` before the launch command.
  - **Faugus** (tested): paste `WINEDLLOVERRIDES=dplayx=n,b` **unquoted** into the game's "Game Arguments" field, space-separated from anything already there.
- One untested line, with no button: "Other launchers (Steam, Lutris…) should work: add the override to the launch environment, e.g. Steam launch options `WINEDLLOVERRIDES="dplayx=n,b" %command%`."
- macOS: "any Wine on macOS (e.g. CrossOver)"; Whisky is archived. Bottles live under the hidden `~/Library`, so point users to Finder's Go → Go to Folder (⇧⌘G).
- Nothing is written outside the game folder, and the Helper writes nothing there either.

README/quickstart wording per OS is left to the spec. It follows the standing rule: claim only what was tested; everything else is "should work". macOS is labelled untested throughout.

### 7. Security

- Bind `127.0.0.1` only; require the token on every request.
- Validate the `Host` header (`127.0.0.1:<port>` or `localhost:<port>`) to defeat DNS rebinding.
- No CORS headers. For state-changing endpoints, reject a foreign `Origin` when present. State-changing endpoints are POST only.
- Don't log the token at info level; printing the launch URL once at startup is fine.
- There are no file-writing endpoints.

### 8. Packaging

One archive per OS, containing the Helper, `dplayx.dll`, README and licences; `SHA256SUMS` alongside. The user extracts it into the game folder.
- **Windows:** `.zip`.
- **Linux:** `.tar.gz` (keeps the executable bit). No `.desktop` file and no `run.sh`; the README says double-click → Run.
- **macOS:** `.zip` built with `ditto`.

How the archives are built, versioned and released: [ADR-0002](0002-releases-versioning-pipeline-trust.md).

## Consequences

**Positive**
- One extract, one double-click, no commands typed.
- Cross-platform by construction (browser + Rust).
- No new runtime dependencies for users. For developers: axum, and possibly `webbrowser`.
- The CLI stays fully supported and unchanged.

**Negative / risks**
- A local HTTP attack surface; mitigated by section 7.
- The Ticket changes on every launch and every Stop, so players must re-share it. A stable Ticket would need a persisted key, which would reopen "no config" (out of scope).
- On Linux/Mac the user still has to paste the Wine override into their launcher.
- A Mac user sees a Terminal window. That's accepted: the alternative `.app` wrapper triggers a local-network permission prompt.
- SmartScreen, Firewall and AV issues: see ADR-0002.

## Alternatives considered

- **Native GUI (`egui`/`eframe`):** a larger binary and more Linux build dependencies. Its SmartScreen/AV profile is about the same as the web UI's, so it gains nothing there. Rejected.
- **Tauri:** needs a system webview, and has a slightly worse AV profile. Rejected.
- **Python/Tkinter:** requires Python. Rejected.
- **Separate frontend process:** more moving parts for no benefit. Rejected.
- **Re-exec the Helper for Stop:** it would orphan the running game's IPC connection. Rejected in favour of in-process Transport replacement.
- **SSE for status:** `EventSource` can't send the token header. Rejected in favour of polling.

## Out of scope

- Windows "Launch game" button (post-v1), and a config file.
- Game-folder auto-detection, a native folder picker, and `wine reg add` into a prefix.
- Code signing, auto-update, tray icon, NAT/relay configuration UI, SSE.
- Intel Mac builds, Lutris-specific instructions, and macOS work beyond the CI build and docs.
- Wire-protocol or DirectPlay changes beyond the versioned ALPN (ADR-0002).
- A stable Ticket across Stop or relaunch.

## Acceptance criteria

1. On a fresh Windows machine with the game: extracting the release zip into the game folder and double-clicking the Helper (after SmartScreen "Run anyway") opens the browser to the page, the self-check passes, and a Ticket is shown, with no commands typed.
2. A second machine (Windows, or Linux with the override set) pastes that Ticket, connects, and the two play a game together.
3. Stop gives a new Ticket without quitting, and a game connected to the Helper keeps working through a later session.
4. `datalink-mp host` and `datalink-mp join --ticket` behave as `smac-helper`'s did.
5. Requests are rejected if they lack the token, carry a wrong `Host` header, or come from a foreign `Origin`.
6. Each banner is triggered by its condition, including the Peer protocol mismatch between builds with different ALPNs and the self-check outside a game folder.
7. `cargo test` still passes, including the mesh test in `crates/iroh-transport`.
8. The release pipeline in ADR-0002 produces all three archives.

## Changes from the draft

Where this revision contradicts the Proposed draft, and why:

1. **Host/Join toggle and `POST /api/host` removed.** The Ticket exists from startup, and the UI is four numbered steps (UI flow prototype).
2. **SSE and `GET /api/events` removed.** `EventSource` can't carry the token header, so the page polls.
3. **Embedded DLL (`include_bytes!`), game-folder auto-detection, the paste-a-path field, `/api/setup/install` and the hash-compare/update step all removed.** The user extracts the archive into the game folder, and a self-check finds the folder via `current_exe()`.
4. **"PRACX required" → Thinker or PRACX.**
5. **Game launcher (§6 of the draft), `/api/launch`, the config file and `wine reg add` removed** as out of scope.
6. **The `open` crate is banned on Windows** because it spawns a hidden PowerShell.
7. **The Linux `.desktop` file / launcher script removed.** A `.tar.gz` keeps the executable bit.
8. **macOS is in scope** as an aarch64 CI build plus docs, labelled untested. The draft excluded all macOS work.
9. **"Bump the protocol string with wire changes" replaced** by the semver rule and versioned ALPN in ADR-0002.
10. **A Quit button on every OS**, in addition to the console. Stop is in-process and gives a new Ticket.
11. **Five banners instead of four** (IPC version mismatch added), plus "not your game folder".
12. **Acceptance criteria rewritten** to match. CI must also produce the macOS archive.
13. **The Windows target is decided:** `x86_64-pc-windows-gnu` (ADR-0002), no longer the agent's choice.
14. **Standalone project, not a fork** (ADR-0003). The binary is now `datalink-mp`, and upstream compatibility is no longer a constraint.
