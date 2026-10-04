# Fork or standalone project?

Type: grilling
Status: resolved
Blocked by:

## Question

The repo on disk is a plain clone of `hdevalence/smac-iroh` (MIT OR Apache-2.0). Should this effort be a GitHub fork, designed to be upstream-friendly, or a standalone project seeded from it? If standalone: what name, what wire identity (ALPN), what starting version, and what's left of the upstream-friendly preference?

## Answer

Resolved 2026-09-30 (grilling). Recorded in [ADR-0003](../../../../docs/adr/0003-standalone-project.md).

- **Standalone, `datalink-mp`**, for the repo and the Helper binary. Both `datalink-iroh` and `datalink-mp` were checked on 2026-09-30: each was free on crates.io, and no GitHub repo used either name. `datalink-mp` was chosen because the name says "multiplayer" to players, while "iroh" is the networking library underneath and means nothing to them; the README credit line covers the lineage from smac-iroh. Keep the git history, with a read-only `upstream` remote. Keep both licence files, and credit smac-iroh in the README. Point the `repository` field at the new repo.
- **ALPN `datalink/<peer protocol version>`**, with the Peer protocol version reset to 1. No interop with smac-iroh builds.
- **First release `0.1.0`**, replacing the `0.2.0` in [Versioning and compatibility policy](08-versioning-and-compatibility.md) and [Release trust posture for v1](09-release-trust-posture.md).
- **Upstream is a courtesy, not a constraint.** The CLI `host`/`join` stay for power users and scripts. Generic fixes may go to smac-iroh through a throwaway fork.

Left for /to-spec: whether internal crate names (`smac-helper`, the `SMAC_HELPER_PORT` env var) are renamed along with the binary.
