# 05: The target folder tree

Type: grilling
Status: resolved
Blocked by: 01, 02

## Question

What is the target tree for every *necessary* path in the ledger? Cover:

- **Root:** which files stay (README, licences, `Cargo.toml`, `CONTEXT.md`, `CLAUDE.md`). Where `datalink-mp-README.txt`, the in-archive README the release ships, goes.
- **`docs/`:** split by readers (players, contributors, agents), and where ADRs, research write-ups, `faction-colors.md`, `wine-compatibility.md`, `traffic-capture.md` and the images go.
- **`tools/`, `scripts/`, `build-support/`:** keep, merge or rename.
- **Names:** filename conventions (no spaces, a single case style).

For each move, list its inbound references from the ledger, so the move list is complete.

## Answer

Decided with the maintainer, 2026-10-06. They accepted every recommendation. Crate folder names below are today's; [06](06-crate-renames.md) renames them, and 07 renames the checkout folder.

### Target tree

```
README.md                      players' front page (stays)
CONTRIBUTING.md                NEW: contributor entry point; explains .scratch/ (decision 04)
CONTEXT.md  CLAUDE.md          stay
LICENSE-MIT  LICENSE-APACHE  LICENSE-FONTS   stay at root (release.yml copies them from here)
Cargo.toml  Cargo.lock  .gitignore  .ignore (NEW, hides both archives)
.cargo/  .github/              stay
packaging/README.txt           was datalink-mp-README.txt; still ships under the archive name it has today
build-support/versioninfo.rs   stays
scripts/                       stays (release-version.sh, launch-whisky.sh)
crates/                        datalink-mp, dplayx, dp-types, ipc-protocol, iroh-transport, smac-fixes,
                               mock-dp-client (moved from tools/); smac-helper/ deleted
docs/
  players/        faction-colors.md, wine-compatibility.md, images/ (7 PNGs)
  contributors/   architecture.md, building.md, traffic-capture.md
  maintainers/    releasing.md, capture-test-checklist.html
  adr/            0001-0005, unchanged
  agents/         domain.md, issue-tracker.md, triage-labels.md, unchanged
  archive/        README.md (index), research/ (4 write-ups), 0.1.0-release-checklist.html, wtp-through-the-ages.html
.scratch/         README.md (NEW), efforts, archive/ (decision 03)
```

`tools/` disappears. Names: lowercase kebab-case, no spaces. Only GitHub-special root files stay uppercase. The images' placement under `docs/players/` follows from the reader split; the agent chose it, not asked separately.

### Moves, with what each breaks

| Move | Inbound references to rewrite |
|---|---|
| `datalink-mp-README.txt` -> `packaging/README.txt` | `release.yml` (3 `cp` lines); `docs/maintainers/releasing.md`; release-pipeline 02, 04, 05; game-session-sync 04; post-0.1.0-polish 02 and map; ui-polish-0.1.1 02 (the last five are prose mentions, archived or planning, update where live) |
| `docs/ARCHITECTURE.md` -> `docs/contributors/architecture.md` | `README.md`, `docs/building.md` (4x), internet-play-speed 02 and map, release-pipeline 07 |
| `docs/building.md` -> `docs/contributors/building.md` | `README.md`, `docs/releasing.md` (4x), `scripts/launch-whisky.sh` (error message), game-session-sync 02, post-0.1.0-polish 05, release-pipeline 01, 02, spec |
| `docs/traffic-capture.md` -> `docs/contributors/traffic-capture.md` | `crates/iroh-transport/src/capture.rs`, `analyse_capture.py`, internet-play-speed-build 02, 09, internet-play-speed 01-04 and map, game-session-sync 05, post-0.1.0-polish 05 |
| `docs/releasing.md` -> `docs/maintainers/releasing.md` | `release.yml` (comment), `docs/building.md`, ADR-0002, release-pipeline 02-04 and spec, repo-layout map and 02 |
| `docs/datalink-mp-capture-test-checklist.html` -> `docs/maintainers/capture-test-checklist.html` | no inbound by name (internet-play-speed-build 15, 16 reuse it by description; check on build) |
| `docs/wine-compatibility.md`, `docs/faction-colors.md` -> `docs/players/` | `README.md`, `docs/building.md` (faction-colors), `docs/ARCHITECTURE.md` (wine) |
| `docs/images/*` -> `docs/players/images/` | `README.md` (7 `<img src>`) |
| `tools/mock-dp-client/` -> `crates/mock-dp-client/` | `Cargo.toml` (member; covered by the `crates/*` glob only if members becomes a glob), `Cargo.lock` unaffected, `docs/ARCHITECTURE.md`, `docs/building.md` |
| 4 `docs/research/*.md` -> `docs/archive/research/` | ADR-0002 (branch names, kept by tags), release-pipeline spec (`windows-toolchain.md`), both indexed in `docs/archive/README.md` |
| 0.1.0 checklist, `tools/0001-wtp-through-the-ages.html` -> `docs/archive/` | release-pipeline 04; helper-web-ui 18 *(archive)* |
| Deleted (decision 03): rc4 notes, rc.6 checklist, `capture_test_launch.txt`, `crates/smac-helper/` | internet-play-speed and `docs/traffic-capture.md` mention the launch command: confirm `traffic-capture.md` carries it before deleting |
| New files | `CONTRIBUTING.md`, `.scratch/README.md`, `.ignore`, `docs/archive/README.md`, `.gitignore` line `.scratch/**/captures/` |

### Things the move must not break

`LICENSE-*` stay at the root because `release.yml` and `http.rs` use those paths. `build-support/` stays, so both `build.rs` `include!` paths hold. `crates/datalink-mp/src/` is untouched, so the `include_str!` and font paths hold. Whatever else resolves from the build tickets' point of view is the job of the "prove nothing broke" ticket.

### Left to other tickets

Crate names and folder names (06). The `members` glob versus an explicit list is a build detail for 06. Timing relative to internet-play-speed-build, which cites `docs/traffic-capture.md`, `analyse_capture.py` and `docs/` paths in 18 open tickets (fog).
