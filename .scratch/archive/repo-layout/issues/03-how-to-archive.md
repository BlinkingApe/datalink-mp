# 03: How to archive what isn't necessary

Type: grilling
Status: resolved
Blocked by: 01

## Question

What happens to each path the ledger marks *not necessary*? Candidates:

- an in-repo archive folder (as `.scratch/archive/` already does for efforts)
- delete it from the tip, and keep it reachable through git history plus an annotated tag (such as `pre-layout`) and an index of what went where
- move it to a separate branch or repo

The answer may differ by kind: finished `.scratch` efforts, past RC and test checklists, prototypes, research write-ups whose decisions live in an ADR, and stale branches. Weigh how findable each needs to stay (ADRs link into finished efforts' tickets), how much it clutters a newcomer's view, and what agents will search by mistake.

## Answer

Decided with the maintainer, 2026-10-06. They accepted every recommendation.

- **Finished `.scratch` efforts** (release-pipeline, ui-polish-0.1.1, and the two already archived) live in the repo under `.scratch/archive/`. `.scratch/archive/` keeps its path, so the ADR-0001, 0002 and 0003 links into `archive/web-ui-v1/` stay valid. A root `.ignore` hides archives from ripgrep-based search, and `docs/agents/issue-tracker.md` writes the convention down.
- **Loose not-necessary files**, by kind:
  - *Delete:* `docs/capture_test_launch.txt`, `docs/datalink-mp-rc6-test-checklist.html`, `docs/datalink-mp rc4 tests.md`, and `crates/smac-helper/prototype/ui-flow-prototype.html` (branch `prototype/ui-flow` already holds it).
  - *Archive in `docs/archive/`:* the four `docs/research/*.md` write-ups, `docs/datalink-mp-0.1.0-release-checklist.html`, and `tools/0001-wtp-through-the-ages.html`.
- **Two archive folders, not one:** `.scratch/archive/` for efforts and `docs/archive/` for evidence docs. `.ignore` lists both.
- **Branches and worktrees:** prune all 9 worktrees. Delete the merged local branches (`prototype/ui-flow`, 4 `research/*`, 10 `worktree-agent-*`). Delete their `origin/` copies too, as an explicit maintainer-confirmed step in the build, since it is outward-facing. Before deleting, tag the tips of the three `research/*` branches that ADR-0002 names (for example `archive/research-windows-toolchain`) so name-based references still resolve. The other branches get no tags.
- **Safety net:** an annotated tag `pre-layout`, pushed to `origin`, on the commit before any move. It is the first build step and the rollback point. `docs/archive/README.md` is the index: a table of old path, new path or "deleted", and the tag to recover it from. It also lists what is in `.scratch/archive/`.
- **ADR-0005's trail:** `.scratch/internet-play-speed/` is archived like any finished effort once `internet-play-speed-build` finishes. The ADR-0005 links are rewritten in the same commit, and the link check in the proof step catches misses. This is a note on that build, not a task of the layout build.
- **Finished-effort rule**, written into `docs/agents/issue-tracker.md`: an effort is finished when every ticket is resolved or closed and no open ticket elsewhere cites it as the spec to build from. Moving it to `.scratch/archive/` and fixing inbound links is the closing step of every effort.
