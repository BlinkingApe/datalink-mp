# Research: SmartScreen / antivirus risk for an unsigned smac-helper.exe + dplayx.dll release

- **Date:** 2026-09-29
- **Question:** For v1, the plan is an unsigned `smac-helper.exe` and `dplayx.dll`, shipped together in a .zip that users extract into the game folder. How likely are SmartScreen warnings and AV false positives? Does the system-named, memory-patching DLL make this worse? What free or cheap mitigations exist? Does any of this favour a particular frontend (web UI, egui or Tauri)? Context: [ADR-0001](../adr/0001-web-ui-frontend.md).
- **Method:** Primary sources wherever they exist: Microsoft Learn docs, SignPath Foundation's published terms, and the issue trackers and release notes of comparable projects (DXVK, cnc-ddraw, Thinker, dgVoodoo, Tauri, rust-lang). Claims from community forums are labelled as such.

## TL;DR / verdict

**Viable, but only with some conditions.** An unsigned v1 is workable for this audience: a small, technical community of old-game players who already install unsigned DLL mods such as PRACX, Thinker and cnc-ddraw. Three things need to be true:

1. **The SmartScreen "Windows protected your PC" prompt is certain, on every release.** It is not a risk to manage. Unsigned files start at zero reputation on every new version, and it takes "several weeks and hundreds of clean installs" to clear ([MS SmartScreen reputation][ss]). A project this size will probably never clear it. Handle it with documentation.
2. **Some Defender ML false positives (`Wacatac.*!ml`-style) and occasional third-party detections are likely at some point.** The 32-bit MinGW-built `dplayx.dll` is the component most at risk, not the helper. DXVK, whose build is closest to ours (32-bit MinGW, system-named DLL replacements), had Defender flag its 32-bit DLLs twice in 2026 ([DXVK #5762][dxvk5762], [v3.0.2][dxvk302] and [v3.1.1][dxvk311] notes). Budget time to submit each release to Microsoft's WDSI portal.
3. **Smart App Control (SAC) is the one real blocker.** On Windows 11 machines with SAC in enforcement mode, "unknown, unsigned code [is] blocked by default" and there is no "Run anyway" ([MS SAC overview][sac]). SAC's signature checks "apply to all executable files", so this could also stop the game from loading the unsigned `dplayx.dll`. That part is inferred from the doc wording and not tested. Only signing (or earned reputation) fixes this. SAC is only on for clean installs, and Windows turns it off automatically for many users, so the affected share is unknown but not zero.

**Recommendation:** ship v1 unsigned as decided, with the documentation and process mitigations below. In parallel, apply to **SignPath Foundation** (free, no geography limits) once the fork has a public release history. **Azure Artifact Signing** ($9.99/month) is the fallback, but only if the maintainer is an individual in the US or Canada, or has an organisation in one of the listed countries. The frontend choice barely matters for AV risk. Tauri is slightly worse. The web UI and egui are about even. There is one concrete implementation note for the web UI: **don't open the browser with the `open` crate**. Its current 5.4.x release spawns a hidden `powershell.exe -Command …`. Call `ShellExecuteW` directly, or use `webbrowser` (see §2b).

## 1. SmartScreen: how it works and what to expect

From Microsoft's developer doc [SmartScreen reputation for Windows app developers][ss] (updated 2026-08):

- SmartScreen checks two signals: **publisher reputation** (the signing certificate) and **file hash reputation**.
- "When a file is not signed, SmartScreen reputation must build for each new version of your files, starting with zero reputation. Reputation cannot transfer from previous versions unless both were signed using the same publisher identity."
- For an unsigned file: "Warning — 'Windows protected your PC'; User must choose 'Run anyway'… Enterprise policy can prevent continuation entirely." Self-signed certificates behave the same as unsigned.
- An OV-signed file (and even an EV-signed one) still shows the warning at first, "until reputation accumulates", but the verified publisher name is displayed. "EV certificates no longer bypass SmartScreen."
- "There is no exact threshold, but it can take several weeks and hundreds of clean installs from a wide audience."
- "There is no need (or mechanism) to manually submit a file for SmartScreen reputation review for consumer endpoints."
- **Smart App Control:** "On Windows 11 devices, the Smart App Control feature may supersede SmartScreen Application Reputation. Smart App Control will block execution of unsigned files unless the file has a positive reputation. Smart App Control signature checks apply to all executable files, not just those downloaded from the Internet."

From the [Smart App Control overview][sac]:

- "Malware, Potentially Unwanted Apps (PUA), and unknown, unsigned code are blocked by default."
- If Microsoft's app-intelligence service cannot make a prediction, SAC "will still allow an app to run if it is signed with a certificate issued by a certificate authority (CA) within the Trusted Root Program." **Signing alone, with no reputation yet, is enough to get past SAC.** This is the strongest argument for signing.
- SAC can only be on after a clean Windows 11 install (or a reset). In most cases it turns on automatically, and "if we detect that you're one of those users [developers, etc.], we automatically turn Smart App Control off."

**What this means for smac-iroh:**

- Mark-of-the-Web: a .zip downloaded from GitHub and extracted with Explorer passes the internet-zone mark on to the extracted files. That is why SmartScreen fires when the user double-clicks `smac-helper.exe`. This is standard Windows behaviour and we did not look for a primary source for it. The DLL is loaded by the game, not launched by the user, so SmartScreen's prompt does not apply to it. Defender AV and SAC still do.
- Every release is a new hash, so every release starts at zero. Frequent releases, which the protocol-version coupling encourages, keep users seeing the prompt permanently.
- **The ADR's embed-and-install design was rightly dropped.** An unsigned exe that writes a DLL next to another program's exe matches the DLL-sideloading dropper pattern. The zip-extract flow avoids that behaviour entirely.

## 2. Antivirus false positives

### 2a. Unsigned Rust binaries in general

- Defender's machine-learning detections (suffix `!ml`, most often `Trojan:Win32/Wacatac.*!ml` or `Trojan:Script/Wacatac.B!ml`) are a long-running, recurring problem for small unsigned Rust executables. Evidence:
  - [rust-lang/rust#88297][rust88297]: a hello-world built with `stable-x86_64-pc-windows-gnu` flagged as `Trojan:Script/Wacatac.B!ml` (2021). It shows up while the file is in `Downloads`.
  - [rust-lang/rust#147547][rust147547] (closed 2025-10): a release-mode **hello-world** got 4/~70 detections on VirusTotal, including Defender `Trojan:Script/Wacatac.B!ml`. Rust maintainers closed it: "virus checkers can be very quirky for new binaries" and "users of AV products should be making reports of incorrect detections to *their* AV vendors" (ChrisDenton, nagisa). The upstream position is that the toolchain won't be changed to avoid AV, so fixing false positives falls to each project.
  - [rust-lang/rust#36277][rust36277]: "Windows Defender goes nuts when running rustdoc on some projects" (open since 2016).
  - [rust-lang/rust-analyzer#15278][ra15278]: `rust-analyzer-proc-macro-srv.exe` flagged as a trojan.
  - [tauri-apps/tauri#2486][tauri2486]: "[Windows] Trojan alert from windows defender and other anti-virus providers", open since 2021 with 84 comments. Related reports: [#4749][tauri4749] (MSI and .exe false positives), [#10302][tauri10302] (Kaspersky), [#7829][tauri7829] (Norton).
- The pattern across these reports is unsigned, rare and newly compiled. Microsoft does not publish the features its ML models weigh. We found no primary source for them, so anything more specific (for example "Rust panics look like X") is folklore.
- **The GNU toolchain is a risk factor.** Both rust#88297 and DXVK's 2026 flags involve MinGW builds. The DXVK maintainer found that "even the 2.7.1 32-bit binaries got flagged when building with an updated MinGW toolchain" ([DXVK #5762][dxvk5762]). The DLL *must* be `i686-pc-windows-gnu` here (see `.cargo/config.toml`). The helper does not have to be: the ADR leaves `-msvc` vs `-gnu` open. Building the helper with `x86_64-pc-windows-msvc` on a Windows CI runner is cheap insurance. No primary source proves MSVC builds get fewer flags, so this is only an inference from where the reports cluster.

### 2b. Behaviours specific to smac-helper

We found no primary evidence that any of these, done by an ordinary user-mode process, triggers detections by itself. Together they add to the behavioural profile of an unknown binary:

| Behaviour | Assessment |
|---|---|
| Binds a TCP listener on `127.0.0.1` | Low. Many apps do this, and a loopback-only listener does not trigger a Windows Firewall prompt (general knowledge; no primary source retrieved). |
| Opens the default browser | Low **if done without a shell intermediary**. Note that the `open` crate the ADR suggests does *not* do this. In `open` ≤ 5.3 it spawns `cmd` ([v5.3.2 windows.rs][open532]). In 5.4.x, current as of 2026-09, it first spawns a **hidden `powershell.exe -NoProfile -NonInteractive -Command <inline script>`**, then `explorer.exe` ([v5.4.4 windows.rs][open544]). An unknown unsigned binary spawning hidden PowerShell with an inline command is a classic behavioural-detection pattern. **Recommendation:** call `ShellExecuteW(NULL, "open", url, …)` directly via `windows-sys` (`Win32_UI_Shell`, about 10 lines), or use the `webbrowser` crate. On Windows, `webbrowser` resolves the handler with `AssocQueryStringW` and launches the browser exe directly ([webbrowser windows.rs][webbrowser]). Avoid `cmd /c start` too. |
| Binds UDP (QUIC) for inbound/outbound peer traffic | Causes the expected Windows Defender Firewall "allow access" prompt. This is a firewall UX step, not an AV detection. It is the same for every frontend because it comes from the iroh endpoint. |
| Connects to iroh relay/discovery servers | Low. These are ordinary HTTPS/QUIC destinations. Unsigned P2P tools do get flagged by some third-party AVs as "RiskWare/NetTool"-style PUA. We did not find this documented for iroh specifically ([n0-computer/iroh issue search][irohsearch] shows no AV reports, only firewall ones). |
| Console window stays visible | Slightly helpful. Hidden-window background processes look more like malware. |

### 2c. The DLL: a system-named replacement that patches game memory

Facts from this repo: `crates/smac-fixes` patches the game's **own address space** only. It calls `VirtualProtect` → writes code → restores protection, and uses `VirtualAlloc` with `PAGE_EXECUTE_READWRITE` for trampolines (`blit_fix.rs`, `border_fix.rs`, `classic_colors.rs`, `probe.rs`). It does **not** use `WriteProcessMemory`, `CreateRemoteThread` or cross-process injection. That puts it in the same class as Thinker and cnc-ddraw, and in a milder class than trainers and injectors. Still, RWX allocation plus code patching inside a DLL that carries a system DLL name is exactly what heuristic engines look for.

How comparable projects have fared:

| Project | What it is | AV history (primary source) |
|---|---|---|
| **DXVK** | MinGW-built replacement `d3d9/d3d11/dxgi.dll` | 2026: ClamAV `Win.Trojan.Virut-53` on the 32-bit `d3d11.dll`, bisected to a harmless code change. Later, **Windows Defender started flagging the 32-bit DLLs "out of nowhere"**. The maintainer temporarily **took all 3.x releases down** and restored them later. Users' WDSI submissions came back "not malware". Release notes for [v3.0.2][dxvk302] and [v3.1.1][dxvk311] carry AV warnings. ([#5762][dxvk5762], [#5893][dxvk5893]) |
| **cnc-ddraw** | Replacement `ddraw.dll` plus proxy DLLs | [v7.1.0.0][cncddraw] (2024-12): "Updated all proxy dlls to the latest petool version to avoid AV false positives", so the fix was changing the build tooling. |
| **Thinker** (SMAC mod, same game) | `thinker.dll`, which patches the SMAC binary in memory at load | Maintainer: "Antivirus heuristics become really nervous when they see some program patching binaries in memory, usually the detection is like HEUR/Malware or something generic. Not sure if there's a way to get completely rid of them other than maybe providing signed binaries, but that's a hassle." BitDefender flagged older `thinker.dll` builds ([discussion #25][thinker25]). The maintainer also suggested Defender heuristics might interfere with unsigned binaries at launch ([#38][thinker38]). |
| **dgVoodoo2** | Replacement DirectX DLLs plus control-panel exe | Community-reported (forum, not first-party): `dgVoodooCpl.exe` 2.86.2 flagged by Defender as `Trojan:Win32/VBClone!rfn` in July 2025. A user's WDSI submission had not resolved it two months later ([VOGONS thread][vogons107461]). An earlier episode was resolved after the developer reported it to Microsoft ([VOGONS t=63255][vogons63255]). |

**Takeaways:**

- Detections for these projects come and go with signature and model updates. They often reappear with **no code change** (for example, a toolchain bump), and they are not proof of anything. Expect the same here.
- The DLL is the component most likely to be flagged: it is 32-bit, built with MinGW, carries a system DLL name, and allocates RWX memory. When that happens, the symptom is bad: Defender quarantines `dplayx.dll` from the game folder, the game falls back to the system DirectPlay, or crashes, as in DXVK #5893, and the user sees "multiplayer doesn't work" rather than an AV prompt. **The helper UI should detect "DLL missing or not the expected hash" and tell the user it may have been quarantined.** The setup checks in ADR §5 already compare hashes, and they should word the failure this way.
- The game folder is often under `C:\Program Files (x86)` (GOG/Steam defaults). Extracting there needs elevation, which is a separate UX problem.

## 3. Mitigations for an open-source project

### 3a. Code signing: SignPath Foundation (free)

From SignPath Foundation's [conditions page][sp-terms]:

- **Eligibility:** OSI-approved license with no commercial dual-licensing (MIT OR Apache-2.0 is fine). No proprietary components. No malware or PUA. The project must be **actively maintained** and **already released** in the form to be signed. Its functionality must be documented on the download page.
- **Publisher shown to users:** the certificate is issued to **"SignPath Foundation"**, not to the project. SmartScreen and UAC will show "SignPath Foundation".
- **Reputation gate:** "For executable programs that may be downloaded and executed based on our signature, we require a certain verifiable reputation." They are "under no obligation to accept your project". A young fork with few users may be declined or asked to wait.
- **Forks:** allowed if "your project visibly uses a fork of the upstream project" and you do code review on upstream changes. This applies here (fork of `hdevalence/smac-iroh`).
- **Process obligations:** MFA for all team members on SignPath and GitHub. Named Author, Reviewer and Approver roles. **Manual approval of every signing request.** Binaries must be built from source in a verifiable CI build (GitHub Actions integration). A "Code signing policy" section on the homepage and release page, with the attribution text and a privacy statement. Product name and version metadata set on every signed binary. That means adding a Windows VERSIONINFO resource to both the exe and the DLL, for example via `winres` or `embed-resource`. MinGW handles this for the DLL too.
- **No hacking tools:** software must not "circumvent security measures of their execution environment". In-process patching of a 1999 game's own code to fix bugs is not that. Worth describing it plainly in the application.
- **Signing the DLL:** both artifacts are built from this repo, so both can be signed. Signing the DLL is what helps with SAC and heuristic AV on the loaded module.
- The application form itself ([signpath.org/apply][sp-apply]) is a JS-rendered form. We could not retrieve its fields or expected turnaround here.

### 3b. Code signing: Azure Artifact Signing (formerly Trusted Signing)

From Microsoft Learn ([quickstart][as-qs], [FAQ][as-faq], [SmartScreen doc][ss]):

- **Cost:** "Starts at $9.99/month" (Basic SKU: 5,000 signatures/month). Needs a **paid** Azure subscription. Free, trial and sponsored subscriptions are rejected. Billing is not pro-rated.
- **Region limits (the key constraint):** "Public Trust certificates are available to organizations in the United States, Canada, the European Union, the United Kingdom, Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway, and Israel. **Individual developers must be located in the United States or Canada.**"
- **Individual validation:** government ID via AU10TIX and Microsoft Authenticator Verified ID. The Azure billing account must be of type Individual, with legal name and address matching the ID. The **certificate CN is your legal name** (no custom CN/O), and city/state/country appear on the certificate, which is a privacy consideration. Organization validation takes 1–20 business days.
- **SmartScreen:** no instant reputation. It "builds up automatically". Artifact Signing issues short-lived certificates, but reputation attaches to the validated identity, so it carries across releases, unlike unsigned builds. It does satisfy SAC's "signed by a Trusted Root Program CA" allowance.
- CI: GitHub Actions integration is supported ([signing integrations][as-int]).

**Paid OV/EV certificates** from commercial CAs cost more (hundreds of USD per year and up) and now need hardware tokens or cloud HSMs. EV no longer gives SmartScreen any advantage ([ss]). They are not worth it for this project.

### 3c. False-positive submission (free, reactive)

- Microsoft: submit at <https://www.microsoft.com/wdsi/filesubmission> **as "Software developer"**. A Microsoft account sign-in is required. "Your submission is immediately scanned… before an analyst starts handling your case". Prevalent files and enterprise (SAID) submitters get priority, so a niche mod gets low priority. Use "rescan" to check for updated determinations. You can dispute through the developer contact form in the results ([submission guide][wdsi-guide]). Microsoft states this is for AV detections, **not** SmartScreen reputation ([ss]).
- Other vendors run their own portals (for example ClamAV at <https://www.clamav.net/reports/fp>, used in DXVK #5762). A community member reports BitDefender cleared a wined3d FP in minutes (DXVK #5762 thread). That is anecdotal.
- **Process suggestion:** as part of each release, upload the release zip's `smac-helper.exe` and `dplayx.dll` to VirusTotal (below). If Defender or a major vendor flags them, file a WDSI "software developer" submission before announcing the release.

### 3d. VirusTotal pre-checks

- Scanning release artifacts on VirusTotal before publishing shows most vendor detections at once. Note that files uploaded to VirusTotal are shared with its security-partner community (standard VT terms; not re-verified here). For open-source binaries that is harmless, and it arguably helps, because vendors see the file early.
- Linking the VT report for each release hash in the release notes is a common trust signal. DXVK does this ([v3.0.2 notes][dxvk302]).

### 3e. Checksums, provenance and reproducible builds

- These do **not** affect SmartScreen or AV verdicts. Their value is user trust and letting a sceptical user or moderator verify a binary came from the tagged source.
- SHA-256 checksums are already planned (ADR §7).
- **GitHub artifact attestations** (`actions/attest-build-provenance`) are free for public repos. They let anyone verify with `gh attestation verify` that a binary was built by this repo's workflow at a given commit. This is cheap and a stronger provenance claim than checksums. It also lines up with SignPath's "built from source in a verifiable way" requirement.
- Bit-for-bit reproducible builds are possible for Rust (fixed toolchain via `rust-toolchain.toml`, `--remap-path-prefix`, and deterministic PE timestamps, e.g. `-Wl,--no-insert-timestamp` for MinGW). This is nice to have, not needed for v1.

### 3f. Documentation (free, necessary)

- Release page and README: a screenshot of the SmartScreen dialog with "More info → Run anyway", a note that the Windows Firewall prompt is expected, and how to restore a quarantined `dplayx.dll` and add a Defender exclusion for the game folder, with the risk explained. Link to the VT report and the checksums.
- Microsoft itself recommends this: "Communicate with early adopters… they may see a SmartScreen prompt on first download" ([ss]).

### 3g. Build hygiene that may reduce ML hits (low confidence)

- Add VERSIONINFO resources (company, product name, description, version) to the exe and the DLL. Unknown, metadata-less PEs are a common ML feature. SignPath requires this anyway.
- Consider an MSVC-target helper (see 2a).
- Avoid UPX or any packer. None is planned. Keep `strip = true` / `opt-level = "z"`: there is no evidence either matters, so don't churn on them.
- None of this is backed by a primary source that states model features. Treat these as cheap experiments, checked against VirusTotal.

## 4. Does this favour a frontend choice?

**Mostly no. The risk comes from being unsigned and from the DLL, not from the UI technology.** All three options ship an unsigned `smac-helper.exe` with the same iroh networking, so SmartScreen, SAC, the firewall prompt and signing options apply equally. The differences:

| Factor | Web UI (ADR) | egui/eframe | Tauri |
|---|---|---|---|
| SmartScreen / SAC | Same (unsigned exe) | Same | Same, unless the NSIS/MSI installer route is used, which adds a second unsigned artifact |
| Documented AV FP history of the framework | None specific | None found (egui tracker search for antivirus/defender/trojan: 0 results) | **Long-running open issue** ([#2486][tauri2486], since 2021) plus several vendor-specific reports |
| Extra behaviours | Loopback HTTP listener; launches browser (use ShellExecute, not `cmd`) | GPU/window only; no listener or child process | Needs WebView2. If it is missing, the bootstrapper downloads and installs it, which looks like dropper behaviour. Tauri apps usually ship as installers |
| Binary size / surface | Smallest | Larger (wgpu/glow) | Medium, plus WebView2 dependency |

**Verdict for the frontend decision:** the web UI and egui are roughly equal on AV risk. The web UI adds a local listener and a browser launch, egui adds a GPU stack, and neither has a documented problem. **Tauri is slightly worse** because of its documented false-positive history and its WebView2 install path. This supports the ADR's rejection of Tauri, but it should not decide web UI vs egui.

## 5. Open items / not verified

- Whether SAC in enforcement mode blocks an **unsigned DLL loaded by an already-allowed (signed or reputable) game exe**. The doc says SAC checks "all executable files", but this was not tested. Worth testing on a clean Windows 11 VM with SAC on, because SAC is the one case with no user override. Note that the GOG `terran_PRACX.exe` / PRACX files are probably unsigned too, which would make the SAC question moot for affected users regardless of our signing.
- SignPath Foundation's application form, turnaround, and whether they would accept a young fork (see the reputation clause).
- Actual detection rates for our binaries. Upload a current CI build to VirusTotal to get a baseline before v1.

[ss]: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
[sac]: https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview
[as-qs]: https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart
[as-faq]: https://learn.microsoft.com/en-us/azure/artifact-signing/faq
[as-int]: https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations
[wdsi-guide]: https://learn.microsoft.com/en-us/defender-xdr/submission-guide
[sp-terms]: https://signpath.org/terms
[sp-apply]: https://signpath.org/apply
[dxvk5762]: https://github.com/doitsujin/dxvk/issues/5762
[dxvk5893]: https://github.com/doitsujin/dxvk/issues/5893
[dxvk302]: https://github.com/doitsujin/dxvk/releases/tag/v3.0.2
[dxvk311]: https://github.com/doitsujin/dxvk/releases/tag/v3.1.1
[cncddraw]: https://github.com/FunkyFr3sh/cnc-ddraw/releases/tag/v7.1.0.0
[thinker25]: https://github.com/induktio/thinker/discussions/25
[thinker38]: https://github.com/induktio/thinker/issues/38
[vogons107461]: https://www.vogons.org/viewtopic.php?t=107461
[vogons63255]: https://www.vogons.org/viewtopic.php?t=63255
[rust88297]: https://github.com/rust-lang/rust/issues/88297
[rust147547]: https://github.com/rust-lang/rust/issues/147547
[rust36277]: https://github.com/rust-lang/rust/issues/36277
[ra15278]: https://github.com/rust-lang/rust-analyzer/issues/15278
[tauri2486]: https://github.com/tauri-apps/tauri/issues/2486
[tauri4749]: https://github.com/tauri-apps/tauri/issues/4749
[tauri10302]: https://github.com/tauri-apps/tauri/issues/10302
[tauri7829]: https://github.com/tauri-apps/tauri/issues/7829
[open532]: https://github.com/Byron/open-rs/blob/v5.3.2/src/windows.rs
[open544]: https://github.com/Byron/open-rs/blob/v5.4.4/src/windows.rs
[webbrowser]: https://github.com/amodm/webbrowser-rs/blob/main/src/windows.rs
[irohsearch]: https://github.com/n0-computer/iroh/issues?q=antivirus+OR+defender+OR+firewall
