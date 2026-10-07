# Spec: the repo layout

Status: ready-for-agent

Source decisions: the [repo-layout map](map.md) and its tickets 01 to 11 (all resolved, 2026-10-06). The ledger of every path, branch and worktree is [ledger.md](ledger.md). Vocabulary: `CONTEXT.md` (Helper, DLL, Ticket, Game folder).

Where this spec and a ticket disagree, the ticket wins; each ticket's `## Answer` holds the detail and the inbound-reference lists this spec only points at. Unlike most specs, this one names paths, because the paths are the decision.

## Problem Statement

The repo has grown by accretion. A player opening it meets two READMEs at the root (`README.md` and `datalink-mp-README.txt`). A contributor finds one flat `docs/` mixing player guides, contributor guides, maintainer guides, release-testing material and research write-ups. `crates/smac-helper/` is an orphan holding one prototype HTML file, `tools/` holds a crate and a design page, and the internal crates carry names that mislead (`iroh-transport` reads like an iroh crate; `smac-fixes` carries a mark ADR-0003 avoided). Twenty local branches and nine worktrees pile up, most merged or abandoned. The checkout folder is still called `smac-iroh`, a legacy name from the upstream repo. A newcomer cannot tell what is live and what is history, and an agent searching the repo reads the history as if it were current.

## Solution

Reorganize the repo once, in one run, so that:

- the root serves **players** first, with one README and one clear step to **contributors'** docs;
- `docs/` is split by reader (players, contributors, maintainers, adr, agents, archive);
- everything not necessary (see the ledger) is archived, deleted or tagged, and a root `.ignore` hides archives from agent search;
- product-specific crates share a `datalink-` prefix and their folders follow their names;
- merged branches and all worktrees are gone, with tags where names are cited;
- the checkout folder is `datalink-mp`;
- nothing a player sees changes, and a scripted set of checks proves the release pipeline and the builds are intact.

Doing the moves is this spec's build; deciding them was the map's.

## User Stories

1. As a player, I want the repo root to hold one README, so that I know where to start.
2. As a player, I want the shipped `datalink-mp-README.txt` inside the release archive to keep its name, so that nothing I download changes.
3. As a player, I want release asset names, the GitHub repo name and the `datalink-mp` binary unchanged, so that links and habits keep working.
4. As a player, I want the faction-colour and Wine guides under `docs/players/`, so that I find them without reading contributor material.
5. As a player, I want the README's images to keep rendering, so that the front page isn't broken by the move.
6. As a new contributor, I want a root `CONTRIBUTING.md`, so that architecture, building and the `.scratch` workflow are one step away.
7. As a contributor, I want `docs/contributors/` to hold architecture, building and traffic-capture guides, so that I find build knowledge in one place.
8. As a maintainer, I want `docs/maintainers/` to hold the releasing guide and the capture-test checklist, so that release work has one home.
9. As a contributor, I want crate folders named after their crates, so that a path tells me the crate.
10. As a contributor, I want product-specific crates prefixed `datalink-` and `dplayx` and `dp-types` left alone, so that I can see which crates are this product.
11. As a contributor, I want `mock-dp-client` inside `crates/` as `datalink-mock-client`, so that no crate hides in `tools/`.
12. As a contributor, I want the orphan `crates/smac-helper/` gone, so that no folder looks like a crate and isn't.
13. As a contributor, I want lowercase kebab-case file names with no spaces, so that paths are predictable and shell-safe.
14. As a contributor, I want `.scratch/` kept public, tracked and at the same path, with a short `.scratch/README.md`, so that I learn what the planning folder is.
15. As a contributor, I want raw captures gitignored under `.scratch/**/captures/`, so that peers' IP addresses and iroh node IDs never reach the public repo.
16. As a contributor, I want finished efforts moved to `.scratch/archive/` and evidence docs to `docs/archive/`, so that live and finished work are separate.
17. As a contributor, I want `docs/archive/README.md` to index old path, new path or "deleted", and the tag to recover it from, so that I can find anything that was moved.
18. As an agent, I want a root `.ignore` listing both archives, so that searches don't return stale material.
19. As an agent, I want `CLAUDE.md` to say archives are hidden by `.ignore` and to link to the layout, so that I look in them deliberately.
20. As an agent, I want the "Repo conventions" section in `docs/agents/issue-tracker.md` (finished-effort rule, `.ignore`, research/prototype convention, text-only commits), so that I follow the workflow without asking.
21. As an agent, I want open build tickets, ADR-0005 and `docs/traffic-capture.md` to cite the new paths and crate names, so that tickets I pick up point at things that exist.
22. As a maintainer, I want an annotated `pre-layout` tag pushed before any move, so that I have a rollback point.
23. As a maintainer, I want three research branches ADR-0002 names tagged `archive/research-<name>`, and `prototype/turn-sync-activity` tagged `archive/prototype-turn-sync-activity`, so that cited names still resolve.
24. As a maintainer, I want all nine worktrees pruned and the merged branches (local and `origin/`) deleted, so that `git branch` shows only live work.
25. As a maintainer, I want the three unmerged research write-ups merged to `main` before the moves, so that no evidence lives only on this machine.
26. As a maintainer, I want the checkout folder renamed to `datalink-mp` last, with Claude Code's project-memory directory moved with it, so that nothing is repaired mid-build.
27. As a maintainer, I want the build gate, grep, link check, packaging replay, throwaway-tag dry run and archive comparison run before merging, so that I know the release pipeline still works.
28. As a maintainer, I want to confirm every outward-facing step (pushing tags, deleting `origin/` branches, the dry-run tag), so that nothing public happens unasked.

