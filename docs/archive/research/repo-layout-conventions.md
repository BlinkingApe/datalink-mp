# Research: Repository layout conventions for newcomers

Supports `.scratch/archive/repo-layout/` ticket 02 (layout conventions). It asks what well-regarded open-source projects do so a newcomer can find their way, and which of those practices fit this repo.

Sources: this repo at commit `7d26254`. The other repos were read through the GitHub API at their default branches on 2026-10-06: ripgrep, tokio, iroh, rust-analyzer and cargo. Docs are cited by URL. "Fits this repo?" notes are judgement, and are labelled as such.

## TL;DR

1. **Workspace:** the Cargo book now *recommends* a flat `crates/` directory with a `crates/*` glob. This repo already uses `crates/` and `tools/`, but a tracked stray file in each of them would stop a glob from working. Crate prefixes are a matter of taste because Cargo has no stable namespaces. The one rule that matters: don't borrow another project's prefix (`iroh-transport`).
2. **Docs by audience:** Diátaxis sorts docs by the reader's *need*, not by audience. For audience it only says to split where readers really differ, for example keeping contributors' how-to guides apart. GitHub gives the root (or `.github/`, or `docs/`) `README` and `CONTRIBUTING` special treatment. Rust projects put contributor docs one level down (`docs/contributing/`, `docs/book/src/contributing/`).
3. **ADRs:** there are three common directory names (`doc/adr`, `docs/adr`, `docs/decisions`). Every primary source says the same thing: **never delete a superseded ADR. Keep it and mark it superseded.** No convention exists for finished planning notes.
4. **Archiving:** moving files into `archive/` breaks branch-based links and splits `git log` history. Deleting them breaks the same links and hides the files from grep. Neither choice is standard. Permalinks pinned to a commit SHA survive both.
5. **Agent files:** `AGENTS.md` is the cross-tool format. Claude Code reads it when there is no `CLAUDE.md`, and otherwise through `@AGENTS.md` imports. Copilot reads `AGENTS.md`, or a root `CLAUDE.md`. Rust projects keep these files at the root next to `CONTRIBUTING.md`, plus a separate `AI_POLICY.md` for *humans* using AI.

---

## 1. Rust workspace layout

