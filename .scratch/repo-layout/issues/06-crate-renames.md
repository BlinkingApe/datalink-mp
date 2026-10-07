# 06: Crate renames

Type: grilling
Status: resolved
Blocked by: 01

## Question

[ADR-0003](../../../docs/adr/0003-standalone-project.md) left internal crate names open. Today they are `dp-types`, `ipc-protocol`, `iroh-transport`, `datalink-mp`, `dplayx`, `smac-fixes` and `tools/mock-dp-client`. Which ones get renamed, and to what? Should they share a prefix (such as `datalink-`) so a newcomer sees one product?

Hard constraints:

- The DLL must still build as `dplayx.dll` (lib name and `dplayx.def`).
- The Helper binary stays `datalink-mp`.

List what each rename touches (`Cargo.toml` paths, `use` statements, `build.rs`, the release workflow, docs), so the move list is complete. Decide whether crate folder names follow crate names.

## Answer

Decided with the maintainer, 2026-10-06. They accepted the recommendation on all three questions.

### Names

| Today | New | Why |
|---|---|---|
| `datalink-mp` | `datalink-mp` | Helper crate and binary, unchanged |
| `dplayx` | `dplayx` | DLL name is fixed (`[lib] name`, `dplayx.def`, release.yml `-p dplayx`) |
| `dp-types` | `dp-types` | Generic DirectPlay types, no product name; stays reusable on its own |
| `ipc-protocol` | `datalink-ipc` | DLL-to-Helper protocol is specific to this product |
| `iroh-transport` | `datalink-transport` | The one name research found misleading (reads like an iroh crate) |
| `smac-fixes` | `datalink-fixes` | Drops the SMAC mark ADR-0003 avoided; the crate is internal |
| `mock-dp-client` | `datalink-mock-client` | Joins `crates/`, shares the prefix |

Rule: product-specific crates carry `datalink-`; `dplayx` (fixed by the DLL) and `dp-types` (generic) don't. Crate folder = crate name, so every folder is renamed with it (`crates/datalink-ipc/` etc.). No crate is published, so there is no crates.io fallout. This closes ADR-0003's "renaming is the spec's call".

### What each rename touches

Rust identifiers change with the hyphen-to-underscore form: `ipc_protocol`->`datalink_ipc`, `iroh_transport`->`datalink_transport`, `smac_fixes`->`datalink_fixes`. `dp_types` is untouched.

- **Root `Cargo.toml`:** `members` (replace the list with `crates/*`, which also picks up the moved mock client), the three `[workspace.dependencies]` path entries (`ipc-protocol`, `iroh-transport`; `dp-types` unchanged). `Cargo.lock` regenerates: 4 package entries and their dependents change; build with `--locked` only after committing the new lock.
- **Crate manifests:** each renamed crate's `name =`; `datalink-mp/Cargo.toml` (ipc-protocol, iroh-transport path deps); `dplayx/Cargo.toml` (ipc-protocol workspace dep, smac-fixes path dep, update the `../` path); `ipc-protocol/Cargo.toml` is only renamed; `smac-fixes/Cargo.toml` `[lib] name = "smac_fixes"` becomes `datalink_fixes`; `mock-dp-client/Cargo.toml` package and `[[bin]] name`.
- **Rust sources (`use`/paths):** `ipc_protocol` in 9 files (dplayx `ipc_client.rs`, `directplay.rs`; datalink-mp `main.rs`, `controller.rs`, `ipc_server.rs`, tests `common/mod.rs`, `ipc.rs`, `stop_failure.rs`, `ui.rs`); `iroh_transport` in 9 files (the `mesh_networking.rs` test, datalink-mp `main.rs`, `lib.rs`, `controller.rs`, tests `ipc.rs`, `ui.rs`, `stop_failure.rs`, `cli.rs`, and mock-dp-client `main.rs`); `smac_fixes` in `dplayx/src/lib.rs` (2 calls).
- **Strings that are not compile errors if missed:** `datalink-mp/src/main.rs:93` `"iroh_transport=debug"` log directive (must become `datalink_transport=debug`, or Helper debug logs silently lose the transport); the `smac-fixes:` prefix in the `probe.rs:351` eprintln; any log filter or `RUST_LOG` example in docs. `build.rs` files and `dplayx.def` mention no renamed crate.
- **Release workflow and scripts:** `release.yml` builds with `-p dplayx` and `-p datalink-mp` only, so crate renames leave it alone (it is still edited for the README path moved by [05](05-target-folder-tree.md); see [10](10-prove-nothing-broke.md)); `.cargo/config.toml` likewise. No script names an internal crate.
- **Live docs:** `docs/building.md` (crate table, 4 rows plus the mock client), `docs/ARCHITECTURE.md` (7 mentions, headings and paths), `docs/traffic-capture.md`, `ADR-0001`, `ADR-0004`; these ADRs are accepted records, so edit only the crate paths and note nothing else. `docs/research/av-risk.md` and `transport-restart.md` are archived, so leave them (names in history stay as written).
- **`.scratch`:** planning prose in internet-play-speed (map and `analysis/turn-sync-fixes.md`), game-session-sync, post-0.1.0-polish and release-pipeline. Update only efforts still live; archived efforts keep the old names, and the build tickets should say so. `internet-play-speed-build` tickets don't cite these crate names today.

### Ordering and checks it hands on

- Do the folder moves with `git mv` in the same commit as the manifest edits, then regenerate the lock.
- The "prove nothing broke" ticket must include: `cargo check --workspace`, the i686 DLL build and `-p datalink-mp` build, `grep` for each old name (hyphen and underscore) returning only archived material, and one run of the Helper with default `RUST_LOG` to see transport debug lines still appear.
- `docs/building.md` is itself moving to `docs/contributors/`, so edit it once, at its new path.
