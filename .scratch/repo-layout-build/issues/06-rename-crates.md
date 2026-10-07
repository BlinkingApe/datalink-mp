# 06: Rename the crates

**What to build:** The workspace builds and tests green with the four renamed crates and the mock client under `crates/`; the Helper's transport debug logs still appear.

**Blocked by:** 05

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] `ipc-protocol`, `iroh-transport`, `smac-fixes`, `mock-dp-client` become `datalink-ipc`, `datalink-transport`, `datalink-fixes`, `datalink-mock-client`; folders follow names via `git mv`; `dplayx`, `dp-types` and `datalink-mp` are unchanged
- [x] Root `members` is `crates/*`; workspace path dependencies, every crate manifest, the `use` paths and `smac_fixes` calls are updated per the crate-renames ticket; `Cargo.lock` is regenerated and committed, with a diff against the baseline showing only crate-name changes
- [x] The `iroh_transport=debug` directive and the `smac-fixes:` eprintln prefix are renamed; a Helper run at debug level shows the transport crate logging
- [x] Live docs (`contributors/building.md`, `architecture.md`, `traffic-capture.md`, ADR-0001 and 0004 crate paths only) are edited once at their new paths; archived material keeps old names
- [x] i686 DLL and Helper `--locked` builds, `cargo test --workspace` and `cargo clippy` pass. One ticket by design: a half-renamed workspace cannot build green, so expand-contract does not apply

## Comments

2026-10-07 (agent). Built with `scripts/check-old-names.sh` as the red/green signal and `cargo test --workspace` plus the build gate as the regression net.

**Renames** (one commit, folders by `git mv`): `crates/ipc-protocol` → `crates/datalink-ipc`, `crates/iroh-transport` → `crates/datalink-transport`, `crates/smac-fixes` → `crates/datalink-fixes`, `tools/mock-dp-client` → `crates/datalink-mock-client` (`tools/` is now gone). Edits:

- root `members = ["crates/*"]` and the two `[workspace.dependencies]` path entries
- each renamed crate's `name =`; `datalink-fixes`' `[lib] name = "datalink_fixes"`; the mock client's `[[bin]] name`
- the `datalink-mp` and `dplayx` path and workspace deps
- the `use` and qualified paths in the 9 + 9 files the decision ticket lists, plus the two `datalink_fixes::` calls in `dplayx/src/lib.rs`

**Strings the compiler doesn't check:**

- The Helper's default filter directive is now `datalink_transport=debug` (`crates/datalink-mp/src/main.rs:93`).
- The `datalink-fixes:` prefix replaces `smac-fixes:` in the `probe.rs` eprintln. The grep also found the same prefix in six `info!` lines in `datalink-fixes/src/lib.rs`, which the decision ticket didn't name; they are renamed too, for consistency.
- No doc carries a `RUST_LOG` example naming a crate.
- `build.rs`, `dplayx.def`, `release.yml` and `.cargo/config.toml` name no renamed crate and are untouched. The `SMAC_*` env var names are not crate names and stay.

**Docs** (edited once, at their new paths): `docs/contributors/architecture.md` (7: the diagram, four headings and paths, the mesh-test path), `building.md` (crate table, 4 rows), `traffic-capture.md` (the `capture.rs` path), ADR-0001:163 (crate path only). ADR-0001:85's `iroh_transport::Ticket::parse` is a Rust path in the record's prose, not a crate path, so it stays (the `docs/adr/*` allowlist entry covers it). ADR-0004 names "smac-fixes" only in prose, with no crate path, so it is unchanged.

**Cargo.lock:** regenerated with `cargo metadata`. The text diff only moves the four package entries to their new alphabetical places and swaps the names in the `datalink-mp`, `dplayx` and mock-client dependency lists. I checked it mechanically against `notes/Cargo.lock.pre-layout`: once the four old names are mapped to the new ones, both lockfiles hold the same 434 packages with identical versions, sources, checksums and dependency lists (lock format 4 in both). No dependency version changed.

**Runtime check:** `SMAC_HELPER_LOG_FILE=<file> datalink-mp host --port 47913` with `RUST_LOG` unset, so the default `debug` filter with the `datalink_transport=debug` directive applies. It ran with a 15 s timeout, was stopped with SIGINT, and the process was confirmed gone. The log carries the renamed target:

```
2026-10-07T10:23:20.051122Z  INFO datalink_transport::runtime: Initializing Iroh endpoint...
2026-10-07T10:23:20.092135Z  INFO datalink_transport::runtime: Endpoint ID: 5a00e7cd...
```

These are INFO lines under the debug filter. The transport's `debug!` calls fire on DLL traffic (send, receive, enum sessions). A host and joiner pair with no DLL attached logged no transport DEBUG lines either, because the CLI joiner waits for the DLL before it connects. So the target is live and the filter accepts it. The ticket 10 proof run with a game can confirm the DEBUG lines.

**Proof gate:**

- `check-old-names.sh`: before **134** not allowlisted / 157 allowlisted; after **19 / 157**. All 19 are live-effort prose for ticket 07: `.scratch/game-session-sync` 6, `internet-play-speed` 6, `internet-play-speed-build` 2, `post-0.1.0-polish` 5. No hit remains in code, manifests, the lockfile or live docs.
- `check-links.py`: before 23 broken / 373 allowlisted / 22 branch refs; after **39 / 404 / 22**. The 16 new broken links are backticked `crates/iroh-transport/...` paths in those same live-effort tickets; ticket 07 rewrites them. The 31 new allowlisted ones are old crate paths in the archives and in this effort's own tickets. Every broken link is under `.scratch/game-session-sync`, `internet-play-speed`, `internet-play-speed-build` or `post-0.1.0-polish`.
- `scripts/test-proof-scripts.sh`: all pass.
- Build gate (all `--locked`, after regenerating the lock): the i686 `dplayx`, x86_64-windows `datalink-mp` and native `datalink-mp` release builds exit 0. `cargo test --workspace`: **243 passed, 0 failed, 2 ignored** (as baseline). `cargo clippy --workspace --all-targets` exits 0. Its warnings per crate equal `baseline-clippy.txt` under the new names: `datalink-transport` lib 4 / test 4 dup; `datalink-mock-client` bin 2 / test 2 dup; `datalink-fixes` lib test 4 / lib 2 dup; `dplayx` 4 / 4.

**For ticket 07:** the 19 old-name hits and the 16 `crates/iroh-transport/...` paths listed above. That includes the planned `crates/iroh-transport/src/early_ack.rs` (internet-play-speed 09) and `crates/iroh-transport/tests/fixtures/` (internet-play-speed-build 05), which become `crates/datalink-transport/...`.

**Noted, not changed:** `cargo fmt --check` reports diffs across most of the workspace, but they predate this ticket. The renamed code was not reformatted.
