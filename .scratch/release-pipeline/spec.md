# Spec: the release pipeline for datalink-mp v1

Status: ready-for-agent

Source decisions: [ADR-0002](../../docs/adr/0002-releases-versioning-pipeline-trust.md) sections 2 (Pipeline), 3 (Trust posture — the per-release checklist items only) and 4 (Release gate for 0.1.0). Decision trail: [the web UI v1 map](../archive/web-ui-v1/map.md). Vocabulary: `CONTEXT.md`.

Where this spec and ADR-0002 disagree, the ADR wins, except for the items listed under "Decisions this spec adds", which the ADR left to the spec.

## Problem Statement

Nothing today produces a release a player could actually receive. Every artifact that exists is a local, ad hoc cross-build: the DLL's link step fails on current mingw toolchains without an undocumented workaround, no Windows build of the Helper is ever persisted, neither Windows binary carries a version a player could check without starting the app, and no tagged version's bytes have ever been verified end to end before being handed to anyone. `datalink-mp` has no CI at all.

This isn't abstract: [ticket 17](../archive/helper-web-ui/issues/17-manual-check-of-the-page.md) of the web UI spec is blocked on exactly this gap — there's no Windows archive to run the manual check against.

## Solution

A maintainer pushes a `vX.Y.Z` tag. GitHub Actions builds and uploads all three platform archives (Windows `.zip`, Linux `.tar.gz`, macOS `.zip`) sharing one DLL, stamps the Release version into both Windows binaries, publishes `SHA256SUMS` and attestations, and opens a draft GitHub Release with everything attached — never auto-publishing. For `0.1.0`, the maintainer pushes an `-rc.N` tag first, runs the smoke test and a real cross-platform game against exactly those CI-built bytes, and only then tags and publishes the real release by hand.

## User Stories

1. As a maintainer, I want pushing a `v*` tag to build the DLL, Windows Helper and Linux Helper without any local cross-compilation, so that I never hand a player bytes I built by hand on my own machine.
2. As a maintainer, I want the DLL build to succeed even if the `_Unwind_Resume` clash reappears on CI's own toolchain, so that a mingw version bump doesn't silently break every release.
3. As a maintainer, I want CI to fail if the pushed tag doesn't match the workspace version, so that I can't accidentally ship a mislabeled release.
4. As a maintainer, I want the Linux Helper built as a static musl binary, so that a player on an older-glibc distro or SteamOS can run it.
5. As a maintainer, I want one DLL built once and reused across all three archives, so that there's a single tested DLL and a single published hash.
6. As a maintainer, I want the macOS build ad-hoc re-signed and `codesign --verify`'d, so that Gatekeeper's basic checks pass even though the app is unsigned.
7. As a maintainer, I want every release's archives covered by `SHA256SUMS` and GitHub artifact attestations, so a player (or I) can verify a download matches what CI built.
8. As a maintainer, I want the exe and DLL to carry VERSIONINFO, so a player can read the Release version from Windows file properties without starting the app.
9. As a maintainer, I want CI to open a draft release rather than publish automatically, so a human always makes the final publish decision.
10. As a maintainer, I want `-rc.N` tags marked pre-release, so release candidates don't look like finished releases on the Releases page.
11. As a maintainer, I want to test the exact CI-built archives from an RC tag — never a local build — before publishing `0.1.0`, so the tested bytes are the ones that ship.
12. As a maintainer, I want a Windows smoke test (SmartScreen → Run anyway → UI opens → Ticket shown) in the gate, so the first thing a player sees is confirmed working.
13. As a maintainer, I want one real multiplayer game between Linux (Faugus, musl archive) and real Windows in the gate, so the core promise is proven end to end before release.
14. As a maintainer, I want a VirusTotal scan of the exe and DLL linked in the release notes, so a flagged binary is caught and documented before a player downloads it.
15. As a maintainer, I want a WDSI submission path ready if Defender flags a release, so a false positive doesn't block players indefinitely.
16. As the agent working ticket 17 of the web UI spec, I want a CI-built Windows archive to exist, so I have real bytes to run the manual check against.
17. As a contributor who wants to offer the `unwind_stubs.c` fix to upstream `smac-iroh`, I want its removal to be one small, self-contained commit, so it can be offered as a courtesy without dragging in unrelated pipeline changes.

## Implementation Decisions

