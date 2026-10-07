# 09: Full proof run

**What to build:** The agent-run checks pass on the final tree, with a report for the maintainer.

**Blocked by:** 06, 07, 08

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Build gate with the exact `release.yml` commands, test and clippy all pass
- [x] Grep and link-check scripts pass; non-allowlisted hits and branch-pointing links are listed for the maintainer
- [x] A local replay of the release packaging steps yields the same archive file names as before (`datalink-mp-README.txt`, three `datalink-mp-LICENSE-*.txt`, the Helper binary, `dplayx.dll`)
- [x] Rollback rule applied if needed: reset the layout branch to `pre-layout` and re-plan if a failure is unexplained after one fix attempt or a dependency version changed

## Comments

2026-10-07, on `layout` at f46f90d. Every check passed on the first run, so the rollback rule wasn't needed.

| Check | Result |
|---|---|
| `cargo build --release --locked --target i686-pc-windows-gnu -p dplayx` | ok |
| `cargo build --release --locked --target x86_64-pc-windows-gnu -p datalink-mp` | ok |
| `cargo build --release --locked -p datalink-mp` (native; stands in for musl, see 02) | ok |
| `cargo test --workspace` | 243 passed, 0 failed, 2 ignored, 18 suites (same as baseline) |
| `cargo clippy --workspace --all-targets` | ok; same warnings per crate as baseline, under the new names |
| `scripts/check-old-names.sh` | 0 not allowlisted, 164 allowlisted |
| `scripts/check-links.py` | 0 broken not allowlisted, 426 allowlisted |
| `scripts/test-proof-scripts.sh` | all pass |
| `Cargo.lock` against `pre-layout` with the four names mapped | 434 packages; versions, sources, checksums and dependencies identical |
| Packaging replay (release.yml's pack steps with stand-in binaries, `pre-layout` tree vs `layout` tree) | identical: Windows zip and Linux tar.gz each hold the Helper, `dplayx.dll`, `datalink-mp-README.txt` and three `datalink-mp-LICENSE-*.txt`; the macOS stage holds the same minus the DLL; names, modes and content hashes match |

**For the maintainer: the 5 branch-pointing references, all kept on purpose:**

- `docs/adr/0001-web-ui-frontend.md:59` `prototype/ui-flow`, and `docs/adr/0002-releases-versioning-pipeline-trust.md:6` `research/{windows-toolchain,macos-artifact,av-risk}`. Accepted ADRs only get crate-path edits. The three research names resolve through `archive/research-*` tags. `prototype/ui-flow` is now deleted (its content is reachable from `main` as `6399164`, and `docs/archive/README.md` says so).
- `docs/release-notes/0.1.1.md:16`, a `blob/main/docs/contributors/traffic-capture.md` URL that resolves once `layout` reaches `main`. The published v0.1.1 Release body still links the old `blob/main/docs/traffic-capture.md` and needs a hand edit after merge.

No grep hits remain outside the allowlist.
