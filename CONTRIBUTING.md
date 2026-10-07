# Contributing

The [README](README.md) is for players. This page is for people who change the
code or the docs. The words the project uses (Helper, DLL, Ticket, Game folder)
are defined in [GLOSSARY.md](GLOSSARY.md).

## Architecture

datalink-mp is a replacement `dplayx.dll` that the game loads, and a native
Helper that talks to other Helpers over Iroh. The DLL and the Helper talk over
localhost. [docs/contributors/architecture.md](docs/contributors/architecture.md)
explains the design and why it is split this way. Decisions and their reasons
are in the ADRs, [docs/adr/](docs/adr/).

The crates live in `crates/`, one folder per crate, named after the crate;
`Cargo.toml` lists them. The
[repository layout table](docs/contributors/building.md#repository-layout) says
what each one does.

## Building

[docs/contributors/building.md](docs/contributors/building.md) covers the
toolchain, the build, assembling a Game folder by hand, and the diagnostic
logs. [docs/contributors/traffic-capture.md](docs/contributors/traffic-capture.md)
covers recording a game's traffic for analysis.

Releases are cut by the maintainer:
[docs/maintainers/releasing.md](docs/maintainers/releasing.md).

## Where docs live

| Path | What |
|---|---|
| `README.md` | the player's guide, also the GitHub front page |
| `packaging/README.txt` | the README that ships inside every release archive |
| `docs/players/` | player guides the README links to |
| `docs/contributors/` | architecture, building, traffic capture |
| `docs/maintainers/` | releasing and release testing |
| `docs/adr/` | architecture decision records |
| `docs/agents/` | how coding agents use this repo |
| `docs/archive/` | finished evidence docs; [its README](docs/archive/README.md) maps old paths to new ones |
| `.scratch/` | planning: efforts, tickets and specs (below) |

## How `.scratch/` works

`.scratch/` is the project's issue tracker. It is tracked and public, and the
ADRs link into it as evidence.

- **Efforts.** Each piece of work has a folder, `.scratch/<effort>/`.
- **Map.** An effort starts as a map, `map.md`: what is decided so far and
  what is still unknown.
- **Tickets.** Each question or task is a ticket,
  `issues/NN-<slug>.md`, numbered from `01`. A decision ticket records its
  answer under `## Answer`; discussion goes under `## Comments` at the bottom.
- **Specs.** When the map's questions are answered, `spec.md` says what to
  build, and a build effort (often `<effort>-build/`) holds the build tickets.
- **`Status:` lines.** Every ticket has a `Status:` line near the top:
  `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`,
  `claimed`, `resolved` or `done`. `Blocked by:` lists the tickets that must be
  finished first.
- **Archive.** When every ticket of an effort is resolved or closed, and no
  open ticket elsewhere builds from its spec, the effort moves to
  `.scratch/archive/` and its inbound links are fixed. Evidence docs that are
  no longer current move to `docs/archive/`. The root `.ignore` hides both
  archives from search; name the path to look in them.
- **Text only.** Commit maps, tickets, specs, analyses and small analysis
  scripts. Keep raw captures (pcaps, packet dumps, game logs), screenshots and
  binaries local: captures can hold peers' IP addresses and iroh node IDs. Put
  them in a `captures/` folder inside the effort, `.scratch/<effort>/captures/`,
  which `.gitignore` ignores.
- **Research and prototypes.** A research write-up lands on `main` when its
  ticket closes. A prototype's last commit is tagged `archive/prototype-<name>`
  and its branch deleted. Tickets point at the doc or the tag, never at a
  branch.

The full rules, as agents follow them, are in
[docs/agents/issue-tracker.md](docs/agents/issue-tracker.md).

## Checks

Before you send a change, run `cargo test --workspace` and
`cargo clippy --workspace --all-targets`. If you moved or renamed files, run
`scripts/check-links.py` too: it reports every relative link and backticked
path that no longer resolves.
