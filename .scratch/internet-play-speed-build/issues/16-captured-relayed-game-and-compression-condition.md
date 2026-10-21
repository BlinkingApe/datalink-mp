# 16: Captured Relayed game and the compression condition (HITL)

**What to build:** Play a captured game with `DATALINK_RELAY_ONLY=1` on one Helper (a tester abroad if one can be scheduled), with early acks on. Judge against the Relayed target (≤ 1.5 s per Turn sync). Then settle the compression condition from this capture and the lossy Direct one from [19](19-captured-game-on-a-lossy-long-distance-link.md): with Early acks on, do bytes still add ≥ 1 s to a resync turn or the game start, on either link? If so, record that compression returns as a new effort (a Peer protocol change, so a minor release); if not, record that the speed work needs no Peer protocol change. See [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 5 and its 2026-10-21 amendment.

**Blocked by:** 03, 15, 19

**Status:** ready-for-human

- [ ] Capture analysed and compared with the ≤ 1.5 s target
- [ ] The compression condition is answered, yes or no, with figures from both this capture and 19's

## Comments

- 2026-10-21: The condition no longer needs a Relayed capture. The [lossy Direct game](../analysis/lossy-direct-game.md) (Russia, about 5% loss) already points to yes: Early acks alone should leave about 6–10 s of bytes in its game start, and the payloads compress to 8–12%. That's an estimate from a game without Early acks, so 19 re-plays that link with them on, and this ticket weighs both captures. Blocked by 19 added.
