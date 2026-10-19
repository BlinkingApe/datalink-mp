# 17: Release Patch B (opt-in)

**What to build:** Release the fixes as a 0.1.x patch with both switches off by default: `DATALINK_EARLY_ACKS=1` (Helper) and `DATALINK_FAST_WAKE=1` (DLL), each testable alone, plus the page activity. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 4. Release notes say how testers turn them on and what to send back (the log, and a capture when asked). They also carry this line from [transport-message-path ticket 02](../../archive/transport-message-path/issues/02-act-on-a-lost-connection-when-it-happens.md): the reconnect to a friend now starts as soon as the loss is seen, and no longer gets lost if you click Host Game or Join Game just after the drop. From [friend-registry](../../archive/friend-registry/spec.md), they also say that after a friend's connection is lost and re-dialled, messages sent straight to that friend reach them again.

**Blocked by:** 12, 13, 14, 15

**Status:** ready-for-agent

- [ ] Version bumped as a patch (no IPC or Peer protocol change) and notes written
- [ ] Both switches default off
- [ ] Release notes carry the earlier-reconnect line
- [ ] Release notes say directed messages reach a re-dialled friend again
- [ ] The release gate passes

## Comments

- 2026-10-19: The friend registry ([friend-registry](../../archive/friend-registry/spec.md)) fixed a bug for this patch: after a friend's connection was lost and re-dialled, a Send addressed to that friend's player failed with "player not found". It bit hardest in games of three or more, between two Joiners. The release notes should mention that directed messages reach a re-dialled friend again. Added to the description and checklist above.
