# 04: Proof scripts

**What to build:** Two reusable scripts the later tickets run to show nothing broke: an old-name grep with an allowlist, and a link checker.

**Blocked by:** 02

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Grep script searches tracked files for every old crate name (hyphen and underscore), `datalink-mp-README.txt` and `tools/`, and fails on any hit not on the allowlist (ADRs, archived history)
- [ ] Link-check script resolves every relative markdown link and backticked path in README, CONTRIBUTING, `docs/`, ADRs, `CLAUDE.md` and `.scratch/**`, and lists links that point at a branch separately
- [ ] Both scripts live in `scripts/`, are documented in a header comment, and have been run against the baseline with the output recorded in the ticket
