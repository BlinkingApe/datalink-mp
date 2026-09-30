# Map: Web UI v1 for datalink-mp

Label: wayfinder:map

## Destination

An **Accepted, revised ADR-0001** and a **decided release pipeline** (Windows x86_64, Linux x86_64, macOS aarch64) for `datalink-mp`, a standalone project seeded from `hdevalence/smac-iroh`, with nothing left to decide. The map then hands off to `/to-spec` → `/to-tickets` → `/implement`; building happens afterwards, as a separate effort.

## Notes

- Domain: Rust workspace; see `CONTEXT.md` (Helper, DLL, Ticket, IPC version, Peer protocol version, Release version) and `docs/adr/0001-web-ui-frontend.md` (the starting reference, to be revised, not followed blindly).
- Tracker: local markdown, this directory. Research findings live on throwaway `research/<name>` branches, as `docs/research/<name>.md`.
- Standing preferences:
  - Standalone, with upstream as a courtesy only: CLI `host`/`join` unchanged; generic fixes may be offered to smac-iroh; no design effort goes into upstream compatibility (see [Fork or standalone project?](issues/10-fork-or-standalone.md)).
  - Local cross-builds for experiments; CI for tagged releases.
  - README claims only what has been tested; everything else is "should work".
  - The user has no Mac: macOS is built in CI and labelled untested.
- Skills for grilling tickets: `grilling` + `domain-modeling`.

### v1 promise (the cut line)

A player with SMAC plus Thinker or PRACX extracts one archive into the game folder and double-clicks the Helper, then plays multiplayer with friends on Windows, Linux or Mac without typing any commands. Linux/Mac players paste the Wine override into their launcher themselves; Mac players also do a one-time Gatekeeper "Open Anyway", and leave the Terminal window that opens by itself.

### Decided while charting (feeds the ADR revision)

- **Launching and quitting**
  - Bare `datalink-mp` opens the UI (no documented usage relies on the old default of `host`).
  - The UI has a Quit button on every platform; the Windows console also stays.
- **Live status**
  - The page polls `GET /api/status` about once a second; no SSE, since `EventSource` can't carry the token header.
  - The status panel shows the Release version (with IPC and Peer protocol versions as small print), state, whether the DLL is connected, and the peer count with short IDs. No player names.
- **Error handling**
  - Five banners:
    - IPC version mismatch (re-extract the zip)
    - Peer protocol version mismatch (your friend has a different release)
    - Invalid ticket
    - Can't reach the host
    - IPC port in use
  - Stop is required. A self-restart (re-exec) fallback was accepted while charting, but research found it would orphan a running game's IPC connection, so it's in-process only (see Decisions so far).
- **Setup and packaging**
  - The Helper's own folder is the game folder. No auto-detection, no config file, no install endpoint, no `include_bytes!` DLL.
  - A startup self-check looks for `dplayx.dll` and at least one of `thinker.exe` / `terran_PRACX.exe`. Otherwise it shows the banner "not your game folder".
  - Release assets per OS are an archive containing the Helper, `dplayx.dll`, README and licences, plus `SHA256SUMS`:
    - Windows: `.zip`
    - Linux: `.tar.gz`, keeping the executable bit; the README says double-click → Run. No `.desktop` file or `run.sh`.
    - macOS: `.zip`
  - Linux setup text covers the generic `WINEDLLOVERRIDES` string, Faugus and Steam, with Copy buttons.

## Decisions so far

<!-- one line per closed ticket -->

