# 02: Take the pre-layout baseline

**What to build:** A rollback point and a known-good reference: an annotated `pre-layout` tag on the Patch A release commit, with the build, test and clippy results recorded.

**Blocked by:** 01

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Annotated `pre-layout` tag is on the Patch A release commit (after 01's merges) and pushed (maintainer confirms)
- [ ] Baseline run recorded in the ticket: i686 DLL build and Helper build with `--locked`, `cargo test --workspace`, `cargo clippy`
- [ ] A copy of the baseline `Cargo.lock` is available for the later diff
