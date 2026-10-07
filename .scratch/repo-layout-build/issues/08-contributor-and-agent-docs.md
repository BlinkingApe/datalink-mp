# 08: Contributor and agent docs

**What to build:** A newcomer, a contributor and an agent each find their entry point, and the conventions are written down where they will be read.

**Blocked by:** 05

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Root `CONTRIBUTING.md` covers architecture, building, and how `.scratch` works (map, tickets, specs, `Status:` lines, archive, text-only, raw captures stay local), linking to the `docs/` guides
- [ ] `.scratch/README.md` is a few lines pointing at that section; `.gitignore` has `.scratch/**/captures/`
- [ ] `CLAUDE.md` gains a short Layout section of links and the warning that `.ignore` hides archives; no crate names
- [ ] `docs/agents/issue-tracker.md` gains a Repo conventions section: finished-effort rule, `.ignore`, research/prototype convention, text-only commits; Wayfinding operations is untouched; `domain.md` and `triage-labels.md` are unchanged; no `AGENTS.md`
