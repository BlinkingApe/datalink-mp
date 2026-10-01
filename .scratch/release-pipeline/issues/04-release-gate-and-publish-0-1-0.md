# 04: Release gate and publish 0.1.0

**What to build:** Nothing is built. The maintainer proves the CI-built `-rc.N` archives work on real machines, scans them, and publishes them as `0.1.0`. Only the archives from the gated RC's CI run count; local builds don't.

Run ADR-0002 section 4 (the gate) and the per-release items of section 3 (VirusTotal, WDSI), with the spec's "Decisions this spec adds" for how the RC's bytes become the release. Record each check in `docs/releasing.md` as it's run: what was done, on which machine and launcher, and anything that surprised you. Keep it to steps and gotchas a future release needs, so it doesn't restate `release.yml`. Open a new ticket for each defect found; a defect means a new `-rc.N` and a fresh gate.

**Blocked by:** 03 (macOS archive in the same release)

**Status:** ready-for-human

The gate, on the archives of one `-rc.N` draft:

- [ ] Windows smoke test: extract the `.zip` into the Game folder, SmartScreen → More info → Run anyway, the page opens, a Ticket is shown
- [x] Windows file properties of the exe and the DLL show `0.1.0`
- [x] One real multiplayer game between Linux (Faugus, the musl `.tar.gz`) and the maintainer's own Windows machine, both sides running only the Helper and DLL from the archives
- [ ] The same game with a Windows-using friend over the internet

Trust checks:

- [ ] VirusTotal scan of the exe and the DLL; report links saved for the release notes
- [ ] If Defender flags either: a WDSI "software developer" submission is filed, and `docs/releasing.md` says how to file and follow one up

Publish:

- [ ] Tag `v0.1.0` on the gated RC's commit and push it
- [ ] In the draft that run creates, replace the archives and `SHA256SUMS` with the gated RC's (`gh release download` from the RC, `gh release upload --clobber` to `v0.1.0`)
- [ ] `sha256sum -c SHA256SUMS` and `gh attestation verify` pass on archives downloaded from the `v0.1.0` draft
- [ ] Release notes link the VirusTotal reports and say macOS is untested
- [ ] Publish the `v0.1.0` release by hand, not marked pre-release
- [ ] `docs/releasing.md` holds the whole gate and publish sequence as it was actually run

## Comments

**2026-10-01 (agent):** Checked before the gate; no gate box is ticked, because every one needs a person, a Windows machine, a VirusTotal account or a publish decision.

- The RC to gate is `v0.1.0-rc.2` (commit `d4f9f09`, run https://github.com/BlinkingApe/datalink-mp/actions/runs/36881643024). It's the only tag on `origin` and its draft is the only release (draft, pre-release).
- Downloaded fresh from the draft: `sha256sum -c SHA256SUMS` passes for all three archives and `gh attestation verify` exits 0 for each.
- Hashes in the Windows `.zip`, for a VirusTotal hash search before uploading:
  - `datalink-mp.exe` `f5ef73aba1ece90811eda33bcc714d767278614ff0cf127fce98d1b6be50b725`
  - `dplayx.dll` `203a2ba0e63132759bc4dc095e6cb77650cb7434e60301219d868c326b9a097c` (the same in all three archives)
- `main` is one commit past the RC (`4c12b62`, ticket notes only). Tag `v0.1.0` on `d4f9f09`, the gated commit, not on `main`: `git tag v0.1.0 v0.1.0-rc.2^{}`.
- `docs/releasing.md` section 4's commands say `v0.1.0-rc.1` as an example; with this RC it's `gh release download v0.1.0-rc.2 -D gated`.

**2026-10-01 (maintainer, recorded by agent):** First gate results on `v0.1.0-rc.2`.

- A game between Linux and the maintainer's own Windows machine connected and started, both sides running only the RC's Helper and DLL. Box ticked.
- Smoke test, first try: the Windows files were copied over a USB stick, not downloaded. A Windows Security (firewall) alert appeared, was allowed, and the page opened. A USB copy has no download mark, so this doesn't test SmartScreen.
- Smoke test, second try: the `.zip` was downloaded from the GitHub draft and extracted over the first copy. There was no firewall alert (already allowed for that path) and **no SmartScreen prompt**. The cause isn't known yet: the extractor may have dropped the download mark, or SmartScreen may be off. The SmartScreen part of the smoke test is still unproven, so that box stays unticked.
- VirusTotal: 1 of about 70 engines flagged a file, Acronis (Static ML). Microsoft (Defender) shows Undetected, so no WDSI submission is needed. Still missing: which file was flagged and both report links for the release notes.
- Not yet run: file properties showing `0.1.0`, and the game with a friend over the internet.
- No defect found, so no new ticket or `-rc.N`.

**2026-10-01 (maintainer, recorded by agent):** More gate results.

- Windows file properties show `0.1.0` for the exe and the DLL. Box ticked.
- VirusTotal: the DLL is clean (0/71). The Acronis (Static ML) hit was on the exe. Report links still needed.
- SmartScreen, a third try: SmartScreen's app check is confirmed on. A fresh browser download was extracted into a new folder. The firewall alert came back, but there was still no SmartScreen prompt. Whether the extracted exe carries the download mark (Unblock / `Zone.Identifier`) hasn't been checked yet.
- The game with a friend over the internet is postponed. A substitute was proposed: the laptop on a phone's mobile data, the other machine on home Wi-Fi.
