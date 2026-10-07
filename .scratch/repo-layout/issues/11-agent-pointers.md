# 11: Agent pointers after the moves

Type: grilling
Status: resolved

## Question

Which agent-facing files change after the layout build, and what exactly do they say? Candidates:

- `CLAUDE.md` and `docs/agents/*` (`domain.md`, `issue-tracker.md`, `triage-labels.md`) name fixed paths; check each against [The target folder tree](05-target-folder-tree.md).
- The `.scratch/<effort>/` convention in `docs/agents/issue-tracker.md`: now with `.scratch/archive/`, the root `.ignore`, the finished-effort rule ([How to archive what isn't necessary](03-how-to-archive.md)) and the research/prototype convention ([Branch and worktree policy](08-branch-and-worktree-policy.md)).
- Is the optional `AGENTS.md` bridge from [Conventions for a repo layout a newcomer can follow](02-layout-conventions.md) added?
- Claude Code's per-folder project memory, keyed by the checkout path: [Rename the local checkout folder](07-local-checkout-folder.md) moves the directory. Do any memory entries name paths or crate names that the renames invalidate?

## Answer

Checked: no path in `CLAUDE.md` or `docs/agents/*` moves under [The target folder tree](05-target-folder-tree.md); they name only `.scratch/<feature>/`, `docs/agents/*`, `CONTEXT.md` and `docs/adr/`. The only gaps are the new conventions.

- **`docs/agents/issue-tracker.md`** gets one new "Repo conventions" section with four rules: the finished-effort rule and `.scratch/archive/` ([03](03-how-to-archive.md)); the root `.ignore` that hides archives from search; the research/prototype convention (findings land on `main` at ticket close, prototypes get a pushed `archive/prototype-` tag, no ticket pointer names a branch, see [08](08-branch-and-worktree-policy.md)); text only committed, captures gitignored under `.scratch/**/captures/` ([04](04-scratch-public-or-not.md)). The "Wayfinding operations" section is untouched. `CONTRIBUTING.md` carries a short summary that links to it.
- **`CLAUDE.md`** gains a short "Layout" section: one line each on where code, docs, the shipped README and archives live (links to `CONTRIBUTING.md`), plus a warning that `.ignore` hides archives, so agents look in `docs/archive/` or `.scratch/archive/` explicitly. No crate names listed; `Cargo.toml` is the source of truth.
- **`AGENTS.md` bridge:** not added. Claude-only repo; cheap to add if a second agent tool appears.
- **`docs/agents/domain.md` and `triage-labels.md`:** unchanged.
- **Project memory:** the memory dir for this checkout holds only transcripts, no memory entries, so nothing names a path or crate the renames invalidate. Only the key directory moves ([07](07-local-checkout-folder.md)).