**Flat `crates/` plus a virtual manifest.** The Cargo book says: *"Keep all member packages in a flat directory (commonly `crates/`) and use a glob pattern for the `members` field, e.g. `members = ["crates/*"]`. This minimizes churn in maintaining the `members` list."* ([Cargo book, Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), tip in `doc/book/src/reference/workspaces.md` in rust-lang/cargo.) The book also describes a virtual manifest as useful *"when there isn't a 'primary' package, or you want to keep all the packages organized in separate directories"* (same page). matklad, who led rust-analyzer, gives the reason: *"Make the root of the workspace a virtual manifest. It might be tempting to put the main crate into the root, but that pollutes the root with `src/`"* ([Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)).

What real projects do:

| Repo | Members live in | Root manifest |
|---|---|---|
| rust-analyzer | `crates/*`, `lib/*`, `xtask/` (globs) | virtual |
| cargo | `crates/*`, `credential/*`, root package | root package `cargo` |
| ripgrep | `crates/`, listed one by one (the `rg` binary's source is `crates/core`, built by the root package) | root package `ripgrep` |
| tokio | crate dirs at the root (`tokio/`, `tokio-util/`, ...) | virtual |
| iroh | crate dirs at the root (`iroh/`, `iroh-relay/`, ...) | virtual |

So `crates/` is the documented recommendation and the majority choice, but some major projects don't use it.

**Build-support code and scripts.** The `xtask` pattern puts automation in a workspace binary crate, usually `xtask/` at the root, run through a `.cargo/config.toml` alias `xtask = "run --package xtask --"`. It replaces shell scripts and Makefiles, so a contributor needs only `cargo` ([matklad/cargo-xtask](https://github.com/matklad/cargo-xtask)). rust-analyzer (`xtask/`, with `cargo codegen` and `cargo dist` aliases) and cargo (`crates/xtask-*`, all `publish = false`) both use it. ripgrep keeps `ci/` and `scripts/` shell directories instead. Cargo itself has no convention for build code shared between `build.rs` files. Its only layout rule is that a `build.rs` lives in the package root ([Cargo book, Package Layout](https://doc.rust-lang.org/cargo/guide/project-layout.html)).

**Crate naming.** There is no official rule about product prefixes. RFC 430 only says crates should be `snake_case` and *"prefer single word"* ([RFC 430](https://rust-lang.github.io/rfcs/0430-finalizing-naming-conventions.html)), while the Cargo book's own convention for binaries and targets is `kebab-case` (Package Layout page). Prefixes are common because *"the Cargo-level namespace of crates is flat"* (matklad, above): `tokio-util`, `iroh-relay`, `cargo-util`, and ripgrep's `crates/searcher`, which is published as `grep-searcher`. Real namespaces (`open-namespaces`, RFC 3243) are still unstable ([Cargo unstable features](https://doc.rust-lang.org/cargo/reference/unstable.html#open-namespaces)). rust-analyzer shows the opposite approach: its internal crates have short names (`hir`, `ide`, `syntax`) and are published under a separate `ra_ap_` prefix ([crates.io `ra_ap_hir`](https://crates.io/crates/ra_ap_hir)). matklad's one firm rule: *"Don't succumb to the temptation to strip common prefix from folder names. If each crate is named exactly as the folder it lives in, navigation and renames become easier."*

**Fits this repo?** (judgement)
- `crates/` + `tools/` with a virtual root manifest already matches the recommendation, and every directory name equals its package name.
- Two tracked non-crate files sit in member directories: `crates/smac-helper/prototype/ui-flow-prototype.html` (the `smac-helper` crate no longer exists) and `tools/0001-wtp-through-the-ages.html` (an archived issue says it was "kept out of git", but it is tracked now). They confuse newcomers, and they would get in the way of switching `members` to `crates/*`. Moving them to `docs/` or `.scratch/archive/` is cheap.
- `build-support/versioninfo.rs` pulled in with `include!` from two `build.rs` files works fine. The more idiomatic fix, if it grows, is a small `publish = false` build-dependency crate.
- Two bash scripts don't justify an `xtask`. Reconsider if a script has to run on Windows, or once there are several.
- Naming: none of the crates is published (`iroh-transport` doesn't exist on crates.io), so prefixes are optional. But `iroh-transport` reads as an n0 crate. If anything gets renamed, that's the one to rename, for example to `datalink-transport`. The rest (`dp-types`, `ipc-protocol`, `smac-fixes`, `dplayx`) is mixed but defensible. Note that `smac-fixes` has the lib name `smac_fixes`, which is just Cargo's normal hyphen-to-underscore mapping.

## 2. Docs split by audience

**Diátaxis** sorts content by the reader's *need*: tutorials (learning), how-to guides (tasks), reference (information) and explanation (understanding) ([diataxis.fr](https://diataxis.fr/)). It "doesn't impose implementation constraints" and recommends adopting it in small steps ([How to use Diátaxis](https://diataxis.fr/how-to-use-diataxis/)). On audiences, the site's "Diátaxis in complex hierarchies" page took up exactly the users / developers / contributors case. It suggests deciding how each audience sees the product, then perhaps *"allowing the developer-facing content to follow on from the user-facing material, while completely separating the contributors' how-to guides from both"* ([archived copy](https://web.archive.org/web/2024/https://diataxis.fr/complex-hierarchies/); the page is no longer in the current site source). So Diátaxis gives no audience folder scheme. Audience is a top-level split you add only where readers really differ.

**What GitHub privileges.** GitHub finds `README` and `CONTRIBUTING` in `.github/`, then the root, then `docs/`, in that order. It links `CONTRIBUTING` whenever someone opens an issue or PR ([About READMEs](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes), [Contributor guidelines](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/setting-guidelines-for-repository-contributors)).

**What Rust projects do:**
- **ripgrep:** everything is at the root, split by audience through file names: `README.md`, `GUIDE.md` and `FAQ.md` (users), `CONTRIBUTING.md` (contributors), `RELEASE-CHECKLIST.md` (maintainer).
- **tokio:** `CONTRIBUTING.md` at the root, with the detail one level down in `docs/contributing/` (PRs, reviewing, issue tracking).
- **rust-analyzer:** a short root `CONTRIBUTING.md`. Contributor docs live in an mdBook at `docs/book/src/contributing/` (`architecture.md`, `style.md`, `testing.md`, ...).
- **ARCHITECTURE.md:** matklad recommends a short `ARCHITECTURE.md` *"next to `README` and `CONTRIBUTING`"* for 10k–200k-line projects. It should name modules without linking to them, because *"links go stale"*, and be revisited *"a couple of times a year"* rather than kept in sync ([ARCHITECTURE.md](https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html)).

The pattern that emerges: the root holds entry points (README, CONTRIBUTING, licences, CHANGELOG, security and AI policies), and the depth lives one level down.

**Fits this repo?** (judgement) The root already holds the user README, `CONTEXT.md` and licences. `docs/` mixes audiences: `wine-compatibility.md` is for users, `building.md` for contributors, and `releasing.md` plus the rc test checklists are for maintainers. `ARCHITECTURE.md` sits in `docs/` rather than at the root. Following tokio's pattern costs little: add a root `CONTRIBUTING.md` that points to `building.md`, `ARCHITECTURE.md`, `CONTEXT.md` and `docs/adr/`, then move maintainer-only material into something like `docs/maintainers/` (or `docs/dev/`). A full Diátaxis restructure is more than a project this size needs. Use its four types as a lens when writing new pages.

## 3. Planning and decision records

**Where ADRs live.** Nygard's original post uses `doc/arch/adr-NNN.md` ([Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)). adr-tools defaults to `doc/adr` ([npryce/adr-tools](https://github.com/npryce/adr-tools)). MADR says *"Create folder `docs/decisions`"*, with `NNNN-title-with-dashes.md` file names ([MADR](https://adr.github.io/madr/)). The [adr.github.io](https://adr.github.io/) hub names no directory. None of these is settled, and `docs/adr/` with `NNNN-` file names (what this repo uses) is a common middle ground.

**Superseded ADRs: everyone keeps them.** Nygard: *"If a decision is reversed, we will keep the old one around, but mark it as superseded,"* and *"Numbers will not be reused."* MADR's status field includes `deprecated` and `superseded by ADR-0123`. Python's PEP 1 is the longest-running example. Superseded PEPs carry a `Superseded-By` header, and a resolved PEP becomes *"a historical document rather than a living specification"* ([PEP 1](https://peps.python.org/pep-0001/)). No primary source suggests deleting or archiving ADRs.

**Finished planning notes: no convention.** In the projects surveyed, planning lives in GitHub issues and PRs, not in the tree. In-repo planning folders like `.scratch/` are this repo's own convention (`docs/agents/issue-tracker.md`), so there's no outside practice to copy. iroh's root `CHANGELOG_old.md` is one small example of keeping a retired document in the tree under a clear name.

**Fits this repo?** (judgement) Keep `docs/adr/` as it is and never delete or move an ADR. When one is replaced, add a `Superseded by ADR-NNNN` status line and a back-link. For finished `.scratch/` efforts, `.scratch/archive/<feature>/` (already in use) is a reasonable local rule. Write it down in `docs/agents/issue-tracker.md` so agents know archived tickets aren't live, because nothing outside the repo will tell them.

## 4. Archiving: in-repo `archive/` vs delete and rely on history

There is no settled convention. Each option has real costs:

- **Broken links (both options).** GitHub warns that a branch URL shows *"the head of branch"*, which can change. For a stable link you *"put a commit ID"* in the URL (press `y`) ([Getting permanent links to files](https://docs.github.com/en/repositories/working-with-files/using-files/getting-permanent-links-to-files)). Moving a file to `archive/` and deleting it both break every `blob/main/...` link to it. SHA permalinks survive both. This repo's tickets already cite commits (for example `6399164`), which is the robust habit.
- **History split by moves.** After a move, `git log <path>` stops at the rename. `git log --follow` continues *"beyond renames (works only for a single file)"* (`git help log`).
- **Search noise (archive option).** Archived text shows up in `grep`, in agents' searches, and in GitHub search. Claude Code's Grep tool *"is built on ripgrep"* and *"respects `.gitignore`"* ([Claude Code tools reference](https://code.claude.com/docs/en/tools-reference)). ripgrep also honours a `.ignore` file, which takes precedence over gitignore ([ripgrep GUIDE](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md)). So a root `.ignore` listing `.scratch/archive/` would hide archived files from ripgrep-based searches while keeping them tracked. Glob listings don't respect it.
- **Discoverability (delete option).** Deleted files are invisible unless you already know the path or the commit. Tags help: a file deleted after `v0.1.0` stays reachable at `.../blob/v0.1.0/<path>`.

**Fits this repo?** (judgement) Keep ADRs in place (section 3). For `.scratch/`, archiving in-repo is defensible, because agents read across efforts and the "Answer" sections hold reasoning that git log doesn't show. Add the `.ignore` entry to cut search noise. For one-off artefacts (the rc test checklists, prototypes), deleting them and pointing to a tag or SHA is cleaner than archiving.

## 5. Agent-facing files

**`AGENTS.md`** describes itself as *"a README for agents"* that keeps *"READMEs concise and focused on human contributors."* It goes at the repo root, with optional nested copies where *"the closest one takes precedence."* It is supported by Codex, Jules, Aider, VS Code, Copilot, Cursor and others. For migration it suggests renaming existing agent files to `AGENTS.md` and symlinking the old names ([agents.md](https://agents.md/)).

**Claude Code** reads `./CLAUDE.md` or `./.claude/CLAUDE.md`, and loads subdirectory `CLAUDE.md` files on demand. By default it reads `AGENTS.md` *only when there is no `CLAUDE.md`*. To share one file across tools, the documented pattern is a `CLAUDE.md` that starts with `@AGENTS.md` followed by any Claude-specific lines. The docs recommend keeping each file *"under 200 lines"* and moving narrower rules into `.claude/rules/` ([Claude Code memory](https://code.claude.com/docs/en/memory)).

**GitHub Copilot** reads `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md`, and *"one or more AGENTS.md files, stored anywhere within the repository"* (nearest wins), or *"a single CLAUDE.md or GEMINI.md file stored in the root"* ([Copilot repository instructions](https://docs.github.com/en/copilot/how-tos/configure-custom-instructions/add-repository-instructions)).

**What Rust projects do.** rust-analyzer has root `AGENTS.md` and `CLAUDE.md` files that are near-identical copies. Each one points the agent to the *human* contributor docs (`docs/book/src/contributing/*.md`) rather than repeating them. ripgrep, iroh and rust-analyzer all ship a root **`AI_POLICY.md`**. That file is a separate thing: rules for *people* who use AI, such as no AI-written issue comments and disclosing AI use. All of these sit at the root, next to `CONTRIBUTING.md`.

**Fits this repo?** (judgement) Today's `CLAUDE.md` is small and mostly points into `docs/agents/`, which is the pattern rust-analyzer uses. If other agents ever matter, move the content to `AGENTS.md` and make `CLAUDE.md` an `@AGENTS.md` import rather than keeping two copies. Keep `docs/agents/` for the longer workflow notes. An `AI_POLICY.md` only matters once outside contributors arrive.
