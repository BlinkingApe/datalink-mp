# macOS aarch64 CI artifact and current Gatekeeper steps

Type: research
Status: resolved
Blocked by:

## Question

The user has no Mac. Establish how to ship an untested but runnable macOS build:

- Building `aarch64-apple-darwin` for `smac-helper` on GitHub Actions macOS runners (free for public repos): runner labels, any Iroh-specific issues.
- Where the 32-bit `dplayx.dll` comes from for the macOS archive: build it on the macOS runner with mingw-w64, or reuse the artifact from the Linux job?
- Current (2025-2026 macOS) Gatekeeper behaviour for an unsigned, quarantined CLI binary downloaded in a `.zip`: exact user steps (System Settings → Privacy & Security → Open Anyway?), and whether ad-hoc signing (`codesign -s -`) changes anything on Apple Silicon.
- What the macOS build instructions (for users building it themselves) need to say.
- Does the Helper's browser-open work on macOS (`open` command / the `open` crate)?

## Research

Findings: branch `research/macos-artifact`, file `docs/research/macos-artifact.md`.

## Comments

### Resolution (2026-09-29)

**Viable. One catch: double-clicking the Helper on a Mac always opens a Terminal window.** Full findings: branch `research/macos-artifact` (commit `ecb177e`), `docs/research/macos-artifact.md`. The user-facing steps are inferred from docs (untested); six items are listed there for the first Mac user to verify.

- **CI:** `runs-on: macos-15` (arm64; free on public repos; avoid `-large`/`-xlarge`, which are billed). iroh has no special issues (the lockfile uses `ring`). Run `codesign --verify` after the build and re-sign ad-hoc if it fails. Build the zip with `ditto -c -k`, because `upload-artifact` drops the executable bit.
- **DLL:** reuse the Linux job's `dplayx.dll` in all three archives: one tested DLL, one published hash. Builds aren't reproducible (link timestamp, absolute `.cargo` paths in panic strings).
- **Gatekeeper:** the right-click → Open bypass was removed in Sequoia. The path is: open → blocked → System Settings → Privacy & Security → Open Anyway (valid for about an hour) → password → open again. An ad-hoc signature is required on Apple Silicon (the linker adds it) but doesn't help with Gatekeeper.
- **Double-click → Terminal.app runs it.** The working directory is `$HOME`, so the Helper must find its own folder via `current_exe()` (applies on every OS). Closing the window quits it. Launching from Terminal also avoids macOS 15's local-network permission prompt, so **don't wrap it in a `.app`**.
- **Browser:** `/usr/bin/open` should work (untested); also print the URL.
- **Build-from-source steps:** `xcode-select --install`, rustup plus the i686 target, Homebrew `mingw-w64`, the two `cargo build` commands; Apple Silicon only. Self-built binaries aren't quarantined.
- **Beyond the question:** Whisky was archived on 2025-05-11, so the docs should say "any Wine on macOS (e.g. CrossOver)". Bottles live under the hidden `~/Library`, so point users to Finder's Go → Go to Folder (⇧⌘G).
