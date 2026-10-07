# Ledger: what's necessary in the repo

Built for [Build the classification ledger](issues/01-classification-ledger.md), 2026-10-06, from the working copy on `capture/traffic-capture` (8 commits ahead of `main`; `main` has no files this branch lacks).

**Necessary** means one of: **(1)** builds, tests or ships the product; **(2)** a living doc read to work today; **(3)** planning for an active effort. See the [map's Notes](map.md).

Inbound references list the files that name a path, by full path or a distinctive file name. `.claude/worktrees/` copies are left out. A reference from an archived effort is marked *(archive)*.

## Root and build

| Path | Verdict | Reason | Confidence | Inbound references |
|---|---|---|---|---|
| `Cargo.toml` | necessary | (1) workspace manifest; lists every member path and the three internal path deps | confident | `docs/releasing.md` (version bump); research `transport-restart.md`; repo-layout 05, 06 |
| `Cargo.lock` | necessary | (1) `--locked` builds in `release.yml` | confident | `docs/releasing.md`; research docs |
| `.cargo/config.toml` | necessary | (1) per-target rustflags for the cross builds | confident | `release.yml` (comment), `docs/building.md`, ADR-0002; research `windows-toolchain.md`, `macos-artifact.md`, `av-risk.md`; release-pipeline 02, spec; web-ui-v1 02 *(archive)* |
| `.gitignore` | necessary | (1) keeps `target/` and logs out | confident | internet-play-speed 03 |
| `.github/workflows/release.yml` | necessary | (1) the release pipeline | confident | `docs/releasing.md`, ADR-0002, `scripts/release-version.sh`; release-pipeline 02, 04, 05, spec; research docs |
| `build-support/versioninfo.rs` | necessary | (1) `include!`d by both `build.rs` files | confident | `crates/datalink-mp/build.rs`, `crates/dplayx/build.rs` (`include!("../../build-support/versioninfo.rs")`: a move breaks the build) |
| `scripts/release-version.sh` | necessary | (1) called by `release.yml` | confident | `release.yml`, `docs/releasing.md`; release-pipeline 02 |
| `scripts/launch-whisky.sh` | necessary | (1) runs the game under Whisky for testing on macOS | confident | `docs/building.md`; research `macos-artifact.md` |
| `LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE-FONTS` | necessary | (1) shipped in every archive; kept by ADR-0003 | confident | `release.yml` (`cp "LICENSE-$l"`, hard-coded root path), `README.md`, `datalink-mp-README.txt`, `crates/datalink-mp/src/http.rs` (FONTS), `docs/datalink-mp-0.1.0-release-checklist.html`; release-pipeline 04, 05; helper-web-ui spec, 18 *(archive)* |
| `README.md` | necessary | (2) the players' front page; (1) GitHub renders it | confident | `docs/building.md`, `docs/releasing.md`; release-pipeline 04, spec; repo-layout map |
| `datalink-mp-README.txt` | necessary | (1) the README shipped inside each archive | confident | `release.yml` (3×, hard-coded root path), `docs/datalink-mp-0.1.0-release-checklist.html`, `docs/datalink-mp-rc6-test-checklist.html`; game-session-sync 04, post-0.1.0-polish 02 and map, release-pipeline 02, 04, 05, ui-polish-0.1.1 02, repo-layout map and 05; helper-web-ui 16, spec *(archive)* |
| `CONTEXT.md` | necessary | (2) the domain glossary | confident | `CLAUDE.md`, `docs/agents/domain.md` (6×); maps of game-session-sync, internet-play-speed, post-0.1.0-polish, repo-layout; internet-play-speed-build 01; release-pipeline spec; web-ui-v1, helper-web-ui *(archive)* |
| `CLAUDE.md` | necessary | (2) agent entry point; names `.scratch/<feature>/`, `docs/agents/*`, `CONTEXT.md`, `docs/adr/` | confident | repo-layout map, 01, 02, 05 |

## Crates and tools

| Path | Verdict | Reason | Confidence | Inbound references |
|---|---|---|---|---|
| `crates/datalink-mp/` (`Cargo.toml`, `build.rs`, `src/` with `page.html` and 7 fonts, `tests/`) | necessary | (1) the Helper | confident | `Cargo.toml` (member); `release.yml` (`-p datalink-mp`, binary name); `src/http.rs` `include_str!("page.html")` and `include_bytes!("fonts/…")`; `build.rs` `include!` of `build-support/`; `docs/ARCHITECTURE.md`, `docs/building.md`; tickets in game-session-sync 01, 02, 04, post-0.1.0-polish 04, 06, internet-play-speed 08 cite `src/` files |
| `crates/dplayx/` (`Cargo.toml`, `build.rs`, `dplayx.def`, `src/`) | necessary | (1) the DLL | confident | `Cargo.toml` (member); `release.yml` (`-p dplayx`, `dplayx.dll` paths); `build.rs` reads `dplayx.def`; `docs/ARCHITECTURE.md`, `docs/building.md`, ADR-0001, ADR-0002; research `windows-toolchain.md`, `transport-restart.md`; game-session-sync 02, internet-play-speed 02 and analysis, post-0.1.0-polish 05, release-pipeline 01, 07; web-ui-v1 02, 05, 09 *(archive)* |
| `crates/dp-types/` | necessary | (1) shared DirectPlay types | confident | `Cargo.toml` (member and `[workspace.dependencies]` path); `docs/building.md`; post-0.1.0-polish 05 |
| `crates/ipc-protocol/` | necessary | (1) DLL–Helper IPC | confident | `Cargo.toml` (member and path dep); `docs/ARCHITECTURE.md`, `docs/building.md`; research `transport-restart.md` |
| `crates/iroh-transport/` (`src/` incl. `capture.rs`, `tests/`) | necessary | (1) the transport | confident | `Cargo.toml` (member and path dep); `docs/ARCHITECTURE.md`, `docs/building.md`, `docs/traffic-capture.md`, ADR-0001; research `transport-restart.md`; game-session-sync 02, 03, 05, internet-play-speed 01, 03, 09 and analysis, post-0.1.0-polish 04, 05, release-pipeline 07; web-ui-v1 01, 08 *(archive)* |
| `crates/smac-fixes/` | necessary | (1) game fixes linked into the DLL | confident | `Cargo.toml` (member); `docs/ARCHITECTURE.md`, `docs/building.md`; research `av-risk.md`; web-ui-v1 04 *(archive)* |
| `tools/mock-dp-client/` | necessary | (1) workspace member; transport test client | confident | `Cargo.toml` (member), `Cargo.lock`; `docs/ARCHITECTURE.md`, `docs/building.md` |
| `crates/smac-helper/prototype/ui-flow-prototype.html` | not necessary | prototype for a closed decision (web-ui-v1 06); the only file under a crate folder that isn't a crate | confident | web-ui-v1 06 *(archive)*, repo-layout 02. Also on branch `prototype/ui-flow`. Folder named by ADR-0001 and research `transport-restart.md` (old crate name) |
| `tools/0001-wtp-through-the-ages.html` | not necessary | the maintainer's design guide for the page's look. Helper-web-ui 18 says "kept out of git", but commit `Commit pending game-session-sync notes, transport changes and WTP page` (2026-10-04) added it. 187 KB | confident (answered, Q1) | helper-web-ui 18 *(archive)*, repo-layout 02 |

## Docs

| Path | Verdict | Reason | Confidence | Inbound references |
|---|---|---|---|---|
| `docs/ARCHITECTURE.md` | necessary | (2) | confident | `README.md`, `docs/building.md` (4×); internet-play-speed 02 and map; release-pipeline 07 |
| `docs/building.md` | necessary | (2) | confident | `README.md`, `docs/releasing.md` (4×), `scripts/launch-whisky.sh` (error message); game-session-sync 02, post-0.1.0-polish 05, release-pipeline 01, 02, spec |
| `docs/releasing.md` | necessary | (2) | confident | `release.yml` (comment), `docs/building.md`, both test checklists; release-pipeline 02, 03, 04, spec; repo-layout map, 02 |
| `docs/wine-compatibility.md` | necessary | (2) README links it | confident | `README.md`, `docs/ARCHITECTURE.md`; repo-layout 05 |
| `docs/faction-colors.md` | necessary | (2) README links it | confident | `README.md`, `docs/building.md`; repo-layout 05 |
| `docs/traffic-capture.md` | necessary | (2) dev tool doc; ships with internet-play-speed-build 02 | confident | `crates/iroh-transport/src/capture.rs`, `.scratch/internet-play-speed/analysis/analyse_capture.py`; internet-play-speed-build 02, 09; internet-play-speed 01–04 and map; game-session-sync 05; post-0.1.0-polish 05; repo-layout 05 |
| `docs/images/` (7 PNGs) | necessary | (1) assets the README shows | confident | `README.md` (7 `<img src="docs/images/…">`) |
| `docs/adr/0001`–`0005` | necessary | (2) ADRs are kept, superseded or not | confident | each other; `docs/releasing.md` (0002); maps of internet-play-speed, game-session-sync, repo-layout; internet-play-speed-build 06, 07, 11, 17, 18 (0005); repo-layout 06 (0003); web-ui-v1, helper-web-ui *(archive)* |
| `docs/agents/domain.md`, `issue-tracker.md`, `triage-labels.md` | necessary | (2) agent docs | confident | `CLAUDE.md`; `issue-tracker.md` → `triage-labels.md`; repo-layout 02, 04 |
| `docs/research/windows-toolchain.md`, `macos-artifact.md`, `av-risk.md`, `transport-restart.md` | not necessary | evidence for closed decisions (web-ui-v1 01–04, ADR-0002). `docs/building.md` and `docs/releasing.md` now carry what's still used | confident (answered, Q2) | web-ui-v1 01–04 and map *(archive)*; release-pipeline 02, spec (`windows-toolchain.md`); ADR-0002 names their branches |
| `docs/datalink-mp-0.1.0-release-checklist.html` | not necessary | the 0.1.0 gate's checklist; 0.1.0 is published | confident (answered, Q3) | release-pipeline 04 |
| `docs/datalink-mp rc4 tests.md` (space in name) | not necessary | raw notes from `-rc.4` testing; what they found is ticketed | confident (answered, Q3) | helper-web-ui 17 *(archive)*, release-pipeline 04 |
| `docs/datalink-mp-rc6-test-checklist.html` (untracked) | not necessary | `-rc.6` checklist; 0.1.0 shipped from `-rc.7` | confident (answered, Q3) | none |
| `docs/datalink-mp-capture-test-checklist.html` | necessary | (3) checklist for a captured game; reused by internet-play-speed-build 15 and 16 | confident (answered, Q4) | none by name |
| `docs/capture_test_launch.txt` (untracked) | not necessary | a 3-line launch command; the capture checklist and `docs/traffic-capture.md` carry it | confident | none |

## `.scratch/` planning

Status counted from each effort's ticket files.

| Path | Verdict | Reason | Confidence | Inbound references |
|---|---|---|---|---|
| `.scratch/repo-layout/` (untracked: map, 7 tickets, this ledger) | necessary | (3) this effort | confident | none outside itself |
| `.scratch/internet-play-speed-build/` (18 tickets, branch only) | necessary | (3) 18 open build tickets | confident | repo-layout map |
| `.scratch/internet-play-speed/` (map, 11 tickets, `.gitignore`, `analysis/` with 2 write-ups and 5 scripts; branch only) | necessary | (3) every ticket is resolved, but internet-play-speed-build builds from it: ADR-0005 calls ticket 09 "the spec to build from", build tickets 04, 07, 11, 12 cite tickets 07–10, and build ticket 02 moves `analysis/analyse_capture.py` with the capture | confident | ADR-0005 (map, ticket 09, ticket 10, `analysis/turn-sync-fixes.md`); internet-play-speed-build 02, 04, 07, 09, 11, 12; `docs/traffic-capture.md` and both capture checklists (`captures/` path); game-session-sync 05, post-0.1.0-polish 05 |
| `.scratch/internet-play-speed/captures/` | out of scope | gitignored; 2.5 MB of raw captures, the maintainer's game state | — | `docs/traffic-capture.md`, `docs/capture_test_launch.txt`, capture checklist (as the `DATALINK_CAPTURE` target) |
| `.scratch/post-0.1.0-polish/` (map, 6 tickets) | necessary | (3) 01–03 are built (ui-polish-0.1.1). 04 is decided but not ticketed for a build yet. 05 and 06 are `needs-triage`. | confident (answered, Q5) | `docs/traffic-capture.md` (05), capture checklist; ui-polish-0.1.1 01–03; internet-play-speed 03, 04; release-pipeline 04; helper-web-ui 17 *(archive)* |
| `.scratch/game-session-sync/` (map, 5 tickets) | necessary | (3) 01–04 resolved. 05 is `needs-triage`, and the map's fog holds "the fix for 09 and the killed host, after 0.1.0". | confident (answered, Q5) | `docs/releasing.md`, `docs/traffic-capture.md`, `crates/iroh-transport/tests/mesh_networking.rs` (comment), both release checklists, capture checklist; post-0.1.0-polish 04 and map; internet-play-speed 03, 04; release-pipeline 04, 07; helper-web-ui 17 *(archive)* |
| `.scratch/release-pipeline/` (spec, 6 tickets) | not necessary | every ticket resolved; 0.1.0 published. ADR-0002 and `docs/releasing.md` are the living record. The spec's `Status: ready-for-agent` is stale | confident | both release checklists; game-session-sync 01, 02, 03 and map; helper-web-ui 17, 19 *(archive)* |
| `.scratch/ui-polish-0.1.1/` (3 tickets) | not necessary | all three resolved and merged to `main` (not yet released) | confident | post-0.1.0-polish 01, 02, 03 |
| `.scratch/archive/web-ui-v1/` (map, 10 tickets) | not necessary | closed, already archived | confident | ADR-0001, ADR-0002, ADR-0003 (decision trails); release-pipeline spec |
| `.scratch/archive/helper-web-ui/` (spec, 20 tickets) | not necessary | closed, already archived | confident | release-pipeline spec, 04; post-0.1.0-polish map |

## Branches

"Merged" means reachable from `main`. "Landed" means an equivalent commit is on `main`. Local `main` is **7 commits ahead of `origin/main`** (unpushed).

| Branch | Verdict | Merged | Last commit | Made by | Confidence | Inbound references |
|---|---|---|---|---|---|---|
| `main` | necessary | — | 2026-10-06 | — | confident | — |
| `capture/traffic-capture` | necessary | no (+8), **not pushed** | 2026-10-06 | internet-play-speed 03–11; internet-play-speed-build charting | confident | internet-play-speed-build 02 |
| `prototype/turn-sync-activity` | necessary | no (+1), not pushed | 2026-10-06 | internet-play-speed 08 | confident | internet-play-speed 08 and map; internet-play-speed-build 12 |
| `research/iroh-connection-stats` | necessary | no (+1), not pushed; its doc exists only here | 2026-10-06 | internet-play-speed 01 | confident | internet-play-speed 01 |
| `research/smac-jackal-turn-sync` | necessary | no (+1), not pushed; its doc exists only here | 2026-10-06 | internet-play-speed 02 | confident | `docs/traffic-capture.md`; internet-play-speed 02, 05 and `analysis/` |
| `research/repo-layout-conventions` | necessary | no (+1), not pushed; its doc exists only here | 2026-10-06 | repo-layout 02 | confident | repo-layout 02 |
| `prototype/ui-flow` (+ `origin/`) | not necessary | yes | 2026-09-30 | web-ui-v1 06 | confident | web-ui-v1 06 *(archive)* (commit `6399164`, reachable from `main`) |
| `research/windows-toolchain` (+ `origin/`) | not necessary | yes | 2026-09-29 | web-ui-v1 02 | confident | ADR-0002 (by branch name); web-ui-v1 02 *(archive)* |
| `research/macos-artifact` (+ `origin/`) | not necessary | yes | 2026-09-29 | web-ui-v1 03 | confident | ADR-0002; web-ui-v1 03 *(archive)* |
| `research/av-risk` (+ `origin/`) | not necessary | yes | 2026-09-29 | web-ui-v1 04 | confident | ADR-0002; web-ui-v1 04 *(archive)* |
| `research/transport-restart` (+ `origin/`) | not necessary | yes | 2026-09-29 | web-ui-v1 01 | confident | web-ui-v1 01 *(archive)* |
| `worktree-agent-a0c11cedfecf4cdfa`, `-a174b5685f555a581`, `-a7296922789c47ddb`, `-ac4907ece49e0e2a2` | not necessary | yes (all at `7d26254`, `origin/main`) | 2026-10-06 | agent worktree branches; their worktrees now hold other branches | confident | none |
| `worktree-agent-a19aa87409b3248db` | not necessary | yes | 2026-10-06 | ui-polish-0.1.1 01 | confident | none |
| `worktree-agent-a823a8f31dc3f6b85` | not necessary | yes | 2026-10-06 | ui-polish-0.1.1 03 | confident | none |
| `worktree-agent-a1e1b2bcd52ca0e95` | not necessary | landed as `d3810c7` | 2026-10-06 | ui-polish-0.1.1 02 | confident | none |
| `worktree-agent-a43729bf675fce14c` | not necessary | yes | 2026-09-30 | helper-web-ui 11 | confident | none |
| `worktree-agent-ab80e9c65c170b5cc` | not necessary | yes | 2026-09-30 | helper-web-ui 15 | confident | none |
| `worktree-agent-acb13c12171204650` | not necessary | landed (`git cherry`) | 2026-09-30 | helper-web-ui 16 | confident | none |
| `origin/main` | necessary | — | 2026-10-06 | — | confident | — |
| `upstream/main` | necessary | — | 2026-07-21 | upstream's initial commit | confident | ADR-0003 keeps the `upstream` remote (push disabled) |

## Worktrees

All under `.claude/worktrees/`, hidden by `.git/info/exclude` (not `.gitignore`). None has uncommitted changes.

| Worktree | Branch | Verdict | Confidence | Note |
|---|---|---|---|---|
| `agent-a0c11cedfecf4cdfa` | `research/smac-jackal-turn-sync` | not necessary | confident | the branch holds the content |
| `agent-a174b5685f555a581` | `research/iroh-connection-stats` | not necessary | confident | the branch holds the content |
| `agent-a7296922789c47ddb` | `prototype/turn-sync-activity` | not necessary | confident | the branch holds the content |
| `agent-ac4907ece49e0e2a2` | `research/repo-layout-conventions` | not necessary | confident | the branch holds the content |
| `agent-a19aa87409b3248db` | `worktree-agent-a19aa…` | not necessary | confident | merged |
| `agent-a1e1b2bcd52ca0e95` | `worktree-agent-a1e1b…` | not necessary | confident | landed |
| `agent-a823a8f31dc3f6b85` | `worktree-agent-a823a…` | not necessary | confident | merged |
| `agent-a43729bf675fce14c` | `worktree-agent-a4372…` | not necessary | confident | prunable: git records it under the old `~/Documents/code/smac-helper-gui/smac-iroh` path |
| `agent-ab80e9c65c170b5cc` | `worktree-agent-ab80e…` | not necessary | confident | prunable, same old path |

## Summary by category

Rows from Q1–Q5 are counted by the maintainer's answers.

| Category | Necessary | Not necessary |
|---|---|---|
| Root and build | 13 rows (all) | — |
| Crates and tools | 7 crates/tools | `crates/smac-helper/` prototype, `tools/0001-wtp-through-the-ages.html` |
| Docs | 13 rows: the living guides, ADRs, agent docs, images, `traffic-capture.md`, the capture checklist | 4 research write-ups, 0.1.0 and rc.6 checklists, rc4 notes, `capture_test_launch.txt` |
| `.scratch/` | repo-layout, internet-play-speed-build, internet-play-speed, post-0.1.0-polish, game-session-sync | release-pipeline, ui-polish-0.1.1, both archives |
| Local branches (20) | `main`, `capture/traffic-capture`, `prototype/turn-sync-activity`, 3 unmerged `research/*` | `prototype/ui-flow`, 4 merged `research/*`, 10 `worktree-agent-*` |
| Remote branches | `origin/main`, `upstream/main` | `origin/prototype/ui-flow`, 4 `origin/research/*` |
| Worktrees (9) | — | all 9 |

Facts later tickets will need:

- **Five branches exist only on this machine:** `capture/traffic-capture` and four unmerged research/prototype branches, plus 7 unpushed commits on `main`.
- **Hard-coded paths a move breaks:** `release.yml` copies `datalink-mp-README.txt` and `LICENSE-*` from the root and builds `-p dplayx` / `-p datalink-mp`. Both `build.rs` files `include!("../../build-support/versioninfo.rs")`. `http.rs` `include_str!`/`include_bytes!` the page and fonts by relative path. `scripts/launch-whisky.sh` names `docs/building.md`.
- **ADR decision trails** point into `.scratch/archive/web-ui-v1/` (ADR-0001, 0002, 0003) and `.scratch/internet-play-speed/` (ADR-0005). ADR-0002 also names three `research/*` branches.
- **The capture path** `.scratch/internet-play-speed/captures` is written into `docs/traffic-capture.md` and both capture launch notes.

## Questions for the maintainer

All five answered by the maintainer on 2026-10-06, each matching the agent's leaning. The rows above record them.

- **Q1.** `tools/0001-wtp-through-the-ages.html`: keep it as the design reference for the page, or take it out of the repo as helper-web-ui 18 intended? → **Not necessary.**
- **Q2.** The four merged `docs/research/*.md` write-ups: living reference, or evidence that goes with its closed decisions? → **Not necessary.** They're evidence.
- **Q3.** Past release-testing material (0.1.0 checklist, rc.6 checklist, rc4 notes): keep any as a template for the next gate? → **None necessary.**
- **Q4.** The captured-game checklist: reused for internet-play-speed-build 15 and 16? → **Necessary.** It's reused.
- **Q5.** Are game-session-sync and post-0.1.0-polish still live, or parked? → **Both live.**
