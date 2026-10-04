# Release trust posture for v1

Type: grilling
Status: resolved
Blocked by:

## Question

What must be true of the binaries before v1 is published? Three linked decisions:

1. **Signing and Smart App Control.** Does v1 ship unsigned (with documented SmartScreen/Firewall steps), or wait for SignPath Foundation signing? Is a Smart App Control test on a clean Windows 11 VM a v1 gate or a post-v1 follow-up? See ticket 04.
2. **DLL build fix.** Delete the `unwind_stubs.c` build step from `crates/dplayx/build.rs` (clean, upstream-PR-worthy, tested in a scratch copy), or use the `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS` workaround? See ticket 02.
3. **Re-test gate.** The currently tested DLL was built with a `RUSTFLAGS` override that dropped `control-flow-guard=no` and `-lws2_32`. Is rebuilding the DLL properly and re-testing Linux ↔ Windows multiplayer a precondition for v1? Who tests what, and on which machines?

Best worked after the user has at least decided on (or tried) the DLL rebuild.

## Answer

Resolved 2026-09-30 (grilling).

1. **Signing: ship v1 unsigned.** Document SmartScreen "More info → Run anyway", the expected Windows Firewall prompt, and how to restore a quarantined `dplayx.dll`. Applying to SignPath Foundation is a follow-up after v1, once the fork has a public release history. Nothing waits on it.
2. **Smart App Control: not a v1 gate.** The README says that if Smart App Control is on, the Helper (and probably the DLL) won't run, and the only workaround is turning SAC off. Testing SAC on a clean Windows 11 VM is a follow-up after v1. It wouldn't change what ships, because only signing fixes SAC.
3. **DLL build fix: delete the `unwind_stubs` build step** (the `cc::Build` call and `unwind_stubs.c`) in `crates/dplayx/build.rs`, as a small separate commit that can go upstream. This holds only if the first CI run on Ubuntu 24.04 (mingw GCC 13.2) links cleanly. If it doesn't, CI alone falls back to `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"`. Never use bare `RUSTFLAGS`.
4. **Re-test gate before tagging `0.2.0`:** test the **CI-built release archives** from a release-candidate tag (for example `v0.2.0-rc.1`), so the tested bytes are the ones that ship. Local builds don't count. It must pass:
   - A Windows smoke test: SmartScreen → Run anyway → the UI opens → a Ticket is shown.
   - One real multiplayer game between Linux (Faugus) and real Windows, both sides using the Helper and DLL from the archives. First the user's own Windows machine or dual-boot against their Linux box, then one game with a Windows-using friend over the internet, **both before announcing**.
   - macOS stays built in CI and labelled untested.
5. **Per-release trust checklist (all adopted):**
   - VirusTotal scan of the exe and DLL before announcing, with the report linked in the release notes.
   - A WDSI "software developer" submission if Defender flags anything.
   - GitHub artifact attestations (`actions/attest-build-provenance`) in the release workflow.
   - VERSIONINFO resources on both the exe and the DLL.
   - The startup self-check banner for a missing `dplayx.dll` says it may have been quarantined by antivirus.

## Comments

**2026-09-30, later:** the `0.2.0` here is superseded by [Fork or standalone project?](10-fork-or-standalone.md). The first release is `0.1.0`.
