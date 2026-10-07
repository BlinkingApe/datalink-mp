# 04: Does `.scratch/` planning history stay in the public repo?

Type: grilling
Status: resolved
Blocked by: 01

## Question

`.scratch/` is tracked: about 93 files of maps, tickets, specs and analyses sit in the public GitHub repo. It's the issue tracker by design (`docs/agents/issue-tracker.md`), and ADRs link into it. Does it stay public and tracked as it is? Or does it move (renamed, nested under `docs/`, gitignored, or kept in a separate repo)? If it stays, how does a newcomer learn what it is? Also: should captures and other maintainer-only material ever be committed there?

## Answer

Decided with the maintainer, 2026-10-06. They accepted every recommendation.

- **Stays tracked and public.** `.scratch/` is the issue tracker by design, ADRs cite it as evidence, and it is small (about 93 files, 3.2 MB). Gitignoring it or moving it to another repo would break the ADR links and cut agents off.
- **Keeps its name and path** at the repo root. No rename, no nesting under `docs/`. This also keeps decision 03's `.scratch/archive/` valid.
- **A newcomer learns what it is** from a section in the new root `CONTRIBUTING.md` (map, tickets, specs, `Status:` lines, archive), plus a short `.scratch/README.md` of a few lines pointing at that section.
- **Text only is committed:** maps, tickets, specs, analyses and small analysis scripts. Raw captures (pcaps, packet dumps, game logs), screenshots and binaries stay out, because captures can hold peers' IP addresses and iroh node IDs. A `.scratch/**/captures/` pattern goes in `.gitignore`, and CONTRIBUTING says to keep raw captures locally.
- **Maintainer-only files:** effort-specific material goes in that effort's `.scratch/<effort>/` folder. Only guides that outlive an effort (such as `releasing.md`) go in `docs/maintainers/`. No further ignore rules.
