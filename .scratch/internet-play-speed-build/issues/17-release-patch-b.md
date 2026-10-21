# 17: Release Patch B (opt-in)

**What to build:** Release the fixes as a 0.1.x patch with all three switches off by default: `DATALINK_EARLY_ACKS=1` (Helper), `DATALINK_FAST_WAKE=1` (DLL) and `DATALINK_CONGESTION=bbr` (Helper, from [20](20-congestion-controller-switch.md)), each testable alone. The page activity ([12](12-page-activity-and-tab-title.md)) ships too if it's done, but doesn't hold up the release. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 4. Release notes say how testers turn the switches on and what to send back (the log, and a capture when asked).

**Blocked by:** 13, 14, 15, 20

**Status:** ready-for-agent

- [x] Version bumped as a patch (no IPC or Peer protocol change) and notes written (0.1.4, `docs/release-notes/0.1.4.md`)
- [x] All three switches default off
- [ ] The release gate passes

## Comments

- 2026-10-19: The friend registry ([friend-registry](../../archive/friend-registry/spec.md)) fixed a bug for this patch: after a friend's connection was lost and re-dialled, a Send addressed to that friend's player failed with "player not found". It bit hardest in games of three or more, between two Joiners. The release notes should mention that directed messages reach a re-dialled friend again. Added to the description and checklist above.
- 2026-10-19: The earlier-reconnect line (transport-message-path ticket 02) and the directed-messages-reach-a-re-dialled-friend line (friend-registry) now ship in 0.1.2, ahead of this patch. They are removed from the description and checklist above. See [docs/release-notes/0.1.2.md](../../../docs/release-notes/0.1.2.md).
- 2026-10-21: Dropped 12 from Blocked by. ADR-0005 section 4 says the page activity never holds up the fixes. Added 20, the congestion controller switch, as the third opt-in switch (ADR-0005's 2026-10-21 amendment).
# 17: Release Patch B (opt-in)

**What to build:** Release the fixes as a 0.1.x patch with all three switches off by default: `DATALINK_EARLY_ACKS=1` (Helper), `DATALINK_FAST_WAKE=1` (DLL) and `DATALINK_CONGESTION=bbr` (Helper, from [20](20-congestion-controller-switch.md)), each testable alone. The page activity ([12](12-page-activity-and-tab-title.md)) ships too if it's done, but doesn't hold up the release. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 4. Release notes say how testers turn the switches on and what to send back (the log, and a capture when asked).

**Blocked by:** 13, 14, 15, 20

**Status:** ready-for-agent

- [x] Version bumped as a patch (no IPC or Peer protocol change) and notes written (0.1.4, `docs/release-notes/0.1.4.md`)
- [x] All three switches default off
- [ ] The release gate passes

## Comments

- 2026-10-19: The friend registry ([friend-registry](../../archive/friend-registry/spec.md)) fixed a bug for this patch: after a friend's connection was lost and re-dialled, a Send addressed to that friend's player failed with "player not found". It bit hardest in games of three or more, between two Joiners. The release notes should mention that directed messages reach a re-dialled friend again. Added to the description and checklist above.
- 2026-10-19: The earlier-reconnect line (transport-message-path ticket 02) and the directed-messages-reach-a-re-dialled-friend line (friend-registry) now ship in 0.1.2, ahead of this patch. They are removed from the description and checklist above. See [docs/release-notes/0.1.2.md](../../../docs/release-notes/0.1.2.md).
- 2026-10-21: Dropped 12 from Blocked by. ADR-0005 section 4 says the page activity never holds up the fixes. Added 20, the congestion controller switch, as the third opt-in switch (ADR-0005's 2026-10-21 amendment).
