# 02: What is publicly known about SMAC's network layer and its Turn sync?

Type: research
Status: resolved
Blocked by:

Research: `docs/archive/research/smac-jackal-turn-sync.md` (commit `c9045fb`).

## Question

SMAC's own network layer (JACKAL, per `docs/contributors/architecture.md`) runs its own reliability over DirectPlay: sequence numbers, acks and retransmits. What can public sources tell us about it? Look at Thinker's source and docs, OpenSMACX or other decompilation projects, PRACX, and community write-ups.

- **Reliability:** is it stop-and-wait or windowed? What window size, and what ack and retransmit timeouts? If acks are slow (for example, over a Relayed connection), does it resend whole messages?
- **Turn sync:** what does the game send between turns? The whole game state, per-faction deltas, or something else? In what size of messages, and in which order?
- **Message format:** is there a header that identifies the message's kind, its sender's turn, or whose turn it is? Step 4 in ADR-0004 shows "whose turn" only if this can be read reliably.
- Is any of it compressed already? That would make compression between Helpers pointless.

It's fine if the answer is "nothing public". Say so, and say which sources were checked. [Analyse the capture](05-analyse-the-capture.md) checks the data against whatever model this finds.

## Answer

Resolved 2026-10-06. Nothing public describes JACKAL's transport. Thinker names the functions and decompiles the turn loop; OpenSMACX only names JACKAL; PRACX has no network code; the Apolyton threads are anecdotes. So the transport was read by disassembling `terranx.exe` v2.0, the binary every player has. The details, each marked fact or inference, and how to reproduce the disassembly, are in `docs/archive/research/smac-jackal-turn-sync.md`.

- **Reliability is strict stop-and-wait: a window of one message** (fact). Each reliable send blocks the game until **every** recipient acks it, or until 20 s pass ("SEND MESSAGE TIME EXPIRED"). An ack is a 12-byte echo of the header, and it's itself a game message passing through the Helpers. The game sends with `dwFlags = 0`, so all reliability is its own.
- **Slow acks mean whole-message resends** (fact), with ×1.5 backoff. The first retransmit timeout is `DPCAPS.dwLatency`, which **our DLL hardcodes to 50 ms** (`crates/dplayx/src/directplay.rs:1166`, confirmed). It then adapts as `(sample + old + 20) / 2`. Inference: on a 250 ms relayed link that's up to 3 spurious resends per message, and the timeout can settle below the true RTT.
- **A Turn sync is lockstep upkeep, not a full-state transfer** (fact). Every machine runs the upkeep itself, then they pass up to 8 phase barriers through the host. On top of that come small per-base and per-leader updates and 21 checksums. Whole sections of game state are resent only when a checksum doesn't match. Messages on the wire are 44 bytes (control), 156 bytes (checksums), and at most 2096 bytes (2 KB state chunks).
- **Inference: latency dominates, not bytes.** A Turn sync costs roughly one round trip to the slowest player per reliable message, plus compute.
- **The header is readable** (fact):
  - An 8-byte JACKAL header (kind word: 4 = data, 2 = ack; sequence number at offset 4) wraps every game message.
  - The game message after it starts with its type and the sender's faction. The barriers carry the phase and the turn number.
  - In one-at-a-time mode the host announces whose turn it is with `0x4309`. In simultaneous mode, the players still to finish are those who haven't sent `0x8301`.
- **Compression:** the game links zlib, but only the save-file transfer (`0x0f0b`) uses it. Turn sync payloads travel raw (fact). So compressing between Helpers isn't pointless, but on messages this small it likely gains little (inference).
- **Two of our own debug decoders read the wrong offset.** The READY_SET/NEXT_TURN labels (`directplay.rs:1731` and `:1869`, confirmed) and the `DPLAYX_RXTRACE` fields read the game's type at wire offset 0, where JACKAL's kind word is. They should read 8 bytes further in. [The capture](03-dev-only-traffic-capture.md) must decode from offset 8, and confirm it.

What this changes downstream:
- [The capture](03-dev-only-traffic-capture.md) and [its analysis](05-analyse-the-capture.md) now have a model to check against: pair each data message with its ack by sequence number, and count retransmits.
- For [the page](08-activity-on-the-page-during-turn-sync.md), "whose turn" looks readable.
- It moves the likely fixes for [What makes a Turn sync slow](06-what-makes-turn-sync-slow.md) away from bytes (see the map's fog).
