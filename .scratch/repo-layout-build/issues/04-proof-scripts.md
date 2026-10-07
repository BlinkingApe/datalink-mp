# 04: Proof scripts

**What to build:** Two reusable scripts the later tickets run to show nothing broke: an old-name grep with an allowlist, and a link checker.

**Blocked by:** 02

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Grep script searches tracked files for every old crate name (hyphen and underscore), `datalink-mp-README.txt` and `tools/`, and fails on any hit not on the allowlist (ADRs, archived history)
- [x] Link-check script resolves every relative markdown link and backticked path in README, CONTRIBUTING, `docs/`, ADRs, `CLAUDE.md` and `.scratch/**`, and lists links that point at a branch separately
- [x] Both scripts live in `scripts/`, are documented in a header comment, and have been run against the baseline with the output recorded in the ticket

## Comments

2026-10-07. Built test-first; `scripts/test-proof-scripts.sh` runs 16 tests against throwaway git repos.

**Scripts** (each documented in its header):

- `scripts/check-old-names.sh [--all] [<pathspec>...]`: `git grep` over tracked files for `ipc-protocol`, `iroh-transport`, `smac-fixes`, `mock-dp-client` (hyphen and underscore forms), `datalink-mp-README.txt` and `tools/`. An old name must start at a word edge, so `datalink-transport`, `devtools/` and `rust-tools/` never match. Prints `path:line: text` for every hit not on the allowlist, then a count. Exits 1 if any are left. `--all` also prints allowlisted hits.
- `scripts/check-links.py [--all] [<path>...]`: covers the tracked root `*.md`, `docs/**/*.md` and `.scratch/**/*.md`. It resolves markdown links, reference definitions, `src=`/`href=` and backticked repo paths against the index, so stage moves before checking. The backtick check is a heuristic: a token must contain a `/` and start with `./`, `../`, a tracked top-level name or a tracked folder name. Bare file names and fenced code are skipped. It prints broken links, then branch references prefixed `branch ` (backticked `research/`, `prototype/`, `capture/`, `origin/`, `upstream/`, `worktree-agent-` names, and GitHub tree/blob URLs into a branch of this repo's remotes), then a count. Exits 1 only on broken links.
- Allowlists: `scripts/old-names-allowlist.txt` and `scripts/links-allowlist.txt`, with one format: `<glob> [<regex>]`. A `*` in the glob also matches `/`. The regex is optional and must match the hit's line or the link target. A `!<glob> [<regex>]` entry is a deny that beats any allow. The old-names list allows `docs/adr/*`, with a deny for `crates/` and `tools/` paths of the old crates (ADRs get crate-path edits only). It also allows `.scratch/archive/*`, `docs/archive/*`, `.scratch/repo-layout/*`, `.scratch/repo-layout-build/*` and the proof scripts themselves. The links list allows the same archives and repo-layout folders, plus gitignored `captures/` folders.

**Baseline** (run on this branch, whose content outside `scripts/` and this effort's tickets matches `pre-layout`):

`check-old-names.sh`: exit 1, **167 hits not on the allowlist, 73 allowlisted**.

| Area | Hits |
|---|---|
| Code and manifests: `crates/` (datalink-mp 41, iroh-transport 26, smac-fixes 9, dplayx 6, ipc-protocol 1), `Cargo.lock` 9, `Cargo.toml` 6, `tools/mock-dp-client` 4 | 105 |
| `.github/workflows/release.yml` (shipped README name) | 3 |
| Docs: `docs/research/` 21 (moves to `docs/archive/`), `ARCHITECTURE.md` 7, `building.md` 4, `traffic-capture.md` 1, 0.1.0 checklist 1, ADR-0001 crate path 1 | 35 |
| Live `.scratch` efforts: release-pipeline 6, internet-play-speed 6, game-session-sync 6, post-0.1.0-polish 5, ui-polish-0.1.1 2, internet-play-speed-build 2 | 27 |

Representative lines:

```
crates/datalink-mp/src/main.rs:93:             .add_directive("iroh_transport=debug".parse().unwrap())
crates/smac-fixes/src/probe.rs:351:                 eprintln!("smac-fixes: failed to open probe log {}: {}", path, e);
Cargo.toml:10:     "tools/mock-dp-client",
.github/workflows/release.yml:97:             cp datalink-mp-README.txt "$d/"
docs/adr/0001-web-ui-frontend.md:163: 7. `cargo test` still passes, including the mesh test in `crates/iroh-transport`.
docs/building.md:151: | `tools/mock-dp-client` | interactive transport test client (no game needed) |
```

`check-links.py`: exit 1, **27 broken links not on the allowlist, 192 allowlisted, 23 branch references to review**.

- 26 of the 27 are in `docs/research/`. They name files in other projects (`src/engine.cpp`, `crates/searcher`, `docs/INSTALL.md`), deleted files (`crates/smac-helper/...`, `crates/dplayx/unwind_stubs.c`) or example layouts. Ticket 05 moves them under `docs/archive/`, which is allowlisted.
- 1 is live: `.scratch/internet-play-speed/issues/09-helper-acks-for-the-friends-game.md:47` cites `crates/iroh-transport/src/early_ack.rs`, a file that ticket plans but that doesn't exist yet.
- Two real broken markdown links are allowlisted as archive or repo-layout history: `.scratch/archive/web-ui-v1/issues/08-…md:23` → `../../../../crates/smac-helper/src/main.rs`, and `.scratch/repo-layout/issues/09-…md:15` → `../internet-play-speed-build/` (one `../` short).
- Branch references: `capture/traffic-capture` ×8 across internet-play-speed, internet-play-speed-build 02, game-session-sync 05 and post-0.1.0-polish 05; `research/smac-jackal-turn-sync` and `research/iroh-connection-stats` ×7 in internet-play-speed and `docs/traffic-capture.md:80`; `prototype/turn-sync-activity` ×3 in internet-play-speed; `prototype/ui-flow` in ADR-0001; three `research/*` names in ADR-0002 (kept by the archive tags); and one URL, `docs/release-notes/0.1.1.md:16` → `https://github.com/BlinkingApe/datalink-mp/blob/main/docs/traffic-capture.md`. That URL breaks once 05 moves `traffic-capture.md`, and release notes are published.
- Bare file names (`CONTEXT.md` ×26, `AGENTS.md`, `unwind_stubs.c`, …) are not checked; on the baseline they were mostly other projects' files and stale prose. The link check also skips a backticked path whose first folder exists nowhere in the tree, such as `iroh-transport/src/x.rs` after 06. The grep catches those.

**For later tickets:** 05 needs a line-regex entry for `release.yml`, whose `COMMON=` and `cp` lines keep the shipped name `datalink-mp-README.txt`. For example: `.github/workflows/release.yml   packaging/README\.txt|COMMON=`. 05 may also need one for `docs/maintainers/releasing.md` if it names the shipped file. Rerun both scripts after `git mv`/`git add`, since the link check reads the index.
