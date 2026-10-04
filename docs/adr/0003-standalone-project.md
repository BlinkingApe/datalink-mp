# ADR-0003: Standalone project `datalink-mp`, seeded from smac-iroh

- **Status:** Accepted (2026-09-30)
- **Related:** [ADR-0001](0001-web-ui-frontend.md), [ADR-0002](0002-releases-versioning-pipeline-trust.md). Decision trail: `.scratch/archive/web-ui-v1/issues/10-fork-or-standalone.md`.

This started as a GitHub fork of `hdevalence/smac-iroh` (MIT OR Apache-2.0, © Henry de Valence), designed to be upstream-friendly. Upstream has a single commit and few known users, while this effort adds a web UI, a release pipeline and a new version identity. So this is now **a standalone project, `datalink-mp`, seeded from smac-iroh**, not a fork.

- **Name:** `datalink-mp`, used for the repo and the Helper binary. The Datalink is the game's own encyclopedia, which gives a fan-lore name without using the "SMAC" or "Alpha Centauri" marks. The tagline may say "multiplayer for Sid Meier's Alpha Centauri". Internal crate names may stay; renaming them is the spec's call.
- **Wire identity:** the ALPN is `datalink/<peer protocol version>`, and the Peer protocol version resets to **1**. The project does not interoperate with smac-iroh builds. A shared `dplay-iroh/N` bumped independently by two projects could let incompatible builds accept each other.
- **Version:** the first release is **`0.1.0`**, under ADR-0002's semver rule.
- **Upstream:** being upstream-friendly is no longer a design constraint, only a courtesy:
  - Generic fixes (e.g. deleting the `unwind_stubs` build step, `Transport::shutdown`) may be offered to smac-iroh through a throwaway fork when convenient.
  - No design effort goes into upstream compatibility.
  - `host`/`join` stay unchanged for power users and scripts.
- **Attribution:** keep the smac-iroh git history, with a read-only `upstream` remote. Keep both licence files. The README says "Based on smac-iroh by Henry de Valence". The crate `repository` field points at the new repo (it currently points at `hdevalence/smac`).

## Consequences

- There are no one-click PRs to smac-iroh (GitHub only allows PRs within a fork network), and no "forked from" badge. The README carries the credit.
- Players can't mix `datalink-mp` and `smac-iroh` builds. The Peer protocol mismatch banner covers anyone who tries.
