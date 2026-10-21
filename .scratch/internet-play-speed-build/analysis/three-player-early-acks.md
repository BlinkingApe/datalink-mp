# Analysis: Early acks with three or more players

- **Date:** 2026-10-21
- **Status:** for later consideration. Nothing here is planned or ticketed.
- **Captures:** the three-player test of 2026-10-20 (v0.1.3-rc.1, `wtp.exe`, Windows host, two Linux Joiners, all on one LAN), in the archived alien-crossfire-fixes effort, under `captures/2026-10-20-v0.1.3-rc.1-test4/` (gitignored). One capture per machine, of the same game: the host's `capture-1792479510-3076.jsonl`, Old_Black's `capture-1792480100-5799.jsonl` and Rocky's `capture-1792479641-361229.jsonl`. DPIDs: host `0x10000`, Rocky `0x20000`, Old_Black `0x30000`.
- **Reproduce:** `python3 analysis/fan_out.py <capture> ...`
- **Context:** [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 2 limits Early acks to two-player games.

The game was on a LAN, so these captures show who sends what to whom, not timing. Facts are measured. **[inference]** marks a reading of them.

## TL;DR

1. **ADR-0005's safe rule for three players would almost never apply.** The rule is to early-ack only while every message in flight and the new one are for the same single friend. 80% of the host's reliable payloads (296 of 370), carrying 96% of its bytes (407 of 422 KB), went to both friends.
2. **The ordering problem it guards against is real in this game.** Each Joiner sent the other Joiner reliable messages: game data records (`0x4101`) and `0x4401`/`0x4404` messages, 20–27 payloads each, all addressed to every player. A Joiner that early-acked would let these arrive at the other Joiner ahead of the host messages they followed.
3. **Routing all game messages through the host's Helper** would make Early acks safe for messages sent to every player. It needs a Peer protocol change, and so a minor release.
4. **The gain is unmeasured.** It should matter most when one player has a slow or lossy link, because today every message sent to everyone waits for the slower ack. No three-player internet game has been captured.

## Who sent what

| Sender | Reliable payloads | To both friends | To one friend | Bytes to both / one |
|---|---|---|---|---|
| Host | 370 | 296 | 74 | 407 KB / 15 KB |
| Old_Black (Joiner) | 42 | 20 | 22 | 1.4 KB / 1.6 KB |
| Rocky (Joiner) | 49 | 27 | 22 | 1.9 KB / 1.6 KB |

Payloads are counted once, with resends merged.

- **The host** sent its game data records (`0x4101`, 250 payloads) and all its Turn sync releases (`0x4301`, `0x4303`–`0x4309`) to every player. Its messages to one friend were `0x1303` (36 to each Joiner) and one `0x0f0d` to each.
- **Each Joiner** sent its turn barriers and end of turn (`0x2303`, `0x2305`, `0x2307`, `0x2101`, `0x8301`, `0x2f04`) to the host only. It sent its game data records (`0x4101`) and `0x4401`/`0x4404` to every player, so the other Joiner received them directly.
- A few resends went to one friend only: JACKAL resends a message for every player only to the recipient that hasn't acked yet.

## Why two-player Early acks don't extend

Today JACKAL sends one message at a time, and waits for every recipient's ack before the next. So when Joiner X receives the host's M2, Joiner Y already has M1. Anything X sends Y because of M2 reaches Y after M1.

With Early acks for messages to every player, the host's game can send M2 before M1 has reached Y. Each friend's stream keeps its own order, but nothing orders the host-to-Y stream against the X-to-Y stream. X can get M2, send a `0x4101` to every player, and have it reach Y before M1 does. **[inference]** How the game reacts to that is unknown; it has never seen it.

ADR-0005's safe rule avoids this by early-acking only messages to a single friend. Per the table, that leaves only 4% of the host's bytes.

## Two designs that would be safe

Both are **[inference]**, not prototyped.

**A. Route every game message through the host's Helper.** A Joiner's Helper sends everything to the host's Helper, which forwards messages for the other Joiners onto their streams. Each Joiner then gets all its game messages from one machine on one ordered stream. Suppose Y receives a message N that X sent after getting M2. The host's Helper put M1 and M2 on Y's stream before N arrived from X, so Y gets them first. Then:
- The host's Helper can early-ack a message for every player once it's queued on every friend's stream, using the two-player rules: the cap, swallowing real acks, dropping resends.
- A Joiner's Helper talks only to the host, so the two-player rules already fit it.
- Open details:
  - acks for a Joiner's message to every player come from the host and the other Joiner, so real acks have to be forwarded back and swallowed by the sender's Helper;
  - host migration moves the routing point, and anything in flight at that moment needs a rule;
  - the host's upload carries the Joiners' traffic twice, though per the table that's under 4 KB per Joiner per game.
- Cost: a Peer protocol change (minor release), an extra hop for Joiner-to-Joiner messages (a small share of traffic), and a second reliability change to test after the two-player one.

**B. Order deliveries with stamps.** Each Helper stamps every game message with how far it had delivered each other sender's messages when it sent it. The receiving Helper holds a message until those earlier messages have been delivered. There's no extra hop, but every Helper keeps per-sender state, and it's a Peer protocol change too. It's harder to reason about than A.

## The likely gain

**[inference]** Each message for every player waits for the slower of the two acks, so a three-player game moves at the pace of its worst link. In the [lossy Direct game](lossy-direct-game.md) a 2 KB message took 945 ms to be acked. With one such player in a three-player game, every host broadcast would pay that. Early acks there would recover a large share of the round-trip time, as in the two-player case, but the size depends on a three-player internet game that hasn't been captured.

Compression and the congestion controller switch ([20](../issues/20-congestion-controller-switch.md)) don't depend on message order. They help three-player games as they are.

## When to revisit

- Two-player Early acks have proved safe in real games: [18](../issues/18-turn-on-by-default.md) has flipped them on by default.
- And three-player internet games with a slow or distant player are common enough to matter. A captured one, run through `analyse_capture.py`, would size the gain.

Then design A first, as the simpler of the two to reason about, and prototype it on the three-instance Wine setup.
