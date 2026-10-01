# 03: macOS archive in the same release

**What to build:** The same `-rc.N` tag push now also produces the macOS `.zip`, so the draft release holds all three archives under one `SHA256SUMS`, every archive attested, and the same DLL in each. The release text labels macOS as built in CI and untested.

Scope, from the spec's "Implementation Decisions" (the macOS job) and ADR-0002 section 2:

- **A `macos-15` job** (arm64; not the billed `-large`/`-xlarge` runners) builds the `aarch64-apple-darwin` Helper.
- Run `codesign --verify` on the Helper; if it fails, re-sign ad-hoc and verify again.
- **Take the DLL from ticket 02's Ubuntu job** rather than building one, so all three archives share one DLL and one hash.
- **Pack with `ditto -c -k`**: `zip` and `upload-artifact` drop properties `ditto` keeps, including the executable bit. Pass the finished `.zip` to the release assembly as-is.
- The release assembly from ticket 02 takes the macOS `.zip` as one more input.
- Add the macOS steps and their gotchas to `docs/releasing.md`.

Verify through seam 1, as in ticket 02: you may push and delete `-rc.N` tags, and you clean up the drafts your test runs leave, except the last good RC.

**Blocked by:** 02 (Tag push → draft release with the Windows and Linux archives)

**Status:** ready-for-agent

- [ ] A pushed `-rc.N` tag produces a draft pre-release with the Windows `.zip`, Linux `.tar.gz` and macOS `.zip`, and one `SHA256SUMS` covering all three
- [ ] `sha256sum -c SHA256SUMS` and `gh attestation verify` pass for all three downloaded archives
- [ ] The `dplayx.dll` inside each of the three archives has the same hash
- [ ] The macOS Helper extracted with Archive Utility or `ditto -x -k` keeps its executable bit, and the log shows `codesign --verify` passing
- [ ] The macOS job runs on `macos-15`
- [ ] The draft release text says macOS is built in CI and untested
- [ ] `docs/releasing.md` covers the macOS archive
- [ ] Test tags and drafts are cleaned up; one good `-rc.N` draft remains

## Comments

**2026-10-01 (agent):** Written and linted; seam 1 not yet run, for the same reason as ticket 02 (no `gh` on the dev host to inspect and clean up drafts, and a tag push publishes `main`'s unpushed commits). No Mac to try the job locally either, so the first real run is also the first time the macOS steps execute.

- The `macos` job needs the Ubuntu job, downloads its `dplayx-dll` artifact, and packs with `ditto -c -k --norsrc --noextattr --noacl` (no AppleDouble entries in the zip). It re-extracts with `ditto -x -k` and fails if the Helper isn't executable, and logs `codesign --verify` (re-signing ad-hoc on failure).
- The release job now waits for it, fails if the three archives' `dplayx.dll` hashes differ, and the draft notes say macOS is built in CI and untested.
- Simulated locally with `gh` stubbed and a stand-in macOS zip: the DLL check passes on matching DLLs and fails on different ones; the notes render as Markdown; `--prerelease` appears only for `-rc.N`.
