# 02: Tag push → draft release with the Windows and Linux archives

**What to build:** A maintainer pushes `v0.1.0-rc.1` and, with nothing built locally, gets a draft pre-release on GitHub holding the Windows `.zip`, the Linux `.tar.gz` and a `SHA256SUMS` covering both, with attestations for each archive. A tag that doesn't match the workspace version fails the run before anything is built.

This is the tracer bullet for the whole pipeline: the new, additive `release.yml` with the Ubuntu job, and the release assembly that ticket 03 adds the macOS archive to. Scope, from the spec's "Implementation Decisions" and "Decisions this spec adds":

- **Trigger**: push of any `v*` tag. The version check runs first; it strips `v` and a trailing `-rc.N` and compares with the workspace version.
- **The Ubuntu job** builds the DLL, the Windows Helper and the static musl Linux Helper, and packs:
  - the Windows `.zip`: Helper, DLL, `datalink-mp-README.txt` and both licence files
  - the Linux `.tar.gz`: the same set with the Linux Helper, executable bit kept
  - the DLL on its own as a job artifact, for ticket 03
- **The DLL fix**: delete the `unwind_stubs` step (the `cc::Build` call, `unwind_stubs.c`, and the `cc` build-dependency if nothing else uses it) as its own small commit, so it can later be offered upstream. If CI's mingw still clashes on `_Unwind_Resume`, set the per-target `CARGO_TARGET_I686_PC_WINDOWS_GNU_RUSTFLAGS` link-arg in CI only. Bare `RUSTFLAGS` replaces `.cargo/config.toml`'s flags instead of joining them.
- **Release assembly runs as its own job after the build jobs**, collecting archives from them, so ticket 03 adds one more input rather than restructuring. It writes `SHA256SUMS`, makes the attestations (`actions/attest-build-provenance`), and creates a **draft** release with everything attached, marked pre-release for `-rc.N` tags. The workflow never publishes.
- **`docs/building.md`**: its `_Unwind_Resume` paragraph describes the stubs `build.rs` compiles today. Rewrite it to match what the first CI run showed (stubs gone, and whether the local workaround is still needed on newer mingw).
- **Start `docs/releasing.md`**, mirroring `docs/building.md`'s shape: how to cut an RC tag, what the run produces, where the draft and the attestations show up. Record the gotchas found in this ticket, not a restatement of `release.yml`.

Verify through seam 1 (real tag pushes). You may push and delete `-rc.N` tags and a deliberately mismatched tag. Delete the draft releases and tags your test runs leave behind, except the last good RC. Use seam 2 (local cross-builds) to try the DLL fix before spending a CI run on it.

The first good RC's Windows `.zip` unblocks the Windows half of helper-web-ui ticket 17.

**Blocked by:** 01 (VERSIONINFO on the exe and the DLL)

**Status:** resolved

- [x] The DLL-fix commit removes the `unwind_stubs` step and touches nothing else
- [x] A pushed `-rc.N` tag matching the workspace version produces a draft release marked pre-release, with the Windows `.zip`, Linux `.tar.gz` and `SHA256SUMS` attached
- [x] A pushed tag that doesn't match the workspace version fails at the version check, and no draft release appears
- [x] `sha256sum -c SHA256SUMS` passes on the downloaded archives
- [x] `gh attestation verify` passes for each downloaded archive
- [x] The DLL in the `.zip` links no mingw runtime DLLs and carries VERSIONINFO (`objdump -p`, as `docs/research/windows-toolchain.md` did)
- [x] The Windows Helper in the `.zip` links no mingw runtime DLLs and carries VERSIONINFO
- [x] The Linux Helper in the `.tar.gz` is a static x86_64 executable (`file`) and keeps its executable bit after extraction
- [x] The DLL is also available as a job artifact for a later job in the same run
- [x] `docs/building.md` describes the DLL build as it now behaves
- [x] `docs/releasing.md` exists and says how to cut an RC and where its outputs appear
- [ ] Test tags and drafts are cleaned up; one good `-rc.N` draft remains

## Comments

**2026-10-01 (agent):** Built and checked through seam 2; seam 1 (a real tag push) not yet run.

- The DLL-fix commit is `Drop the DLL's unwind_stubs build step`. Without the stub the DLL links cleanly on Fedora's mingw GCC 15.1 with no workaround, with the same exports and imports. Ubuntu 24.04's `gcc-mingw-w64-i686-win32` 13.2 package ships a DWARF `libgcc_eh.a` that defines `_Unwind_Resume` (checked in the `.deb`), so the first CI run should link without the fallback too. The workflow retries with the per-target flag only if the log shows the clash.
- The version check is `scripts/release-version.sh`, checked locally against `v0.1.0`, `v0.1.0-rc.1`, `-rc.12` (pass) and `v0.1.1`, `-beta`, `-rc`, `-rc.x`, `-rc.1-rc.2`, no `v` (fail).
- The workflow's "Check the binaries" and "Pack the archives" steps were run locally against local cross-builds: no mingw runtime DLL imports, VERSIONINFO 0.1.0 in both, a `static-pie linked` musl Helper that runs, and a `.tar.gz` that keeps the executable bit. `actionlint` (with shellcheck) is clean.
- Archives are named by version, not tag, so the gated RC's archives can `--clobber` the final tag's.
- Not done: pushing a real `-rc.N` and a mismatched tag. `gh` isn't installed on the dev host, so the drafts couldn't be inspected, verified (`gh attestation verify`) or deleted afterwards, and pushing a tag would also publish every commit `main` is ahead of `origin` (24 before this work). Remaining unchecked boxes all need that run.

**2026-10-01 (agent):** Seam 1 run.

- `v0.1.2-rc.1` (mismatched) failed at the version check with `tag v0.1.2-rc.1 doesn't match the workspace version 0.1.0`; no build job ran and no draft appeared (https://github.com/BlinkingApe/datalink-mp/actions/runs/36881637317).
- `v0.1.0-rc.2` passed every job (https://github.com/BlinkingApe/datalink-mp/actions/runs/36881643024) and left a draft pre-release "datalink-mp 0.1.0" with the three archives, `SHA256SUMS` and `attestation.sigstore.json`.
- The DLL linked first time on Ubuntu's mingw GCC 13.2; the `--allow-multiple-definition` fallback didn't fire. `docs/building.md` now says so.
- On the downloaded archives: `sha256sum -c SHA256SUMS` OK for all; `gh attestation verify` exits 0 for each, its attested digest equals the `SHA256SUMS` line, and the signer is `release.yml@refs/tags/v0.1.0-rc.2` (a junk file fails). In the `.zip`, the DLL and exe import no mingw runtime DLL and carry FileVersion 0.1.0 / ProductName datalink-mp. The Linux Helper is `static-pie linked`, extracts `-rwxr-xr-x` and runs. The `dplayx-dll` artifact was consumed by the macOS job.
- Gotcha, now in `docs/releasing.md`: the first tags (`v0.1.0-rc.1`, `v0.1.1-rc.1`) were pushed while `main` on GitHub lacked `release.yml` and started no run at all. After pushing `main`, fresh tags ran normally. `-rc.1` is spent, so the first real RC is `-rc.2`.
- **Cleanup not done:** deleting remote tags was blocked for the agent. The maintainer needs to run `git push --delete origin v0.1.0-rc.1 v0.1.1-rc.1 v0.1.2-rc.1` and `git tag -d v0.1.0-rc.1 v0.1.1-rc.1 v0.1.2-rc.1`. None of them has a draft. The `v0.1.0-rc.2` draft is the one to keep.
