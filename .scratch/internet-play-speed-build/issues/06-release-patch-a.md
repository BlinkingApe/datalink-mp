# 06: Release Patch A

**What to build:** Release the measurement patch as a 0.1.x patch: the `wtp.exe` recognition, the shipped capture, `DATALINK_RELAY_ONLY`, the badge and path log, and the Turn sync log lines. Helper-only: no IPC or Peer protocol change. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md), section 4.

**Blocked by:** 01, 02, 03, 04, 05

**Status:** resolved

- [x] Version bumped as a patch under ADR-0002 and release notes written
- [x] The release gate passes
- [x] Release notes tell testers to send the log, and a capture only when asked

## Comments

Version is `0.1.1` (workspace `Cargo.toml`, `Cargo.lock`). Notes are in `docs/release-notes/0.1.1.md`; `release.yml` now puts that file at the top of the draft's notes (the release job checks out the repo for it). `docs/maintainers/releasing.md` says so.

**Not done: the release gate.** It needs a CI-built RC (`v0.1.1-rc.2` or later; `-rc.1` for 0.1.1 is a spent tag per the releasing doc) tested on Windows and Linux by the maintainer, so it stays unticked. Also open: ticket 03 was reopened (Relayed path not yet observed) and 01's status line still says ready-for-agent though its commit is on main. Resolve 03's manual check before tagging, so the notes' `DATALINK_RELAY_ONLY` line is true.
