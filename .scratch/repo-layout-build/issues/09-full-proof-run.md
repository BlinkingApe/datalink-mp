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

### Code review fixes

2026-10-07, after review of the proof scripts and docs. Each script fix has a failing case in `scripts/test-proof-scripts.sh` first (29 tests, all pass).

- `scripts/check-old-names.sh`: a git grep error (exit above 1, or anything on stderr, such as an unreadable file) now fails with exit 2 instead of reporting "0 hits"; the allowlist's last entry is read when the file has no final newline; a pathspec matching no tracked file is an error (exit 2); `--all` works in any position; `-h`/`--help` prints the header usage.
- `scripts/check-links.py`: links with single-quoted or parenthesised titles are checked; fences follow CommonMark (a fence closes only on its own character, at least as long, with nothing after), so a ``` line inside a ~~~ block no longer flips it; a path naming no tracked file is an error (exit 2); `--all` works in any position; `-h`/`--help` prints usage.
- Both script headers name the effort (repo-layout-build 04) and `CONTRIBUTING.md`'s Checks section instead of the spec path, which moves on archive.
- `docs/adr/0001-web-ui-frontend.md`: `iroh_transport::Ticket::parse` is now `datalink_transport::Ticket::parse` (crate path only).
- `docs/archive/README.md`: says relative links in archived files were fixed so they resolve, while backticked paths and crate names keep their old names; notes that `prototype/turn-sync-activity` is still live (internet-play-speed-build 12) with its tip equal to the tag.

Rerun: `scripts/check-old-names.sh` 0 not allowlisted, 166 allowlisted, exit 0; `scripts/check-links.py` 0 broken not allowlisted, 430 allowlisted, 5 branch references (the same 5 as above), exit 0. On the current tree the new link checker's `--all` output is identical to the old one's.
