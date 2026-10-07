# 07: Rewrite paths in open tickets and live docs

**What to build:** Every ticket an agent will pick up next, and the docs it cites, name paths and crates that exist.

**Blocked by:** 05, 06

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Open internet-play-speed-build tickets, ADR-0005, `docs/contributors/traffic-capture.md` and live-effort prose (post-0.1.0-polish, game-session-sync, internet-play-speed) use new paths and crate names; archived efforts keep old names
- [ ] Build tickets' pointers to research or prototype read as doc paths or the `archive/prototype-` tag, never a branch
- [ ] The link-check script and the grep script report nothing outside the allowlist for these files
