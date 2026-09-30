# ADR-0002: Releases: versioning, pipeline and trust

- **Status:** Accepted (2026-09-30)
- **Applies to:** `datalink-mp` ([ADR-0003](0003-standalone-project.md))
- **Audience:** implementing agent working in the repo
- **Related:** [ADR-0001](0001-web-ui-frontend.md), [ADR-0003](0003-standalone-project.md). Decision trail: `.scratch/web-ui-v1/map.md`. Research: branches `research/windows-toolchain`, `research/macos-artifact`, `research/av-risk`.

## Context

Players compare a single version number, but three things can be incompatible: the Release version, the DLL ↔ Helper handshake (IPC version) and the Helper ↔ Helper wire protocol (Peer protocol version). Before this decision:

- Mismatched peers first met as a postcard decode error. The version preamble only rode the first ordered message, the ALPN was a fixed `dplay-iroh/1`, and `protocol::PROTOCOL_VERSION` was dead code.
- The project ships unsigned binaries, including a system-named 32-bit DLL that patches game memory.
- The user has no Mac.

## Decision

### 1. Versioning

- **Semver with a compatibility rule.** Pre-1.0: a change to the IPC version or the Peer protocol version means a **minor** bump; anything else is a **patch**. Players on the same `0.x` can play together.
- The tag `vX.Y.Z` must equal the workspace version; CI fails otherwise.
- The Release version is the only number players compare. The IPC and Peer protocol versions are small print in the UI.
- **The ALPN carries the Peer protocol version:** `datalink/<STREAM_PROTO_VERSION>`, with the Peer protocol version reset to 1, so `datalink/1` at the first release (ADR-0003).
  - Mismatched builds are rejected in the QUIC handshake, and the joiner maps that rejection to the Peer protocol mismatch banner. The banner doesn't name the friend's release.
  - The stream preamble stays as a second line of defence.
  - Remove the dead `protocol::PROTOCOL_VERSION`.
- The new ALPN deliberately breaks compatibility with smac-iroh's `dplay-iroh/1` builds (ADR-0003). **The first release is `0.1.0`.**
- The Ticket format is unchanged, and carries no version.

### 2. Pipeline

A new, additive `release.yml`, triggered by pushing a `v*` tag:

- **One `ubuntu-24.04` job** (`gcc-mingw-w64-i686 gcc-mingw-w64-x86-64`, rustup targets `i686-pc-windows-gnu` and `x86_64-pc-windows-gnu`) builds:
  - `dplayx.dll` (`i686-pc-windows-gnu`, 32-bit). This is the **single DLL used in all three archives**, giving one tested DLL and one published hash.
  - The Windows Helper (`x86_64-pc-windows-gnu`, static libgcc/winpthread, no runtime DLLs) and the Windows `.zip`.
  - The Linux Helper as a **static `x86_64-unknown-linux-musl` binary (required)**, so it runs on any x86_64 distro regardless of glibc (older LTS, SteamOS), plus the `.tar.gz` with the executable bit kept.
- **A `macos-15` job** (arm64; not the billed `-large`/`-xlarge` runners):
  - Builds `aarch64-apple-darwin` and runs `codesign --verify`, re-signing ad-hoc if it fails.
  - Takes the DLL from the Ubuntu job, and builds the `.zip` with `ditto -c -k` (`upload-artifact` drops the executable bit).
- Every release gets `SHA256SUMS` and GitHub artifact attestations (`actions/attest-build-provenance`).
- VERSIONINFO resources on both the exe and the DLL.
- **The workflow creates a draft GitHub Release** with all of the above attached. Tags with `-rc.N` are marked pre-release. The maintainer publishes by hand after the gate (section 4). CI never publishes on its own.
- **DLL build fix:** delete the `unwind_stubs` build step (the `cc::Build` call and `unwind_stubs.c`) in `crates/dplayx/build.rs`, as a small self-contained commit (which may be offered to smac-iroh as a courtesy).
  - This holds only if the first CI run (mingw GCC 13.2) links cleanly.
  - If it doesn't, CI alone sets `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"`.
  - **Never bare `RUSTFLAGS`**: it replaces the `.cargo/config.toml` flags (`control-flow-guard=no`, `-lws2_32`).
- Local cross-builds (`-gnu` from Linux) are for experiments only; the bytes that ship come from CI.
- Fallbacks if Ubuntu's mingw fails: a Fedora container, or `-msvc` with `+crt-static` on `windows-latest` for the Helper only.

### 3. Trust posture

- **Ship v1 unsigned.** Document:
  - SmartScreen "More info → Run anyway", with a screenshot. It appears on every release.
  - The expected Windows Firewall prompt.
  - How to restore a quarantined `dplayx.dll`.
  - macOS Gatekeeper: open → blocked → System Settings → Privacy & Security → Open Anyway → password → open again.
- **Smart App Control** blocks unsigned code with no bypass. The README says so, and says the only workaround is turning SAC off. It isn't tested for v1.
- **Per release:**
  - A VirusTotal scan of the exe and DLL before publishing, with the report linked in the release notes.
  - A WDSI "software developer" submission if Defender flags anything.

### 4. Release gate for `0.1.0`

Test the **CI-built archives from an RC tag** (e.g. `v0.1.0-rc.1`), so the tested bytes are the ones that ship. Local builds don't count. Before publishing and announcing:

- A Windows smoke test: SmartScreen → Run anyway → the UI opens → a Ticket is shown.
- One real multiplayer game between Linux (Faugus, using the **musl** archive) and real Windows, both sides running the Helper and DLL from the archives:
  - first on the maintainer's own Windows machine or dual-boot
  - then with a Windows-using friend over the internet
- macOS is built in CI and labelled untested.

## Consequences

- Every release pops SmartScreen, and AV false positives (most likely on the DLL) are handled reactively through WDSI.
- Smart App Control users can't run v1.
- A draft-then-publish flow puts one manual step on every release, and that step is where the gate is enforced.
- musl's allocator is slower than glibc's, which is irrelevant at the Helper's traffic levels. The musl build has only been tested through the RC gate, not locally.
- Builds aren't reproducible (link timestamps, absolute `.cargo` paths in panic strings). Trust rests on attestations, checksums and the public source.

## Alternatives considered

- **Version inside the Ticket:** rejected. The ALPN rejection surfaces within about a second of Join, and the Ticket format stays unchanged.
- **glibc Linux build on `ubuntu-24.04`:** needs glibc ≥ 2.39 and excludes Debian 12, Ubuntu 22.04 and some SteamOS versions. Building on `ubuntu-22.04` only delays the problem until that runner is retired.
- **`-msvc` Windows Helper:** can't be cross-built from Linux, and no source shows a better AV reputation. It stays the fallback.
- **Building the DLL on the macOS runner:** it would give a second, different DLL hash for no benefit.
- **Signing now** (SignPath Foundation, Azure Artifact Signing): SignPath needs release history, and Azure is paid and limited to the US/Canada for individuals. Applying to SignPath and testing Smart App Control on a clean Windows 11 VM are follow-ups after v1.
- **CI auto-publishing on tag:** it would skip the RC gate and the VirusTotal step.