## Implementation Decisions

### Preconditions and ordering (tickets 08, 09)

- The layout build runs **between Patch A and the code tickets** of `internet-play-speed-build`. It must not start until build ticket 02 (merges `capture/traffic-capture`, which is then deleted) is merged and ticket 06 (Patch A release) is done. Starting earlier is a new decision.
- Order of steps: (1) merge the three unmerged research write-ups to `main`; tag `prototype/turn-sync-activity`; (2) push the annotated `pre-layout` tag on the Patch A release commit; take the baseline build and test run on it; (3) prune worktrees and delete merged branches, after tagging the three ADR-named research branches; (4) the moves, deletions and crate renames, on one layout branch; (5) path rewrites in open tickets, ADR-0005 and `docs/traffic-capture.md`, in the same commit as the moves; (6) checks; (7) throwaway-tag dry run; (8) merge to `main`; (9) rename the checkout folder.
- Pushing `main` (7 commits ahead of `origin`) is the maintainer's call outside this build.

### Target tree (ticket 05)

```
README.md  CONTRIBUTING.md (new)  CONTEXT.md  CLAUDE.md
LICENSE-MIT  LICENSE-APACHE  LICENSE-FONTS
Cargo.toml  Cargo.lock  .gitignore  .ignore (new)  .cargo/  .github/
packaging/README.txt          (was datalink-mp-README.txt; still ships under that name)
build-support/versioninfo.rs  scripts/
crates/   datalink-mp  dplayx  dp-types  datalink-ipc  datalink-transport  datalink-fixes  datalink-mock-client
docs/     players/ (faction-colors.md, wine-compatibility.md, images/)
          contributors/ (architecture.md, building.md, traffic-capture.md)
          maintainers/ (releasing.md, capture-test-checklist.html)
          adr/  agents/  archive/ (README.md, research/ x7, 0.1.0-release-checklist.html, wtp-through-the-ages.html)
.scratch/ README.md (new), efforts, archive/
```

`tools/` disappears. Names are lowercase kebab-case; only GitHub-special root files stay uppercase. The move table with every inbound reference to rewrite is in [ticket 05](issues/05-target-folder-tree.md); follow it row by row. The three research write-ups merged in step 1 join the four existing ones in `docs/archive/research/`, each with a row in the archive index, and `docs/contributors/traffic-capture.md`'s link to the jackal write-up is rewritten.

### Ledger actions (tickets 01, 03, 08)

- **Delete:** `docs/capture_test_launch.txt` (confirm `traffic-capture.md` carries the launch command first), `docs/datalink-mp-rc6-test-checklist.html`, `docs/datalink-mp rc4 tests.md`, `crates/smac-helper/` (branch `prototype/ui-flow` holds it).
- **Archive:** release-pipeline and ui-polish-0.1.1 to `.scratch/archive/` (the two existing archives stay). Links into `.scratch/archive/web-ui-v1/` from ADR-0001, 0002 and 0003 stay valid because that path is unchanged. `.scratch/internet-play-speed/` is archived later, when `internet-play-speed-build` finishes; that is not part of this build.
- **Branches:** delete the 15 merged local branches (`prototype/ui-flow`, four `research/*`, ten `worktree-agent-*`) and the 5 `origin/` copies (maintainer confirms). Tag first the three ADR-0002 research branches. Keep `main` and the live unmerged work; `capture/traffic-capture` is deleted by the build effort, not here.
- **Worktrees:** `git worktree prune` plus removal of all nine; this clears the two stale `smac-helper-gui` entries.

### Crate renames (ticket 06)

`ipc-protocol` becomes `datalink-ipc`, `iroh-transport` becomes `datalink-transport`, `smac-fixes` becomes `datalink-fixes`, `mock-dp-client` becomes `datalink-mock-client`. Folders follow names, with `git mv` in the same commit as manifest edits, then `Cargo.lock` regenerates and is committed before any `--locked` build. Root `members` becomes `crates/*`; the workspace path dependencies and each manifest are edited as listed in [ticket 06](issues/06-crate-renames.md), as are the nine-file `use` rewrites for `ipc_protocol` and `iroh_transport` and the `smac_fixes` calls. Silent breaks that the compiler will not catch: the `"iroh_transport=debug"` log directive in the Helper's `main.rs` must become `datalink_transport=debug`; the `smac-fixes:` prefix in the `probe.rs` eprintln; any `RUST_LOG` example in docs. Accepted ADRs get only crate-path edits. Archived material keeps old names.