- **Workflow**: a new, additive `.github/workflows/release.yml`, triggered on push of any `v*` tag, including `-rc.N` tags. No existing CI to touch — there is none.
- **Version check**: the job fails immediately if the tag (`v` stripped) doesn't equal the workspace version (for `-rc.N` tags, see "Decisions this spec adds").
- **One Ubuntu job** (`ubuntu-24.04`, `gcc-mingw-w64-i686 gcc-mingw-w64-x86-64`, musl target tooling, rustup targets `i686-pc-windows-gnu`, `x86_64-pc-windows-gnu`, `x86_64-unknown-linux-musl`) builds:
  - **`dplayx.dll`**: the plain `i686-pc-windows-gnu` build first (the fix deletes `unwind_stubs.c`'s `cc::Build` step, so this should link cleanly). If CI's mingw still clashes, retry with `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"` — never bare `RUSTFLAGS`, which replaces rather than joins `.cargo/config.toml`'s `control-flow-guard=no` / `-lws2_32`.
  - the Windows Helper (`x86_64-pc-windows-gnu`), zipped with the DLL, README and licences.
  - the Linux Helper as a static `x86_64-unknown-linux-musl` binary, `.tar.gz`'d with the executable bit kept.
- **A macOS job** (`macos-15`, arm64, not the billed `-large`/`-xlarge` runners) builds `aarch64-apple-darwin`, `codesign --verify`s (ad-hoc re-signing on failure), takes the DLL as an artifact from the Ubuntu job, and zips with `ditto -c -k` (plain `zip`/`upload-artifact` loses properties `ditto` preserves).
- **Every release**: `SHA256SUMS` over all three archives, GitHub artifact attestations (`actions/attest-build-provenance`), a draft GitHub Release with everything attached, `-rc.N` tags flagged pre-release. CI never publishes — a maintainer always does, by hand, after the gate.
- **VERSIONINFO** on both Windows binaries: the `embed-resource` crate — actively maintained, and drives `windres`, which the Ubuntu job's mingw install already provides, so cross-compiling needs no tool beyond what the DLL build already needs.
- **Fallback, if Ubuntu's mingw proves unworkable**: a Fedora container matching the dev host's toolchain, or the Windows Helper alone on `windows-latest` as `-msvc` with `+crt-static` (DLL stays on Ubuntu either way).
- Local cross-builds stay experiment-only, as `docs/building.md` already documents; the bytes that ship always come from CI.

### Decisions this spec adds

- **Ordering**: VERSIONINFO lands first — self-contained, unblocked, touches both crates' `build.rs` — so the Ubuntu job's binaries carry it from the start instead of needing a revisit. The Ubuntu job then blocks the macOS job (which needs its DLL artifact and the workflow skeleton), which blocks the release gate (which needs real CI-built RC archives).
- The `unwind_stubs.c` fix is **not** a separate gating ticket. The Ubuntu job ticket owns the fix attempt and its inline fallback together, exactly as ADR-0002 designs it ("this holds only if the first CI run links cleanly"). Offering the fix upstream to `smac-iroh` as its own commit can be split out later; it doesn't block anything here.
- **VERSIONINFO tool**: `embed-resource` over `winres` (unmaintained) or `winresource` — maintained, and built around clean cross-compilation via the host's `windres`.
- **RC tags pass the version check**: the check strips `v` and a trailing `-rc.N` before comparing, so `v0.1.0-rc.1` matches workspace version `0.1.0` and the RC binaries already report `0.1.0`. Any other suffix fails the check.
- **The RC's bytes are the ones published.** Builds aren't reproducible, so the `v0.1.0` tag's own CI run produces different, untested bytes. After the gate, the maintainer replaces that draft's archives and `SHA256SUMS` with the gated RC's, then publishes. The RC's attestations stay valid because they're bound to the archives' digests.
- **Verification tags**: the agent working the CI tickets may push and delete `-rc.N` tags and a deliberately mismatched tag to prove the pipeline, and deletes the draft releases those runs leave behind.
- **`docs/releasing.md`** (new, mirroring `docs/building.md`'s shape): where the release-gate checklist and the per-release trust checklist (VirusTotal, WDSI) get recorded as they're run for `0.1.0` — the gotchas no config confesses (RC tag naming, where attestations show up, how to re-run WDSI), not a restatement of `release.yml` itself.

## Testing Decisions

- **Seam 1 (primary, new)**: a pushed `v*`/`-rc.N` tag, observed through the real GitHub Actions run. There's no local CI emulator in this repo, and ADR-0002 designs the gate this way already ("local builds don't count"). The Ubuntu- and macOS-job work is verified by pushing a real `-rc.N` tag and inspecting the result: artifact presence, the version-mismatch check actually failing on a wrong tag, `SHA256SUMS`/attestations attached to the draft release.
- **Seam 2 (existing, faster feedback)**: the local `cargo build` commands in `docs/building.md`, used to prove the DLL fix-or-fallback and the VERSIONINFO output before trusting them to a CI run — inspected with `objdump -p ... | grep 'DLL Name'` / `file`, the way `docs/research/windows-toolchain.md` already did.
- The release gate itself has no seam below "a person runs it": the Windows smoke test and the real cross-platform game are inherently manual, same as `.scratch/archive/helper-web-ui/issues/17-manual-check-of-the-page.md`.
- Not automated: the VirusTotal scan and any WDSI submission are manual, per-release maintainer steps, recorded in `docs/releasing.md`, not scripted.

## Out of Scope

- ADR-0002 section 1 (Versioning) — already implemented ([helper-web-ui ticket 02](../archive/helper-web-ui/issues/02-transport-versioned-alpn-and-dial-errors.md)).
- The player-facing trust documentation (SmartScreen, firewall, quarantine recovery, Gatekeeper, Smart App Control) — already written in `README.md`.
- Code signing, the SignPath Foundation application, a clean-VM Smart App Control test — ADR-0002 marks these as follow-ups after v1.
- `-msvc` as the primary Windows target — stays the documented fallback only.
- Offering the `unwind_stubs.c` removal to upstream `smac-iroh` — worth doing, but a separate, unblocked effort from this release pipeline.

## Further Notes

Completing this spec is what unblocks the Windows half of [ticket 17](../archive/helper-web-ui/issues/17-manual-check-of-the-page.md): the first CI-built `-rc.N` Windows archive is the first real bytes that ticket has to test against.
