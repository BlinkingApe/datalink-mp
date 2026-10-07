# 08: Contributor and agent docs

**What to build:** A newcomer, a contributor and an agent each find their entry point, and the conventions are written down where they will be read.

**Blocked by:** 05

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Root `CONTRIBUTING.md` covers architecture, building, and how `.scratch` works (map, tickets, specs, `Status:` lines, archive, text-only, raw captures stay local), linking to the `docs/` guides
- [x] `.scratch/README.md` is a few lines pointing at that section; `.gitignore` has `.scratch/**/captures/`
- [x] `CLAUDE.md` gains a short Layout section of links and the warning that `.ignore` hides archives; no crate names
- [x] `docs/agents/issue-tracker.md` gains a Repo conventions section: finished-effort rule, `.ignore`, research/prototype convention, text-only commits; Wayfinding operations is untouched; `domain.md` and `triage-labels.md` are unchanged; no `AGENTS.md`

## Comments

2026-10-07 (agent). Built against the agreed checks: `git check-ignore`, `check-links.py`, `check-old-names.sh` and a README grep. Each was run red before its slice and green after.

**Changed:**

- `.gitignore` gains `.scratch/**/captures/`, with a one-line reason.
- New `CONTRIBUTING.md` covers architecture, building, a table of where docs live, how `.scratch/` works, and checks. It links to `docs/contributors/*`, `docs/maintainers/releasing.md`, `docs/adr/`, `docs/archive/README.md` and `docs/agents/issue-tracker.md`.
- New `.scratch/README.md`: a few lines pointing at CONTRIBUTING's `.scratch` section.
- `README.md` gains a short "Contributing" section before Credits.
- `CLAUDE.md` gains a "Layout" section (code, docs, shipped README, archives, plus the `.ignore` warning), with no crate names.
- `docs/agents/issue-tracker.md` gains "Repo conventions" (finished efforts, `.ignore`, research and prototypes, text only), inserted before "Wayfinding operations", which is byte-identical. `domain.md` and `triage-labels.md` are unchanged; no `AGENTS.md`.

**Decided here:**

- CONTRIBUTING names no crates. It says crate folders follow crate names and `Cargo.toml` lists them, and it links to the repository-layout table in `docs/contributors/building.md`, which ticket 06 updates.
- The research/prototype rule says the maintainer confirms pushing the prototype tag, because pushing is outward-facing.
- `.scratch/internet-play-speed/.gitignore` (`captures/`) is now redundant with the root rule. It is left in place: it is harmless and that effort is live.

**Proof:**

- `git check-ignore --no-index`: before, `.scratch/foo/captures/x.pcap` was not ignored. After, it and `.scratch/internet-play-speed-build/captures/a/b.log` are ignored by `.gitignore:6`. The tracked `.scratch/internet-play-speed/analysis/direct-game-capture.md` and `analyse_capture.py` are not ignored.
- `check-links.py`: before 23 broken / 391 allowlisted / 22 branch refs; after **23 / 386 / 22**. The five fewer allowlisted are backticked `CONTRIBUTING.md` and `.scratch/README.md` mentions in repo-layout tickets, which now resolve. On the six changed files: 0 broken, 0 allowlisted, 0 branch refs. This comment adds 2 allowlisted (the made-up capture paths above, excused by the `captures/` entry), so the full run now reads 23 / 388 / 22.
- `check-old-names.sh`: 134 / 157 before and after, identical output; 0 hits in the changed files.
- `README.md` links `CONTRIBUTING.md` (Contributing section). `scripts/test-proof-scripts.sh`: all pass.