### Release workflow

`release.yml` is edited once: its three `cp` lines copy `packaging/README.txt` into the archive as `datalink-mp-README.txt`, and its comment points at `docs/maintainers/releasing.md`. `-p dplayx` and `-p datalink-mp` are unchanged. `LICENSE-*` stay at the root, `build-support/` stays, and `crates/datalink-mp/src/` is not restructured, so the `include!`/`include_str!` paths hold.

### Agent pointers (ticket 11)

`docs/agents/issue-tracker.md` gains one "Repo conventions" section: the finished-effort rule (all tickets resolved or closed and no open ticket elsewhere cites it as the spec to build from; the closing step is moving it to `.scratch/archive/` and fixing inbound links), the root `.ignore`, the research/prototype convention (findings land on `main` at ticket close; prototype tips are tagged `archive/prototype-<name>`, pushed, branch deleted; ticket pointers never name a branch), and text-only commits with captures gitignored. The "Wayfinding operations" section is untouched. `CONTRIBUTING.md` summarises and links to it. `CLAUDE.md` gains a short "Layout" section of links plus the `.ignore` warning, without crate names. No `AGENTS.md` bridge. `docs/agents/domain.md` and `triage-labels.md` are unchanged.

### Checkout folder (ticket 07)

Last step, outside git, with Claude Code closed: rename `~/Documents/code/datalink-mp/smac-iroh` to `~/Documents/code/datalink-mp/datalink-mp`, and move the matching key directory under `~/.claude/projects/`. The project's memory directory holds no memory entries (checked), so nothing names an invalidated path. Verify with `git status`, `git worktree list`, a build, and a new Claude Code session seeing the old history. Older orphaned keys are left alone.

### Proof (ticket 10)

Five checks, run by the agent after each file-moving step and at the end, except where the maintainer is named:

1. **Build gate:** the exact `release.yml` commands (`cargo build --release --locked --target i686-pc-windows-gnu -p dplayx`, `-p datalink-mp`), plus `cargo test --workspace` and `cargo clippy`, baselined on `pre-layout`; a `Cargo.lock` diff shows only crate-name changes.
2. **Old-name grep with allowlist,** over every old crate name (hyphen and underscore), `datalink-mp-README.txt` and `tools/`, plus a runtime check that the Helper at debug level still logs from the renamed transport crate. Non-allowlisted hits go to the maintainer.
3. **Link check:** a reusable script in `scripts/` resolving every relative markdown link and backticked path in README, CONTRIBUTING, `docs/`, ADRs, `CLAUDE.md` and `.scratch/**`. Branch-pointing links are listed for manual review.
4. **Release workflow:** read-through against new paths, a local replay of the packaging steps, then a real dry run: the maintainer confirms pushing a throwaway pre-release tag (e.g. `v0.0.0-layout-test`) from the layout branch; the agent checks the draft Release; the tag and draft are then deleted.
5. **Archive contents:** the maintainer compares the draft's Windows, Linux and macOS file lists to the last real release. Expected identical names. Any difference fails.

Rollback: before the dry run, fix forward. Reset the layout branch to `pre-layout` and re-plan if a failure remains unexplained after one fix attempt or a dependency version changed. If archive contents differ and it is not a trivial workflow fix, do not merge. After merge to `main`, restore by revert commit, never rewritten history. Merge to `main` only after all checks pass and the dry run succeeds.

## Testing Decisions

- **Seam:** a single seam, the proof gate above (build, grep, link check, packaging replay, archive comparison). It was chosen by the maintainer in ticket 10, so it is not re-proposed here. A good check observes external behaviour: the build produces the same artifacts, links resolve, release archives contain the same files. It never asserts how files were moved.
- **Existing tests:** the workspace tests (`cargo test --workspace`) are the regression net for the crate renames. They are not modified except for `use` paths.
- **New tooling:** two scripts in `scripts/` (old-name grep with allowlist; link check) that outlive this build. The link check is reusable by every later effort that archives or moves files.
- **Prior art:** the release workflow's own commands (`release.yml`) are the build gate; `scripts/release-version.sh` is the model for a small repo script.

## Out of Scope

- Restructuring code inside crates (modules, file splits).
- Renaming anything a player sees: release asset names, the GitHub repo name, the `datalink-mp` binary.
- Gitignored build output (`target/`).
- Archiving `.scratch/internet-play-speed/` (done when `internet-play-speed-build` finishes).
- Merging or deleting `capture/traffic-capture` (build ticket 02) and pushing `main`.
- An `AGENTS.md` bridge.
- Older orphaned Claude Code project keys, and the outer `datalink-mp/second.log`.

## Further Notes

- Maintainer-confirmed outward-facing steps: pushing `pre-layout` and the archive tags, deleting `origin/` branches, the throwaway release tag.
- Ticket 05's tree lists four research write-ups in `docs/archive/research/`; the three merged in step 1 make seven. The archive index must list all of them.
- The ledger marks each row confident or unsure; the maintainer's answers to the five unsure rows are recorded in it.
