# 05: Move docs, packaging and archives

**What to build:** The repo has the target tree from the spec, apart from the crate renames: a player-first root, `docs/` split by reader, archives separated and hidden from search, and the release workflow still packaging the same files.

**Blocked by:** 03, 04

**Status:** ready-for-agent

Spec: [../../repo-layout/spec.md](../../repo-layout/spec.md), and the decision tickets it cites.

## Acceptance criteria

- [ ] Moves follow the table in the target-folder-tree ticket row by row, using `git mv`, with inbound references rewritten except the open-ticket and live-prose rewrites that ticket 07 handles
- [ ] `datalink-mp-README.txt` is now `packaging/README.txt`; `release.yml` copies it into the archive under the old shipped name and its comment points at `docs/maintainers/releasing.md`
- [ ] Files in the spec's delete list are removed (after confirming `traffic-capture.md` carries the launch command); `crates/smac-helper/` is gone
- [ ] Finished efforts (release-pipeline, ui-polish-0.1.1) are in `.scratch/archive/`; `docs/archive/` holds all seven research write-ups, the 0.1.0 checklist and the WTP page, with a `README.md` index of old path, new path or deleted, and recovery tag
- [ ] Root `.ignore` lists both archives
- [ ] The grep and link-check scripts run and every new hit is understood or on the allowlist; builds still pass
