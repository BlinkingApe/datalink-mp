# SmartScreen/antivirus risk for an unsigned exe shipped next to a DLL

Type: research
Status: resolved
Blocked by:

## Question

A viability check on the web-UI plan. v1 ships an unsigned `smac-helper.exe` alongside a `dplayx.dll` (which hooks into the game; see `crates/smac-fixes`) in a zip that users extract into their game folder.

- How likely are SmartScreen "unrecognized app" warnings and AV false positives (Defender and common third-party) for unsigned Rust binaries, and specifically for a Rust exe that runs a localhost HTTP server and opens the browser?
- Does shipping a replacement system-named DLL (`dplayx.dll`) raise flags?
- Free or cheap mitigations for an open-source project: SignPath Foundation OSS signing, Azure Trusted Signing (cost and eligibility), submitting to Microsoft for analysis, checksums and reproducible builds, VirusTotal pre-checks.
- Does anything here change the choice between a web UI and the alternatives in ADR-0001, or does it apply equally to all of them?

## Research

Findings: branch `research/av-risk`, file `docs/research/av-risk.md`.

## Comments

### Resolution (2026-09-29)

**Verdict: viable to ship v1 unsigned, with conditions.** Full findings: branch `research/av-risk` (commit `01d2739`), `docs/research/av-risk.md`.

- SmartScreen "unrecognized app" appears on **every** unsigned release (reputation resets per file). Treat it as documentation: a "More info → Run anyway" screenshot, plus the expected Firewall prompt.
- AV false positives are likely at some point, most likely on `dplayx.dll` (32-bit MinGW, system DLL name, executable-memory patching). DXVK (the closest analogue) was flagged by Defender in 2026 with no code change. Handle it by submitting to Microsoft WDSI as a developer, running VirusTotal before publishing, and having the self-check say "DLL missing, possibly quarantined by antivirus".
- **Smart App Control** (Windows 11, clean installs) blocks unsigned code with no bypass. It is the one real blocker; the affected share of users is unknown, and whether it also blocks the DLL is untested.
- Mitigations: SignPath Foundation (free for OSI projects; manual approval per release, CI builds, a published signing policy, and "some reputation" required, so a young fork may be told to wait). Azure Artifact Signing (from $9.99/month; individuals US/Canada only). EV certificates no longer bypass SmartScreen.
- Does not decide the frontend: web UI ≈ egui; Tauri slightly worse. The web-UI working hypothesis stands.
- **ADR fix:** don't use the `open` crate on Windows, because 5.4.x spawns hidden `powershell.exe`, a classic detection pattern. Call `ShellExecuteW` via `windows-sys`, or use the `webbrowser` crate.
- Add version metadata (VERSIONINFO) to both binaries.
