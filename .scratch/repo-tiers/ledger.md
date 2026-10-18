# Ledger: necessary, nice-to-have, archive

Built 2026-10-18 from `main` at `f75d89e` (version 0.1.1, released as internet-play-speed-build's Release patch A). It covers the 237 tracked files, local branches and tags. `target/`, gitignored captures and the untracked `.claude/` are out of scope.

## Tiers

- **Necessary**: builds, tests or ships the version on `main`, or is a living doc a human or agent reads to work on it today. Code merged on `main` is necessary, dev-only paths included (traffic capture, relay-only switch, debug probes).
- **Nice-to-have**: planning or unmerged work tied to an open ticket or a named future release. It stays where it is; this ledger only labels it.
- **Archive**: everything else. It follows the repo's archive rules ([issue-tracker.md, Repo conventions](../../docs/agents/issue-tracker.md#repo-conventions)): finished efforts go to `.scratch/archive/`, evidence docs to `docs/archive/`, throwaway files are deleted, redundant branches are removed and tags are kept. Paths already under `.scratch/archive/` or `docs/archive/` need no action.

One row per folder where every path inside it agrees, and one row per path where they don't. **Confidence** is *confident* or *unsure*; only *unsure* rows need review.

## Summary

| Tier | Tracked files | Branches and tags | Needs action |
|---|---|---|---|
| Necessary | 106 | `main`, `v0.1.1` | none |
| Nice-to-have | 52 | tag `archive/prototype-turn-sync-activity` | none (labelled only) |
| Archive | 79 | branch `prototype/turn-sync-activity`; tags `v0.1.0`, `pre-layout`, three `archive/research-*` | delete 1 branch and 2 scripts, edit 1 script |

Unsure rows: 4 (the old-names proof scripts, `test-proof-scripts.sh`, the captured-game checklist).

## Root and build

| Path | Tier | Reason | Confidence | Inbound references | Next step |
|---|---|---|---|---|---|
| `Cargo.toml`, `Cargo.lock` | necessary | workspace manifest, version source; `--locked` release builds | confident | `release.yml`, `docs/maintainers/releasing.md`, `scripts/release-version.sh` | — |
| `.cargo/config.toml` | necessary | per-target rustflags for the cross builds | confident | `release.yml`, `docs/contributors/building.md`, ADR-0002 | — |
| `build-support/versioninfo.rs` | necessary | `include!`d by both `build.rs` files | confident | `crates/datalink-mp/build.rs`, `crates/dplayx/build.rs` | — |
| `.github/workflows/release.yml` | necessary | the release pipeline | confident | `docs/maintainers/releasing.md`, ADR-0002 | — |
| `.gitignore`, `.ignore` | necessary | keeps build output and captures out; hides archives from search | confident | `CLAUDE.md`, `CONTRIBUTING.md`, `docs/agents/issue-tracker.md` | — |
| `LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE-FONTS` | necessary | copied into every release archive; kept by ADR-0003 | confident | `release.yml`, `README.md`, `crates/datalink-mp/src/http.rs` | — |
| `README.md` | necessary | players' guide and GitHub front page | confident | `CONTRIBUTING.md`, `docs/contributors/building.md` | — |
| `packaging/README.txt` | necessary | shipped in each archive as `datalink-mp-README.txt` | confident | `release.yml`, `CLAUDE.md`, `docs/maintainers/releasing.md` | — |
| `CLAUDE.md`, `CONTRIBUTING.md`, `GLOSSARY.md` | necessary | agent entry point, contributor entry point, domain glossary | confident | `docs/agents/*`, every effort map | — |

## Crates

| Path | Tier | Reason | Confidence | Inbound references | Next step |
|---|---|---|---|---|---|
| `crates/dplayx/` | necessary | the shipped DLL (`cdylib`) | confident | `Cargo.toml`, `release.yml` | — |
| `crates/datalink-mp/` | necessary | the shipped Helper binary, its page, fonts and tests | confident | `Cargo.toml`, `release.yml` | — |
| `crates/datalink-transport/` | necessary | Iroh transport, including dev-only capture, path and Turn sync detection now on `main`; tests and fixtures | confident | `datalink-mp`, `datalink-mock-client` manifests | — |
| `crates/datalink-ipc/`, `crates/dp-types/` | necessary | IPC and DirectPlay types the DLL and Helper share | confident | `dplayx`, `datalink-mp` manifests | — |
| `crates/datalink-fixes/` | necessary | game patches and env-gated debug probes, linked into `dplayx` on Windows | confident | `crates/dplayx/Cargo.toml`, `crates/dplayx/src/lib.rs` | — |
| `crates/datalink-mock-client/` | necessary | workspace member; the interactive transport test client (no game needed) | confident | `docs/contributors/building.md`, `docs/contributors/architecture.md` | — |

## Scripts

| Path | Tier | Reason | Confidence | Inbound references | Next step |
|---|---|---|---|---|---|
| `scripts/release-version.sh` | necessary | called by the release pipeline | confident | `release.yml`, `docs/maintainers/releasing.md` | — |
| `scripts/launch-whisky.sh` | necessary | runs the game under Whisky for macOS testing | confident | `docs/contributors/building.md` | — |
| `scripts/check-links.py`, `scripts/links-allowlist.txt` | necessary | the link check CONTRIBUTING's Checks section asks for | confident | `CONTRIBUTING.md`, `docs/agents/issue-tracker.md` | — |
| `scripts/check-old-names.sh`, `scripts/old-names-allowlist.txt` | archive | proof script for repo-layout-build 04, which is finished. Its header points at CONTRIBUTING's Checks section, but that section no longer names it | **unsure**: keep it if you want a standing guard against the pre-layout names | `scripts/test-proof-scripts.sh`; `scripts/check-links.py` and `scripts/links-allowlist.txt` describe their allowlist format by pointing at `old-names-allowlist.txt` | delete both; drop their tests from `test-proof-scripts.sh`; write the allowlist format out in `check-links.py` and `links-allowlist.txt` instead of pointing at the deleted file |
| `scripts/test-proof-scripts.sh` | necessary | tests `check-links.py`, which is necessary; nothing runs it automatically | **unsure**: archive it too if a test for the link checker isn't wanted | none outside `scripts/` | — (edit only, see the row above) |

## Docs

| Path | Tier | Reason | Confidence | Inbound references | Next step |
|---|---|---|---|---|---|
| `docs/adr/` | necessary | decision records. ADR-0004 is superseded by ADR-0005 but kept, since superseded ADRs are never deleted | confident | `CONTRIBUTING.md`, `docs/agents/domain.md`, effort maps | — |
| `docs/agents/` | necessary | how agents use the tracker, labels and glossary | confident | `CLAUDE.md` | — |
| `docs/contributors/` | necessary | architecture, building, traffic capture (the capture ships on `main`) | confident | `CONTRIBUTING.md`, `README.md` | — |
| `docs/maintainers/releasing.md` | necessary | how releases are cut | confident | `CONTRIBUTING.md`, `release.yml` | — |
| `docs/maintainers/capture-test-checklist.html` | nice-to-have | the captured-game checklist. It serves post-0.1.0-polish 05 and game-session-sync 05, both still open; its third ticket (internet-play-speed 04) is resolved. It names the old `capture/traffic-capture` branch | **unsure**: it is effort material sitting in a maintainer folder, and could move into one of those efforts once they restart | `docs/archive/README.md` | — |
| `docs/players/` | necessary | player guides and the seven screenshots `README.md` links | confident | `README.md`, `docs/contributors/*` | — |
| `docs/release-notes/0.1.1.md` | necessary | notes for the version on `main`; `release.yml` reads `docs/release-notes/$VERSION.md` | confident | `release.yml`, `docs/maintainers/releasing.md`, internet-play-speed-build 06 | — |
| `docs/archive/` | archive | already archived: research write-ups, the 0.1.0 checklist, the WTP page. `smac-jackal-turn-sync.md` and `iroh-connection-stats.md` are still cited by `docs/contributors/traffic-capture.md` and internet-play-speed tickets; the links resolve and need no move | confident | `docs/archive/README.md` (its index) | none |

## Planning (`.scratch/`)

| Path | Tier | Reason | Confidence | Inbound references | Next step |
|---|---|---|---|---|---|
| `.scratch/README.md` | necessary | explains the tracker folder | confident | `CONTRIBUTING.md` | — |
| `.scratch/internet-play-speed-build/` | nice-to-have | 0.2.0 build effort: 01–06 done (shipped in 0.1.1), 07–18 open | confident | ADR-0005, `docs/contributors/traffic-capture.md` | — |
| `.scratch/internet-play-speed/` | nice-to-have | every ticket is resolved, but the open build tickets build from its map and analysis scripts, so it isn't finished under the repo's rule | confident | ADR-0005, internet-play-speed-build 09, 12, `docs/contributors/traffic-capture.md` | — |
| `.scratch/game-session-sync/` | nice-to-have | 05 is still `needs-triage` | confident | post-0.1.0-polish map, capture checklist | — |
| `.scratch/post-0.1.0-polish/` | nice-to-have | 05 and 06 are still `needs-triage` | confident | capture checklist | — |
| `.scratch/archive/` | archive | already archived: helper-web-ui, release-pipeline, repo-layout, repo-layout-build, ui-polish-0.1.1, web-ui-v1 | confident | ADRs, `docs/archive/README.md` | none |

## Branches and tags

| Ref | Tier | Reason | Confidence | Inbound references | Next step |
|---|---|---|---|---|---|
| branch `main` | necessary | the current version | confident | everything | — |
| branch `prototype/turn-sync-activity` | archive | local only; its tip is the same commit as tag `archive/prototype-turn-sync-activity`, so nothing is lost | confident | none (tickets cite the tag) | delete the local branch |
| tag `v0.1.1` | necessary | the current release; its GitHub release carries the shipped archives | confident | `docs/release-notes/0.1.1.md` | — |
| tag `archive/prototype-turn-sync-activity` | nice-to-have | the prototype that open internet-play-speed-build 12 builds from | confident | internet-play-speed-build 12, internet-play-speed 08 and map | — |
| tag `v0.1.0` | archive | the previous release | confident | `docs/archive/` | keep |
| tag `pre-layout` | archive | snapshot taken before the layout moves | confident | `docs/archive/README.md`, `scripts/check-old-names.sh` | keep |
| tags `archive/research-av-risk`, `archive/research-macos-artifact`, `archive/research-windows-toolchain` | archive | the research branches ADR-0002 cites, kept as tags | confident | `.scratch/archive/repo-layout-build/` | keep |
| remotes `origin/main`, `upstream/main` | necessary | publishing remote, and the upstream remote ADR-0003 keeps | confident | ADR-0003 | — |

## Move plan

Reviewed 2026-10-18: the unsure rows stand as marked. Steps 1 and 2 are done; step 3 waits on the efforts.

1. Delete the local branch `prototype/turn-sync-activity` (`git branch -D`, because it isn't merged; the tag keeps the commit).
2. If the unsure row stands: delete `scripts/check-old-names.sh` and `scripts/old-names-allowlist.txt`, remove their cases from `scripts/test-proof-scripts.sh`, describe the allowlist format directly in `scripts/check-links.py` and `scripts/links-allowlist.txt`, then run `scripts/test-proof-scripts.sh` and `scripts/check-links.py`.
3. No effort is ready to archive yet. When internet-play-speed-build closes, `internet-play-speed` and `internet-play-speed-build` move to `.scratch/archive/` together. The captured-game checklist follows whichever of game-session-sync and post-0.1.0-polish closes last.
