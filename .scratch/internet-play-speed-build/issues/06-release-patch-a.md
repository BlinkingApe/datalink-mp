# 06: Release Patch A

**What to build:** Release the measurement patch as a 0.1.x patch: the `wtp.exe` recognition, the shipped capture, `DATALINK_RELAY_ONLY`, the badge and path log, and the Turn sync log lines. Helper-only: no IPC or Peer protocol change. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md), section 4.

**Blocked by:** 01, 02, 03, 04, 05

**Status:** ready-for-agent

- [ ] Version bumped as a patch under ADR-0002 and release notes written
- [ ] The release gate passes
- [ ] Release notes tell testers to send the log, and a capture only when asked
