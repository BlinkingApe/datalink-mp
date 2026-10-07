# 02: Take the pre-layout baseline

**What to build:** A rollback point and a known-good reference: an annotated `pre-layout` tag on the Patch A release commit, with the build, test and clippy results recorded.

**Blocked by:** 01

**Status:** done

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [x] Annotated `pre-layout` tag is on the Patch A release commit (after 01's merges) and pushed (maintainer confirms)
- [x] Baseline run recorded in the ticket: i686 DLL build and Helper build with `--locked`, `cargo test --workspace`, `cargo clippy`
- [x] A copy of the baseline `Cargo.lock` is available for the later diff

## Comments

2026-10-07. Annotated tag `pre-layout` is on `a712693` (Patch A `v0.1.1` = `edb57f4`, plus the planning commit and 01's three research merges) and is pushed to `origin`, with the maintainer's approval.

Baseline on that tree:

| Command | Result |
|---|---|
| `cargo build --release --locked --target i686-pc-windows-gnu -p dplayx` | ok (1 `linker_messages` warning) |
| `cargo build --release --locked --target x86_64-pc-windows-gnu -p datalink-mp` | ok |
| `cargo build --release --locked --target x86_64-unknown-linux-musl -p datalink-mp` | fails locally: `x86_64-linux-musl-gcc` isn't installed on this machine (CI installs musl-tools). Native `cargo build --release --locked -p datalink-mp` is used instead and passes |
| `cargo test --workspace` | 243 passed, 0 failed, 2 ignored, 18 suites |
| `cargo clippy --workspace --all-targets` | passes with warnings: `iroh-transport` 4, `mock-dp-client` 2, `smac-fixes` 4+2, `dplayx` 4+4 (lib/test). Not errors; this count is the bar later runs are compared against |

The baseline `Cargo.lock` is copied outside the repo for the 06 diff; `git show pre-layout:Cargo.lock` gives the same file.
