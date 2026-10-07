# 10: Release dry run (HITL)

**What to build:** Proof that the real release pipeline still produces the same archives, on the layout branch, without releasing anything.

**Blocked by:** 09

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Maintainer confirms pushing a throwaway pre-release tag (for example `v0.0.0-layout-test`) from the layout branch
- [ ] Draft Release appears; maintainer compares its Windows, Linux and macOS file lists to the last real release; any difference fails and the layout is not merged unless it is a trivial workflow fix
- [ ] Throwaway tag and draft Release are deleted
