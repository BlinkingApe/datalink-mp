# Revise and accept ADR-0001

Type: grilling
Status: resolved
Blocked by: 01, 02, 03, 04, 05, 06, 08, 09

## Question

Fold the map's decisions and all resolved tickets into a revised `docs/adr/0001-web-ui-frontend.md` with status Accepted, and flag anywhere it now contradicts the original draft. Also settle the Mac wording of the v1 promise: double-clicking opens a Terminal window, so "without opening a terminal" needs rewording (e.g. "no commands to type; leave the Terminal window open").

Also decide Steam's place in the Linux setup text. The charting decision covered Steam with a Copy button, but the test in [Test the Wine override without winecfg](05-wine-override-test.md) couldn't confirm Steam (Flatseal permissions, then a C++ runtime error). Keep it labelled "should work", or drop it for v1.

Slicing the implementation is **not** part of this ticket. Once the map is clear, it hands off to `/to-spec` and then `/to-tickets`.

## Answer

Resolved 2026-09-30 (grilling).

- **ADR-0001 revised in place and Accepted:** [docs/adr/0001-web-ui-frontend.md](../../../../docs/adr/0001-web-ui-frontend.md). It ends with a "Changes from the draft" section listing the 13 contradictions with the Proposed draft.
- **New ADR-0002, Accepted:** [docs/adr/0002-releases-versioning-pipeline-trust.md](../../../../docs/adr/0002-releases-versioning-pipeline-trust.md). It covers versioning, the release pipeline and the trust posture, which are separate from the UI. The versioning rule is the hard-to-reverse part.
- **The v1 promise now reads "without typing any commands"** (true on every OS). The Mac note ("a Terminal window opens by itself; leave it open") goes in the Mac setup text.
- **Steam:** no Copy button. One untested "should work" line covers Steam and other launchers, with the `%command%` example. The generic string and Faugus (both tested) get the buttons.
- **Two pipeline decisions added in this session:**
  - Pushing a `v*` tag creates a **draft** GitHub Release, with `-rc.N` tags marked pre-release. The maintainer publishes after the RC gate and VirusTotal.
  - The Linux Helper is a **static musl** build (required), so it doesn't depend on the distro's glibc.
- **CONTEXT.md:** added **Game folder** and **Stop**.
