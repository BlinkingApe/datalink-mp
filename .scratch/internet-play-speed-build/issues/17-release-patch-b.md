# 17: Release Patch B (opt-in)

**What to build:** Release the fixes as a 0.1.x patch with both switches off by default: `DATALINK_EARLY_ACKS=1` (Helper) and `DATALINK_FAST_WAKE=1` (DLL), each testable alone, plus the page activity. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 4. Release notes say how testers turn them on and what to send back (the log, and a capture when asked).

**Blocked by:** 12, 13, 14, 15

**Status:** ready-for-agent

- [ ] Version bumped as a patch (no IPC or Peer protocol change) and notes written
- [ ] Both switches default off
- [ ] The release gate passes

## Comments

- 2026-10-19: The friend registry ([friend-registry](../../archive/friend-registry/spec.md)) fixed a bug for this patch: after a friend's connection was lost and re-dialled, a Send addressed to that friend's player failed with "player not found". It bit hardest in games of three or more, between two Joiners. The release notes should mention that directed messages reach a re-dialled friend again. Added to the description and checklist above.
- 2026-10-19: The earlier-reconnect line (transport-message-path ticket 02) and the directed-messages-reach-a-re-dialled-friend line (friend-registry) now ship in 0.1.2, ahead of this patch. They are removed from the description and checklist above. See [docs/release-notes/0.1.2.md](../../../docs/release-notes/0.1.2.md).
