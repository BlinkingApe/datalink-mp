# 06: Rename the crates

**What to build:** The workspace builds and tests green with the four renamed crates and the mock client under `crates/`; the Helper's transport debug logs still appear.

**Blocked by:** 05

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] `ipc-protocol`, `iroh-transport`, `smac-fixes`, `mock-dp-client` become `datalink-ipc`, `datalink-transport`, `datalink-fixes`, `datalink-mock-client`; folders follow names via `git mv`; `dplayx`, `dp-types` and `datalink-mp` are unchanged
- [ ] Root `members` is `crates/*`; workspace path dependencies, every crate manifest, the `use` paths and `smac_fixes` calls are updated per the crate-renames ticket; `Cargo.lock` is regenerated and committed, with a diff against the baseline showing only crate-name changes
- [ ] The `iroh_transport=debug` directive and the `smac-fixes:` eprintln prefix are renamed; a Helper run at debug level shows the transport crate logging
- [ ] Live docs (`contributors/building.md`, `architecture.md`, `traffic-capture.md`, ADR-0001 and 0004 crate paths only) are edited once at their new paths; archived material keeps old names
- [ ] i686 DLL and Helper `--locked` builds, `cargo test --workspace` and `cargo clippy` pass. One ticket by design: a half-renamed workspace cannot build green, so expand-contract does not apply
