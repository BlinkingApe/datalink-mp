# 10: How to prove the layout build broke nothing

Type: grilling
Status: resolved
Blocked by: 05, 06, 09

## Question

What checks does the layout spec require, who runs each, and what result means roll back to the `pre-layout` tag? Candidates, from the [ledger](../ledger.md)'s hard-coded paths and the moves in [The target folder tree](05-target-folder-tree.md), [Crate renames](06-crate-renames.md) and [When to do the moves](09-when-to-do-the-moves.md):

- Both build targets (the i686 Windows DLL, the Helper) and the tests, on the renamed crates. Includes the one silent break 06 names: the `iroh_transport=debug` log directive in the Helper's `main.rs`.
- The `build.rs` `include!` paths, `http.rs` `include_str!`/`include_bytes!` paths, and `scripts/launch-whisky.sh`'s error message.
- A link check over README, docs, ADRs, `.scratch` tickets (including the rewritten paths in open internet-play-speed-build tickets, ADR-0005 and `docs/traffic-capture.md`).
- The release workflow, which triggers only on a pushed `v*` tag and opens a draft Release. A real dry run means pushing a throwaway pre-release tag, then deleting the tag and draft: outward-facing, so the maintainer must confirm. Is it worth it, or is a read-through of `release.yml` against the new paths enough?
- Release archive contents: do the archives still hold `README.txt` under its shipped name and the three `LICENSE-*` files?

## Answer

Decided with the maintainer, 2026-10-06. They accepted every recommendation.

**Checks**

1. **Build and test gate.** Run the exact `release.yml` commands locally: `cargo build --release --locked --target i686-pc-windows-gnu -p dplayx` and `-p datalink-mp` for the Helper, plus `cargo test --workspace` and `cargo clippy`. Take a baseline on the `pre-layout` tag first. `--locked` plus a diff of `Cargo.lock` proves only crate names changed, with no dependency version drift.
2. **Silent-break checks.** A script greps tracked files for every old crate name and path (`iroh-transport`, `iroh_transport`, `ipc-protocol`, `ipc_protocol`, `smac-fixes`, `smac_fixes`, `mock-dp-client`, `datalink-mp-README.txt`, `tools/`). Every remaining hit must be on an allowlist (ADRs and archived history keep old names). The `iroh_transport=debug` directive in the Helper's `main.rs` is also checked at runtime: run the Helper at debug level and confirm the renamed crate still logs. `scripts/launch-whisky.sh`'s error message is covered by the grep and the `build.rs`/`http.rs` include paths by the build.
3. **Link check.** A reusable script in `scripts/` resolves every relative markdown link and backticked path in README, CONTRIBUTING, `docs/`, ADRs, `CLAUDE.md` and `.scratch/**` (including the rewritten open internet-play-speed-build tickets, ADR-0005 and `docs/traffic-capture.md`). Links that point at a branch are listed and reviewed by hand.
4. **Release workflow.** `release.yml` is **edited** (correcting [06](06-crate-renames.md)): it copies the root `datalink-mp-README.txt`, which [05](05-target-folder-tree.md) moves to `packaging/README.txt`; the shipped name stays. Proof is a read-through against the new paths, a local script that replays the packaging steps, then a real dry run: push a throwaway pre-release tag (for example `v0.0.0-layout-test`) from the layout branch, check the draft Release, delete the tag and draft. The maintainer confirms the push.
5. **Release archive contents.** Download the draft's Windows, Linux and macOS archives and compare their file lists to the last real release. Expected: identical names (`datalink-mp-README.txt`, three `datalink-mp-LICENSE-*.txt`, the Helper binary, `dplayx.dll`). Any difference fails.

**Who runs what.** The agent runs 1, 2, 3 and the local packaging replay after each build step that moves files, and again at the end. The maintainer confirms and watches the dry run, checks the archive lists (5), and reviews non-allowlisted grep hits and branch-pointing links.

**Rollback.**
- Before the dry run, fix forward on the layout branch; nothing is public.
- Reset the layout branch to `pre-layout` and re-plan if a build or test failure is still unexplained after one fix attempt, or a dependency version in `Cargo.lock` changed.
- If the draft's archive contents differ and it isn't a trivial workflow fix, don't merge: delete the throwaway tag and draft and return to the layout branch.
- After merge to `main`, restore with a revert commit, not rewritten history.

**Order.** The layout build runs on one branch; the checkout-folder rename ([07](07-local-checkout-folder.md)) stays last and outside git. The branch merges to `main` only after all checks pass and the dry run succeeds. Deleting the throwaway tag and draft is a checklist item.
