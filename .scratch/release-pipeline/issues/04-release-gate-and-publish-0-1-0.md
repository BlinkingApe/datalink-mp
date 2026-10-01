# 04: Release gate and publish 0.1.0

**What to build:** Nothing is built. The maintainer proves the CI-built `-rc.N` archives work on real machines, scans them, and publishes them as `0.1.0`. Only the archives from the gated RC's CI run count; local builds don't.

Run ADR-0002 section 4 (the gate) and the per-release items of section 3 (VirusTotal, WDSI), with the spec's "Decisions this spec adds" for how the RC's bytes become the release. Record each check in `docs/releasing.md` as it's run: what was done, on which machine and launcher, and anything that surprised you. Keep it to steps and gotchas a future release needs, so it doesn't restate `release.yml`. Open a new ticket for each defect found; a defect means a new `-rc.N` and a fresh gate.

**Blocked by:** 03 (macOS archive in the same release)

**Status:** ready-for-human

The gate, on the archives of one `-rc.N` draft:

- [ ] Windows smoke test: extract the `.zip` into the Game folder, SmartScreen → More info → Run anyway, the page opens, a Ticket is shown
- [ ] Windows file properties of the exe and the DLL show `0.1.0`
- [ ] One real multiplayer game between Linux (Faugus, the musl `.tar.gz`) and the maintainer's own Windows machine, both sides running only the Helper and DLL from the archives
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
