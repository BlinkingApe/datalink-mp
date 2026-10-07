# 05: Move docs, packaging and archives

**What to build:** The repo has the target tree from the spec, apart from the crate renames: a player-first root, `docs/` split by reader, archives separated and hidden from search, and the release workflow still packaging the same files.

**Blocked by:** 03, 04

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Moves follow the table in the target-folder-tree ticket row by row, using `git mv`, with inbound references rewritten except the open-ticket and live-prose rewrites that ticket 07 handles
- [x] `datalink-mp-README.txt` is now `packaging/README.txt`; `release.yml` copies it into the archive under the old shipped name and its comment points at `docs/maintainers/releasing.md`
- [x] Files in the spec's delete list are removed (after confirming `traffic-capture.md` carries the launch command); `crates/smac-helper/` is gone
- [x] Finished efforts (release-pipeline, ui-polish-0.1.1) are in `.scratch/archive/`; `docs/archive/` holds all seven research write-ups, the 0.1.0 checklist and the WTP page, with a `README.md` index of old path, new path or deleted, and recovery tag
- [x] Root `.ignore` lists both archives
- [x] The grep and link-check scripts run and every new hit is understood or on the allowlist; builds still pass

## Comments

2026-10-07 (agent). Built against the proof gate: both scripts and the packaging replay were run before the moves (red), after the raw `git mv`s (to list what broke), and after the rewrites.

**Moves** (all `git mv`, one commit): the target-folder-tree table row by row, except `tools/mock-dp-client/`, which ticket 06 moves with the crate renames.

- `datalink-mp-README.txt` → `packaging/README.txt`
- `docs/ARCHITECTURE.md`, `building.md`, `traffic-capture.md` → `docs/contributors/` (`architecture.md` lowercased)
- `docs/releasing.md` → `docs/maintainers/releasing.md`; `docs/datalink-mp-capture-test-checklist.html` → `docs/maintainers/capture-test-checklist.html` (nothing cites it by name)
- `docs/faction-colors.md`, `wine-compatibility.md`, `images/` → `docs/players/`
- all seven `docs/research/*.md` → `docs/archive/research/`; `docs/datalink-mp-0.1.0-release-checklist.html` → `docs/archive/0.1.0-release-checklist.html`; `tools/0001-wtp-through-the-ages.html` → `docs/archive/wtp-through-the-ages.html`
- `.scratch/release-pipeline/`, `.scratch/ui-polish-0.1.1/` → `.scratch/archive/`
- deleted: `docs/datalink-mp rc4 tests.md`, `crates/smac-helper/` (its one prototype HTML). The launch command (`DATALINK_CAPTURE=... ./target/release/datalink-mp`) is at `docs/contributors/traffic-capture.md:20`, so the untracked `capture_test_launch.txt` loses nothing.
- new: root `.ignore` (`/.scratch/archive/`, `/docs/archive/`; checked with `rg --hidden`: archive hits disappear, `--no-ignore` brings them back) and `docs/archive/README.md` (old path, new path or deleted, recovery tag; it also lists the four branch tags and the moved live files).

**Rewritten references:** `README.md` (7 images, 4 doc links); `release.yml` (comment → `docs/maintainers/releasing.md`; the Linux/Windows and macOS `cp` lines copy `packaging/README.txt` to `datalink-mp-README.txt`; the `COMMON=` list is unchanged); links inside the moved docs; `docs/maintainers/releasing.md` gains one bullet naming the README's source and shipped name; `scripts/launch-whisky.sh` error message; the `capture.rs` doc comment; `docs/release-notes/0.1.1.md`'s GitHub URL; `docs/contributors/traffic-capture.md`'s pointer to the JACKAL write-up (now a link into `docs/archive/research/`, branch mention dropped).

**Decided here, not in the spec:**

- Relative markdown links inside the two newly archived efforts (and the one inbound link from `archive/helper-web-ui` 17) were re-pointed for the extra folder level, as `web-ui-v1` was when it was archived. All of them resolve (`check-links.py --all` on those folders: 0 broken markdown links). Backticked repo paths in archived text stay as written (history; allowlisted). Same for the two `../adr/` links in the archived research write-ups.
- `.scratch/game-session-sync/map.md:13`'s backticked `../release-pipeline/issues/` is an inbound reference to an effort archived here, but it sits in live-effort prose, so it is left to 07.
- ADR-0001:10's `crates/smac-helper` describes the seed project's layout; it is allowlisted in `links-allowlist.txt` rather than rewritten.

**Allowlist entries added** (each with a comment): `old-names-allowlist.txt`: `release.yml` lines matching `COMMON=\(|cp packaging/README\.txt`, and the `releasing.md` line naming the shipped file. `links-allowlist.txt`: ADR-0001's `crates/smac-helper`.

**Proof gate:**

- `check-old-names.sh`: before 167 not allowlisted / 113 allowlisted; after **134 / 150**. Drops are files that moved into the archives. The only new hits were `release.yml` and `releasing.md` naming the shipped file (now allowlisted). Every remaining hit is a crate name or `tools/mock-dp-client` (ticket 06: code, manifests, `Cargo.lock`, `docs/contributors/architecture.md` 7, `building.md` 4, `traffic-capture.md` 1, ADR-0001:163) or live-effort prose (ticket 07: game-session-sync, internet-play-speed, internet-play-speed-build 03 and 05, post-0.1.0-polish, including three mentions of `datalink-mp-README.txt` there).
- `check-links.py`: before 27 broken / 210 allowlisted / 23 branch refs; after **23 / 373 / 22**. Remaining broken links, all for 07: `docs/building.md`, `docs/traffic-capture.md`, `docs/releasing.md`, `docs/ARCHITECTURE.md` and `docs/research/*` in game-session-sync 02, 05 and map; internet-play-speed-build 02, 06, 09; internet-play-speed analysis, 01-05 and map; post-0.1.0-polish 05. Plus the pre-existing `crates/iroh-transport/src/early_ack.rs` in internet-play-speed 09 (a planned file; 06/07 rename its crate). Branch references are unchanged apart from the release-notes URL, which now points at `main`'s `docs/contributors/traffic-capture.md` (valid once merged).
- `scripts/test-proof-scripts.sh`: all pass.
- Build gate: i686 `dplayx`, x86_64-windows `datalink-mp` and native `datalink-mp` `--locked` release builds exit 0; `cargo test --workspace` **243 passed, 0 failed, 2 ignored** (as baseline); `cargo clippy --workspace --all-targets` exit 0 with the same warning counts per crate as `baseline-clippy.txt`.
- Packaging replay: the `ubuntu` "Pack the archives" step and the `macos` copy lines, run from `release.yml` with stand-in binaries against an export of `pre-layout` and of this tree. The listings (file names, modes, content hashes) of the Windows `.zip`, Linux `.tar.gz` and macOS stage folder are **identical**: `datalink-mp(.exe)`, `dplayx.dll`, `datalink-mp-README.txt`, `datalink-mp-LICENSE-{MIT,APACHE,FONTS}.txt`.

**For the maintainer:** if the 0.1.1 GitHub Release is already published, its body still links to `blob/main/docs/traffic-capture.md`, which 404s once this reaches `main`. Edit the release body by hand; the repo copy is fixed.
