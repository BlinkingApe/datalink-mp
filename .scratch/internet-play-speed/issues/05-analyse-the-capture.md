# 05: Analyse the capture

Type: task
Status: needs-triage
Blocked by: 02, 04

AFK: the agent analyses the capture offline and commits a summary. The raw captures stay gitignored.

## Question

What does a Turn sync consist of, and where does its time go? From [the captured game](04-play-a-captured-internet-game.md), find:

- **Turn syncs:** how to find each one in the traffic (an idle gap, or a message kind), and each one's duration, message count and bytes, per direction.
- **Payloads:** the size distribution. How well do the large ones compress, with a few candidates (for example zstd, lz4 and deflate, at their usual levels)? How much do they shrink when encoded as a delta from the previous message of the same kind, before and after compression?
- **Time:** network round-trip time against the gaps between messages. Does the game send in bursts that then wait for something, the way windowed acks would? Using [what's known about JACKAL](02-what-is-known-about-jackal-and-turn-sync.md), is it the game's own pacing? How long do inbound messages sit in the Helper before the DLL drains them?
- **Retransmits:** repeated identical payloads in a short window.
- **Connection:** Direct or Relayed, and whether that changed during the game.
- **A rough budget:** for one slow Turn sync, how much of its time each cause accounts for (bytes on the wire, round trips, Helper queueing, game pacing).

Check the data against [the JACKAL research](02-what-is-known-about-jackal-and-turn-sync.md), and answer its open questions (section 9 of `docs/research/smac-jackal-turn-sync.md`):
- Does every send arrive with `dwFlags = 0`?
- How many reliable messages and barriers make up a Turn sync?
- How many retransmits are there, and are they spaced at 50 ms × 1.5ⁿ?
- What fraction of a Turn sync is round trips against compute?
- Are the header offsets right?

Commit the summary as a file in this directory, and link it from the answer. It feeds [What makes a Turn sync slow, and what do we fix?](06-what-makes-turn-sync-slow.md).
