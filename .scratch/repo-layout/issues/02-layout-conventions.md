# 02: Conventions for a repo layout a newcomer can follow

Type: research
Status: resolved
Blocked by:

Research: branch `research/repo-layout-conventions` (commit `8dc7743`), file `docs/research/repo-layout-conventions.md`.

## Question

What do well-regarded open-source projects do so that a newcomer can find their way, and which of it fits this repo: a Rust workspace with a 32-bit Windows DLL, a native Helper, a release pipeline, ADRs, and agent-driven planning in `.scratch/`? Specifically:

- **Rust workspace layout:** `crates/` and `tools/` (or `xtask`), where build-support code and scripts go, and conventions for naming crates within one product (prefixing with the product name or not).
- **Docs split by audience:** how users, contributors and maintainers are separated, for example Diátaxis, `CONTRIBUTING.md`, or `docs/dev/`. What belongs at the root, and what goes one level down.
- **Planning and decision records:** where ADRs live (`docs/adr/` or `doc/decisions/`), and what projects do with superseded ADRs and finished planning notes: keep them in the tree, archive them in-repo, or leave them to git history.
- **Archiving:** in-repo `archive/` folders versus deleting and relying on history and tags. Known pitfalls, such as broken permalinks and search noise.
- **Agent-facing files:** current conventions for `CLAUDE.md`, `AGENTS.md` and similar, and where they sit relative to human docs.

Prefer primary sources: the Cargo book, rust-lang and well-known Rust repos (ripgrep, tokio, iroh itself), the Diátaxis site, adr.github.io, and the GitHub docs. It's fine to say a question has no settled convention.

## Answer

Full findings with sources: `docs/research/repo-layout-conventions.md` on branch `research/repo-layout-conventions`.

- **Workspace:** the Cargo book recommends a flat `crates/` folder with a `members = ["crates/*"]` glob, and this repo already matches it, apart from two stray HTML files in member folders: `crates/smac-helper/prototype/ui-flow-prototype.html` and `tools/0001-wtp-through-the-ages.html`. An `xtask` crate isn't worth it for two bash scripts.
- **Crate names:** there's no official rule on product prefixes. The one firm convention, folder name equals crate name, already holds here. `iroh-transport` reads like one of iroh's own crates, and it isn't published, so renaming it is cheap.
- **Docs:** Diátaxis sorts docs by what the reader needs, not by audience. GitHub treats a root `README` and `CONTRIBUTING` specially. Rust projects keep entry points at the root and put contributor depth one level down. The suggestion: a root `CONTRIBUTING.md` pointing into `docs/`, and maintainer-only material (`releasing.md`, RC checklists) in something like `docs/maintainers/`.
- **ADRs:** the folder name varies across sources, so `docs/adr/` is fine. Every source agrees: never delete a superseded ADR, mark it superseded and don't reuse its number. There's no outside convention for finished planning notes, so `.scratch/archive/` is defensible if `docs/agents/issue-tracker.md` writes it down.
- **Archiving:** moving and deleting both break GitHub links that point at a branch. Only links pinned to a commit or tag survive. Moves split `git log` history unless you use `--follow`. A root `.ignore` listing archive folders hides them from ripgrep-based search, Claude's Grep included, while keeping them in git.
- **Agent files:** `AGENTS.md` is the cross-tool standard. Claude Code reads it only when there's no `CLAUDE.md`, and the documented bridge is a `CLAUDE.md` that imports `@AGENTS.md`. Good agent files point at the human contributor docs instead of repeating them, as this repo's `CLAUDE.md` already does.
