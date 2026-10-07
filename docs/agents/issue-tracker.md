# Issue tracker: Local Markdown

Issues and specs for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`, never a single combined tickets file
- Triage state is recorded as a `Status:` line near the top of each issue file (see `triage-labels.md` for the role strings)
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/` (creating the directory if needed).

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or the issue number directly.

## Repo conventions

- **Finished efforts.** An effort is finished when every ticket is resolved or closed and no open ticket elsewhere cites it as the spec to build from. Closing it is the effort's last step: `git mv` the folder to `.scratch/archive/<effort>/`, fix every inbound link, then run `scripts/check-links.py`. An evidence doc that stops being current moves to `docs/archive/`, with a row in `docs/archive/README.md`.
- **`.ignore`.** The root `.ignore` hides `.scratch/archive/` and `docs/archive/` from Grep, Glob and `rg`, so searches skip them. To read history, name the path or search with `rg --no-ignore`.
- **Research and prototypes.** When a research ticket closes, its write-up lands on `main` as a doc and the branch is deleted. When a prototype ticket closes, its tip is tagged `archive/prototype-<name>`, the tag is pushed (the maintainer confirms the push), and the branch is deleted. Ticket pointers read "Research: `docs/<path>`" or "Prototype: tag `archive/prototype-<name>`", never a branch name.
- **Text only.** Commit maps, tickets, specs, analyses and small analysis scripts. Raw captures (pcaps, packet dumps, game logs), screenshots and binaries stay local in `.scratch/<effort>/captures/`, which `.gitignore` ignores, because captures can hold peers' IP addresses and iroh node IDs.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` (the Notes / Decisions-so-far / Fog body).
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
