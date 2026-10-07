# Map: Repo layout

Label: wayfinder:map

## Destination

A decided **Repo layout spec**, handed to `/to-tickets`. It holds a ledger marking every path, branch and worktree necessary or not, the target folder tree (crate names included), and the full list of moves, renames, archive and delete steps, plus how to prove nothing broke. Doing the moves is a separate build effort.

## Notes

- Domain: `CONTEXT.md` for product terms (Helper, DLL, Ticket, Game folder). [ADR-0003](../../docs/adr/0003-standalone-project.md) left crate renames open and keeps the `upstream` remote and both licence files. [ADR-0002](../../docs/adr/0002-releases-versioning-pipeline-trust.md) and `docs/releasing.md` cover the release pipeline the moves must not break.
- Tracker: local markdown, this directory.
- Skills for grilling tickets: `grilling` + `domain-modeling`.
- **Plan, don't do.** No file is moved by this map.

### Decided while charting

- **Scope:** tracked files, untracked files in the working copy, local and remote git branches, and worktrees. Gitignored build output (`target/`) is out.
- **Readers, in layers:** the root and README serve **players** first. **Contributors** get one clear entry point (architecture, building, how the `.scratch`/ADR workflow runs) one step away. **Agents** count as readers too: `CLAUDE.md` and `docs/agents/` name fixed paths.
- **Necessary means** a path does one of three things. (1) It builds, tests or ships the product: code, manifests, CI, scripts, release inputs, licences, and assets the README links to. (2) It's a living doc a human or agent reads to work today: README, `CONTEXT.md`, ADRs, architecture, building and releasing guides, `docs/agents/`. (3) It's planning for an active effort (a `.scratch` effort with open tickets). Anything else is a candidate for archiving or deleting.
- **Reorganize means** moving and renaming files and folders, **and renaming crates**. Restructuring code inside crates is out. Names players see stay as they are: release asset names, the GitHub repo name and the `datalink-mp` binary.
- **The local checkout folder** (`smac-iroh`, a legacy name from the upstream repo) is renamed too.
- **Timing:** plan now. Whether to do the moves before or after `internet-play-speed-build` is decided on this map.
- **Ledger review:** the agent marks each row *confident* or *unsure*. The maintainer reviews only the unsure rows, plus a summary by category.
- **Must-address pain points:** the `smac-iroh` folder name, the root `datalink-mp-README.txt` next to `README.md`, the flat mixed `docs/`, and the orphan `crates/smac-helper/` (it holds only a prototype HTML file).

## Decisions so far

<!-- one line per closed ticket -->

- [Conventions for a repo layout a newcomer can follow](issues/02-layout-conventions.md): the `crates/` layout already fits Cargo's recommendation, apart from two stray HTML files. Keep ADRs and never delete superseded ones. Add a root `CONTRIBUTING.md` and put maintainer docs one level down. Moves break links that point at a branch. A `.ignore` file hides archives from agent search. `iroh-transport` is the one crate name that misleads. An `AGENTS.md` bridge is optional.
- [Build the classification ledger](issues/01-classification-ledger.md): done as [`ledger.md`](ledger.md). Beyond code, build files and living docs, the necessary paths are the captured-game checklist, five `.scratch` efforts (both post-0.1.0-polish and game-session-sync are live), and five unmerged, unpushed branches. Everything else is a candidate: the smac-helper prototype, the WTP page, four research write-ups, past RC material, release-pipeline, ui-polish-0.1.1 and the archives, the merged branches and all nine worktrees.

- [How to archive what isn't necessary](issues/03-how-to-archive.md): efforts go to `.scratch/archive/` and evidence docs to `docs/archive/`, with a root `.ignore` hiding both from search. Throwaway files are deleted. A pushed `pre-layout` tag is the first step, and `docs/archive/README.md` is the index. Merged branches and all worktrees are removed, with tags for the three research branches ADR-0002 names. An effort is finished when its tickets are done and no open ticket builds from it.
- [Does `.scratch/` planning history stay in the public repo?](issues/04-scratch-public-or-not.md): yes, tracked and public, same name and path. CONTRIBUTING explains it and a short `.scratch/README.md` points there. Text only is committed, and raw captures are gitignored under `.scratch/**/captures/`. Effort-specific files live in the effort's folder, lasting maintainer guides in `docs/maintainers/`.
- [The target folder tree](issues/05-target-folder-tree.md): root keeps README, licences, CONTEXT, CLAUDE, plus new CONTRIBUTING; the shipped README moves to `packaging/README.txt`. `docs/` splits into `players/`, `contributors/`, `maintainers/`, `adr/`, `agents/`, `archive/`. `tools/` goes (mock-dp-client joins `crates/`); `scripts/` and `build-support/` stay. Lowercase kebab-case names. Full move list with inbound references in the ticket.
- [Crate renames](issues/06-crate-renames.md): product-specific crates take a `datalink-` prefix and folders follow crate names: `ipc-protocol`->`datalink-ipc`, `iroh-transport`->`datalink-transport`, `smac-fixes`->`datalink-fixes`, `mock-dp-client`->`datalink-mock-client`. `dplayx` (DLL) and `dp-types` (generic) stay. The release workflow needs one edit, for the README path 05 moves (see 10); the one silent break is the `iroh_transport=debug` log directive in the Helper's `main.rs`.
- [Rename the local checkout folder](issues/07-local-checkout-folder.md): `smac-iroh` becomes `~/Documents/code/datalink-mp/datalink-mp`, done last in the build, after worktree cleanup. Move Claude Code's project-memory key dir with it, and update the one absolute path in `docs/capture_test_launch.txt`. Tracked files hold no checkout path.
- [Branch and worktree policy](issues/08-branch-and-worktree-policy.md): 03 already covers merged branches and worktrees. The three unmerged research write-ups merge to `main` first and are then filed like other evidence docs. `prototype/turn-sync-activity` gets a pushed `archive/prototype-` tag. Going forward findings land on `main` at ticket close, prototypes are tagged, and no ticket pointer names a branch.
- [When to do the moves](issues/09-when-to-do-the-moves.md): between Patch A and the code tickets. Build 01–06 land first (the capture branch merges, Patch A ships), then the whole layout build runs in one go, rewriting the open tickets' paths in the same commit, then build 07–18 continues on the new tree.
- [How to prove the layout build broke nothing](issues/10-prove-nothing-broke.md): five checks: build/test/clippy gate against a `pre-layout` baseline, a scripted old-name grep with allowlist plus a runtime check of the `iroh_transport` log directive, a reusable link-check script, a read-through and packaging replay of `release.yml` followed by a real throwaway-tag dry run (maintainer confirms), and a comparison of release archive contents. `release.yml` is edited for the README path. Fix forward before the dry run; reset to `pre-layout` after one failed fix, dependency drift or archive differences that aren't a trivial fix. Merge to `main` only after the dry run.
- [Agent pointers after the moves](issues/11-agent-pointers.md): no existing agent path goes stale. `docs/agents/issue-tracker.md` gains one "Repo conventions" section (finished-effort and archive, `.ignore`, research/prototype, captures gitignored); `CLAUDE.md` gains a short "Layout" section of links plus the `.ignore` warning. No `AGENTS.md` bridge; `domain.md` and `triage-labels.md` unchanged. No memory entries exist to invalidate.

## Not yet specified

<!-- empty: all fog has graduated into tickets and every ticket is resolved -->

## Out of scope

- **Restructuring code inside crates** (modules, file splits). A separate effort.
- **Renaming names players see:** release asset names, the GitHub repo name, the `datalink-mp` binary.
- **Gitignored build output** (`target/`).
