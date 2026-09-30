# 01: Rename the Helper to `datalink-mp`

**What to build:** The Helper's package and binary are called `datalink-mp` (ADR-0003). A player or script that ran `smac-helper host` now runs `datalink-mp host` and gets the same behaviour. Nothing else about the Helper changes in this ticket: it is a prefactor so that every later ticket uses the final name.

Scope, from the spec's "Naming" section:

- The Helper's package and binary are renamed. The other crates keep their names.
- The workspace `repository` field points at `BlinkingApe/datalink-mp`.
- User-visible strings that say "smac-helper" change to "datalink-mp": the Helper's `--help` text, its log lines, and the DLL's "is the Helper running?" log lines. These are log text only, so the IPC version does not change.
- The environment variables keep their names (`SMAC_HELPER_PORT`, `SMAC_HELPER_LOG_FILE`). The Ticket keeps its `smac` prefix.
- The default log filter names the Helper's crate; it must follow the rename so file logging still captures the Helper's own lines.

Documentation is not part of this ticket (see ticket 16).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `cargo build` produces a binary named `datalink-mp`, and no binary named `smac-helper`
- [x] `datalink-mp host` prints a Ticket as the first line of standard output and serves the DLL as before
- [x] `datalink-mp --help` does not mention "smac-helper"
- [x] No "smac-helper" string remains in the Helper's or the DLL's log and error text
- [x] With `SMAC_HELPER_LOG_FILE` set, the log file still receives the Helper's debug lines
- [x] `SMAC_HELPER_PORT` is still honoured by both the Helper and the DLL
- [x] The IPC version and the Ticket format are unchanged
- [x] The workspace `repository` field points at `BlinkingApe/datalink-mp`
- [x] `cargo test` passes for the whole workspace

## Comments

Implemented on `main` in `0258a65` (tidied in `7997345`). The crate directory moved to `crates/datalink-mp` as well.

Verified by running the built binary: the Ticket on standard output, an IPC handshake, `SMAC_HELPER_PORT`, `--help`, and debug lines with target `datalink_mp` in the log file. The DLL side was verified by `cargo check -p dplayx --target i686-pc-windows-gnu` and by reading the code; it was not run under Wine.

`README.md` and `docs/` still name `smac-helper` until ticket 16.
