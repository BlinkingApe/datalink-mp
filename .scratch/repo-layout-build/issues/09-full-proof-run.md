# 09: Full proof run

**What to build:** The agent-run checks pass on the final tree, with a report for the maintainer.

**Blocked by:** 06, 07, 08

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Build gate with the exact `release.yml` commands, test and clippy all pass
- [ ] Grep and link-check scripts pass; non-allowlisted hits and branch-pointing links are listed for the maintainer
- [ ] A local replay of the release packaging steps yields the same archive file names as before (`datalink-mp-README.txt`, three `datalink-mp-LICENSE-*.txt`, the Helper binary, `dplayx.dll`)
- [ ] Rollback rule applied if needed: reset the layout branch to `pre-layout` and re-plan if a failure is unexplained after one fix attempt or a dependency version changed
