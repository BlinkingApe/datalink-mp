# 07: Rewrite paths in open tickets and live docs

**What to build:** Every ticket an agent will pick up next, and the docs it cites, name paths and crates that exist.

**Blocked by:** 05, 06

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Open internet-play-speed-build tickets, ADR-0005, `docs/contributors/traffic-capture.md` and live-effort prose (post-0.1.0-polish, game-session-sync, internet-play-speed) use new paths and crate names; archived efforts keep old names
- [x] Build tickets' pointers to research or prototype read as doc paths or the `archive/prototype-` tag, never a branch
- [x] The link-check script and the grep script report nothing outside the allowlist for these files

## Comments

2026-10-07 (agent). Built with `scripts/check-old-names.sh` and `scripts/check-links.py` as the red/green signal.

**Red** (layout tip `3c21c8a`): old names **19** not allowlisted / 162 allowlisted. Links **39** broken / 409 allowlisted / 22 branch references. All were in the files 05 and 06 left for this ticket.

**Rewritten** (25 files in game-session-sync, internet-play-speed, internet-play-speed-build and post-0.1.0-polish, done tickets included). ADR-0005 had no hits, and 05 had already fixed `docs/contributors/traffic-capture.md`.

- Crate paths: `crates/iroh-transport/...` → `crates/datalink-transport/...`, `cargo test -p iroh-transport` → `-p datalink-transport`, and "smac-fixes" → "datalink-fixes" in the internet-play-speed map.
- Docs: `docs/traffic-capture.md`, `building.md` and `ARCHITECTURE.md` → `docs/contributors/...`; `docs/releasing.md` → `docs/maintainers/releasing.md`; `docs/research/` → `docs/archive/research/`. This includes the markdown link in `analysis/direct-game-capture.md` and the docstring path in `analysis/analyse_capture.py`. That script and the other four analysis scripts still compile (`py_compile`).
- `datalink-mp-README.txt` → `packaging/README.txt` (game-session-sync 04, post-0.1.0-polish 02 and its map): the file to edit is the source, which still ships under the old name.
- The game-session-sync map's `../release-pipeline/issues/` → `../archive/release-pipeline/issues/`.
- `.scratch/repo-layout/issues/09` line 15: `../internet-play-speed-build/` → `../../internet-play-speed-build/`, which was one `../` short.

**Branch pointers.** All 17 in these files are gone.

- Research: "Research: branch `research/<name>` (commit …), file `docs/research/<name>.md`" → "Research: `docs/archive/research/<name>.md` (commit …)". The "(on) branch `research/…`" asides after a doc path are dropped (internet-play-speed 01, 02 and the analysis notes).
- Prototype: internet-play-speed 08 and the map point at tag `archive/prototype-turn-sync-activity`. 08 names the prototype's `serve.py`.
- `capture/traffic-capture` is merged into `main`. Its pointers now name commit `b90a677` (reachable from `main`) or link to internet-play-speed 03, which built it: internet-play-speed 03, 04 and the map; internet-play-speed-build 02; game-session-sync 05; post-0.1.0-polish 05. Internet-play-speed 04's "ran the capturing build from `capture/traffic-capture`" now reads "from ticket 03".

**Allowlist entry added** to `scripts/links-allowlist.txt`, with a comment: internet-play-speed 09's `crates/datalink-transport/src/early_ack.rs`, a planned module that internet-play-speed-build 07 creates. No old-names allowlist entry was needed.

**Green:**

- Old names: **0** not allowlisted / 162 allowlisted, exit 0.
- Links: **0** broken / 409 allowlisted / **5** branch references, exit 0. The 5 are outside this ticket's files and are meant to stay. ADR-0001:59 names `prototype/ui-flow` and ADR-0002:6 names three `research/*` branches; both are accepted records, and ADR-0002's three resolve through the `archive/research-*` tags. `docs/release-notes/0.1.1.md:16` is a `blob/main` URL that resolves once `layout` reaches `main`.
- `scripts/test-proof-scripts.sh`: all 16 pass.

**Noted, not changed:** the live maps and tickets still say `CONTEXT.md` in places, such as the "Domain:" lines in the internet-play-speed, game-session-sync and post-0.1.0-polish maps. That file became `GLOSSARY.md` before the layout spec, which doesn't cover the rename, and the link check skips bare file names. A follow-up could repoint the "Domain:" lines.