- [SmartScreen/antivirus risk for an unsigned exe shipped next to a DLL](issues/04-av-smartscreen-risk.md): viable unsigned with documented warnings; Smart App Control is the one hard blocker; avoid the `open` crate on Windows; the frontend choice is unaffected.
- [macOS aarch64 CI artifact and current Gatekeeper steps](issues/03-macos-ci-artifact.md): `macos-15` runner, reuse the Linux-built DLL, zip with `ditto`; one-time Open Anyway in Settings; double-click runs it in a Terminal window (so no `.app` wrapper); Whisky is archived.
- [Can the transport be restarted in-process, and are the error states observable?](issues/01-transport-restart-and-observability.md): yes, in-process Stop (about 1–1.5 days) beats re-exec (which would orphan the game's IPC connection); all five banners are observable with small changes; axum on its own runtime, IPC loop on a thread, blocking calls in `spawn_blocking`.
- [Windows helper toolchain](issues/02-windows-toolchain.md): `x86_64-pc-windows-gnu`, cross-built locally (7.2 MB, no runtime DLLs); one Ubuntu mingw job builds the whole zip; the DLL build needs a fix for the `_Unwind_Resume` clash.
- [Test the Wine override without winecfg](issues/05-wine-override-test.md): the DLL in the folder isn't enough; `WINEDLLOVERRIDES=dplayx=n,b` is required and confirmed working in Faugus ("Game Arguments" field, unquoted) and the shell under GE-Proton11-7; Steam was inconclusive (Flatseal, then a C++ runtime error).
- [UI flow: Host/Join toggle or one screen?](issues/06-ui-flow-prototype.md): numbered steps (game folder → Ticket → start the game → play), taken as prototyped; no Host/Join toggle and no `POST /api/host`, since the Ticket exists from startup.
- [Versioning and compatibility policy](issues/08-versioning-and-compatibility.md): semver, where an IPC or Peer protocol version change means a minor bump; the versioned ALPN (now `datalink/<peer version>` from `0.1.0`, see Fork or standalone project?) rejects mismatched builds at connect and drives the banner; the Ticket isn't persisted and changes after Stop; the Ticket format is unchanged.
- [Release trust posture for v1](issues/09-release-trust-posture.md): ship unsigned with documented SmartScreen/Firewall/quarantine steps; SAC documented, not tested; delete the `unwind_stubs` build step (CI-only rustflags fallback); gate the first release on CI-built RC archives passing a Windows smoke test plus Linux ↔ Windows games (own machine, then a friend); VirusTotal, WDSI, attestations, VERSIONINFO and a "quarantined?" banner on every release.
- [Revise and accept ADR-0001](issues/07-revise-adr.md): ADR-0001 (web UI) revised and Accepted, with a list of changes from the draft; new ADR-0002 (versioning, pipeline, trust) Accepted; the promise now says "without typing any commands"; Steam gets an untested line, not a button; drafts for tagged releases; the Linux build is static musl.
- [Fork or standalone project?](issues/10-fork-or-standalone.md): standalone `datalink-mp` seeded from smac-iroh (history kept, credited in the README); ALPN `datalink/<peer version>` reset to 1; first release `0.1.0`; upstream is a courtesy, not a constraint ([ADR-0003](../../docs/adr/0003-standalone-project.md)).

## Not yet specified

None: the way is clear. Hand off to `/to-spec` → `/to-tickets`, from [ADR-0001](../../docs/adr/0001-web-ui-frontend.md), [ADR-0002](../../docs/adr/0002-releases-versioning-pipeline-trust.md) and [ADR-0003](../../docs/adr/0003-standalone-project.md).

<!-- Left for /to-spec, not map decisions: -->
- UI port default, fallback range, and the single-instance mechanism: implementation-level.
- README / quickstart wording per OS: the facts are in the Wine override and Gatekeeper tickets; the wording is left for /to-spec.

## Out of scope

- The Windows "Launch game" button: post-v1 by decision, and this map's destination is v1. A separate effort later.
- `wine reg add` into a user-pasted prefix: the copy-string covers it.
- A native folder picker, and game-folder auto-detection: made unnecessary by extracting into the game folder.
- SignPath Foundation signing application, and a Smart App Control test on a clean Windows 11 VM: follow-ups after v1 (see [Release trust posture for v1](issues/09-release-trust-posture.md)).
- Code signing, auto-update, tray icon, NAT/relay configuration UI.
- Intel Mac builds; Lutris-specific instructions; macOS work beyond CI and docs.
- SSE event stream.
- Wire-protocol or DirectPlay semantic changes beyond the version mechanism (the versioned ALPN is in scope; see [Versioning and compatibility policy](issues/08-versioning-and-compatibility.md)).
- A stable Ticket across Stop or relaunch (a persisted key would reopen "no config"; see [Versioning and compatibility policy](issues/08-versioning-and-compatibility.md)).
