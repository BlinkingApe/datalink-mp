# Decision brief: What makes a Turn sync slow, and what do we fix?

Prepared for the grilling of the internet-play-speed ticket of that name. What was decided is in [that ticket's answer](../issues/06-what-makes-turn-sync-slow.md); it follows section 4's recommendations, except that the Helper's acks ship **opt-in** in their first 0.1.x patch.

**Sources.** The measured numbers come from the captured Direct game (`.scratch/internet-play-speed/analysis/direct-game-capture.md`) and a rerun of `analysis/analyse_capture.py` on `captures/capture-1791300326-1036127.jsonl`. JACKAL's behaviour comes from `docs/archive/research/smac-jackal-turn-sync.md`. The code facts come from `crates/dplayx/src/directplay.rs` and `crates/datalink-transport/src/runtime.rs`.

**New in this brief: a replay model.** The analysis's "upper bounds" assume each fix removes its whole bucket. That overstates the gain, because some round trips are dependencies that no ack trick removes. For example, the host can't release a barrier before the Joiner's `0x2303` has crossed the network. So I replayed each Turn sync message by message:

- Each side's compute and reaction time is kept exactly as measured.
- Each message waits for what it waited for today: its own previous message's ack, plus every message from the other side that its game had drained before sending.
- Then the network parameters are changed.

With today's parameters the model reproduces the four Turn syncs to within 0.3 s, and the game start to within 0.8 s. The scripts are next to this file: `replay_turn_sync.py` (the model and the main table, run as `python3 replay_turn_sync.py analyse_capture.py <capture>`), `replay_fixes.py` and `replay_relayed.py` (the fix and Relayed tables, run as `python3 replay_fixes.py replay_turn_sync.py analyse_capture.py <capture>`), and `check_acks.py <capture>` (the ack-equals-header check).

The model's assumptions:
- The Joiner's send times are inferred as half an RTT before arrival.
- The hotspot uplink carries 120 KB/s, taken from the 8.3 ms/KB fit. I also checked 40 KB/s.
- The game is two players, one at a time, and compute doesn't change.
- "Relayed" means the same game at the 146 ms median RTT measured on the relay before the game. That's a projection, not a measurement.

---

## 1. The budget of a Turn sync

**Measured** (the analysis's buckets; seconds and % of the Turn sync):

| | Game start | Turn sync 1 (leaders resync) | Turn sync 2 | Turn sync 3 | Turn sync 4 (small vehicles resync) |
|---|---|---|---|---|---|
| Length | 38.6 | 24.7 | 6.1 | 9.1 | 17.0 |
| Compute, host / Joiner | 8.2 / 8.3 (42%) | 14.2 / 2.6 (67%) | 4.6 / 0.1 (78%) | 0.7 / 6.0 (74%) | 12.5 / 0.9 (78%) |
| Round trips (a host message awaiting its ack) | 20.8 (54%) | 7.2 (29%) | 1.2 (19%) | 2.1 (23%) | 3.4 (20%) |
| … of which bytes (unique KB × 8.3–14 ms/KB) | 2.4–4.0 | 0.9–1.5 | ~0.01 | ~0.01 | 0.04–0.07 |
| … of which the Joiner's poll before it acks (~5 ms per host message) | ~1.0 | ~0.5 | ~0.1 | ~0.1 | ~0.2 |
| DLL poll at the host (message waiting for the game) | 1.3 (3%) | 0.7 (3%) | 0.1 (2%) | 0.2 (2%) | 0.2 (1%) |
| Helper queueing (out → written) | < 0.5 ms max | ≈ 0 | ≈ 0 | ≈ 0 | ≈ 0 |
| Reliable messages, host + Joiner | 195 + 12 | 95 + 11 | 20 + 8 | 25 + 11 | 31 + 10 |
| Unique bytes, host | 286 KB | 110 KB | 1.2 KB | 1.4 KB | 5.1 KB |
| Spurious resends, host (bytes) | 141 (224 KB) | 60 (68 KB) | 13 (0.9 KB) | 17 (0.9 KB) | 22 (4.5 KB) |
| QUIC RTT median | 72 ms | 58 ms | 52 ms | 77 ms | 74 ms |

**Replayed** (the model's floor with zero network time, and the share that can be won):

| | Game start | Turn sync 1 | Turn sync 2 | Turn sync 3 | Turn sync 4 |
|---|---|---|---|---|---|
| Floor: compute only | 17.0 | 16.9 | 4.8 | 7.1 | 14.0 |
| **Winnable, Direct** | 20.8 (55%) | 7.8 (32%) | 1.2 (20%) | 2.2 (24%) | 3.3 (19%) |
| Winnable, Relayed (projected 146 ms) | 33.2 | 15.9 | 3.1 | 3.8 | 4.6 |

In short:

- **Direct:** a normal Turn sync loses 1.2–3.3 s to the network, out of 6–17 s.
- **A resync Turn sync** loses 7.8 s. Most of that is 68 two-KB chunks, each paying one round trip, not the bytes themselves.
- **The game start** loses 21 s.
- **Relayed,** each of these roughly doubles (projected).
- The rest is the game computing, mostly the Linux host spending 4–10 s on one phase. No Helper fix touches that.

---

## 2. The candidate fixes

### Summary

Gains are seconds saved per Turn sync from the replay, Direct, with Relayed (projected) in brackets. "TS" is Turn sync; TS 2–4 are the normal ones.

| Fix | Game start | TS 1 (resync) | TS 2–4 (normal) | Touches | Crosses ADR-0004's line? | Risk | Build cost | Earliest release |
|---|---|---|---|---|---|---|---|---|
| **A. Helper acks for the friend's game** (both Helpers) | −18.8 (−30.5) | −6.7, 27% (−13.9, 43%) | −0.8 to −2.2, 13–15% (−2.3 to −3.3, 18–29%) | Helper only | Yes, "invisible to the game". Not "exactly the bytes it sent" | Medium: delivery semantics (below) | Medium: about 3–5 agent days plus 2 test games | 0.1.x patch |
| A, host's Helper only (friend on stock 0.1.0) | −18.6 (−30.0) | −6.6 (−13.6) | −0.7 to −2.1 (−1.9 to −3.0) | as A | as A | as A | as A | as A |
| **A + DLL wakes the game at once** (signal after Send, or a 1 ms poll) | −19.8 (−31.5) | −7.3, 30% (−14.5) | −0.9 to −2.4, 14–16% (−2.4 to −3.5) | + DLL, timing only | + "invisible to the DLL" | Low | Small: a few lines plus a CPU check | 0.1.x patch |
| B. DLL poll 10 → 1 ms alone | −1.9, 5% (−1.8) | −0.9, 4% (−0.9) | −0.1 to −0.3, 2% (−0.2) | DLL | "invisible to the DLL" | Low (CPU) | Trivial | 0.1.x patch |
| B'. Event-driven DLL (blocking IPC wait) | ~ as B | ~ as B | ~ as B | DLL + Helper + **IPC version** | as B | Low–medium | Medium | minor (0.2.0), per ADR-0002 |
| C. Compression (ADR step 2) alone | −2.1, 6% (−2.2) | −0.8, 3% (−0.8) | ~0 | **Peer protocol** | No | Low | Medium: about 1–2 days | minor (0.2.0) |
| C after A | ~0 | 0 at 120 KB/s, −0.8 at 40 KB/s | 0 | as C | No | Low | as C | minor (0.2.0) |
| D. Delta encoding (ADR step 3) | — | about −0.05 on top of C | 0 | Peer protocol | No | Medium (needs the game's message kinds) | Medium–large | minor (0.2.0) |
| E1. Realistic `dwLatency` (e.g. 250 ms) | ≤ −0.2 | ≤ −0.06 | ~0 | DLL | Yes: the game sees a different `GetCaps` | Low | Trivial | 0.1.x patch |
| E2. Helper drops a resend whose original is in flight | ≤ −0.2 | ≤ −0.06 | ~0 | Helper only | No in substance (the friend's game gets fewer copies it would drop anyway) | Low, if tied to the stream's lifetime | Small (part of A) | 0.1.x patch |
| F. Shorter round trip (Direct more often, own relay) | Relayed→Direct is worth 12.4 today, 0.7 after A | 8.1 today, 0.9 after A | 1.3–2.0 today, 0.2–0.6 after A | Infrastructure / Iroh config | No | Low | Large (own relay: hosting and running it) | any |

E1 and E2 cut bandwidth, not waiting, on this link. The host's sends fall from 823 to 484, and its bytes by 43%. The time bound is the 13 resends (7 KB) that went out after the ack had already reached the Helper, which delayed the next message. On a Relayed connection, up to 3 resends per message are expected (unmeasured). With A in place, resends from the acking side stop by themselves, because the ack arrives in about 6 ms, well before the 50 ms timer.

### A. The Helper acks for the friend's game

**How it works.** JACKAL sends one reliable message at a time, and the game thread blocks until every recipient's ack matches the current sequence number (`Net::send_group`'s loop). The ack is a 12-byte echo. I checked all 989 acks in the speed game (984 of kind 2, 5 of the setup kind 0x12): **every one equals the first 12 bytes of the data message, with the kind word rewritten to `(kind & ~4) | 2`**, sent from the recipient's player DPID to the sender's. So a Helper can build the exact ack the friend's game would send.

When its game sends a reliable message (kind & 6 == 4) to a friend, and the frame has been written to that friend's stream, the Helper puts that ack into its own game's inbound queue. It does this the same way it already queues host→server loopback (`runtime.rs:697`). The game moves on to its next message without a round trip. The friend's game still acks for real; the Helper swallows those acks and uses them to know what has really arrived.

**What it wins, and what it can't.** The ack is never what a barrier waits for. The barrier waits for the friend's own message. So every change of direction (host→Joiner→host) still costs a real round trip: 13–22 per Turn sync. A wins the round trips of the runs between those changes:
- the `0x1303`, `0x1303`, `0x4303` triple before each barrier, and the `0x4307` echoes
- above all, the 68-message resync, which goes from 5.9 s to about 0.7 s, then limited by the uplink
- the game start's 167-message state sync

It wins 64–90% of the winnable share on Direct, and 66–95% on Relayed when combined with B.

**It works one-sided.** The Peer protocol doesn't change: the friend's Helper and game behave exactly as before. A host whose Helper has A gets nearly all the gain against a stock 0.1.0 friend (−6.6 s against −6.7 s on Turn sync 1), because the host sends 80% of the reliable messages.

**What the Helper must take over from JACKAL, so this stays correct.** None of these are optional.

1. **Delivery across a reconnect.** Today, if a friend's connection drops and the reconnect driver (`runtime.rs:877`) heals it, anything lost on the old stream is resent by JACKAL within its 20 s. With A, JACKAL thinks it was delivered. So the Helper keeps each message until the friend's real ack arrives, and replays the unacked ones first on the new stream. The friend's JACKAL drops any copy it already has (`seq <= last_seq`), so a replay is harmless.
2. **A friend who's gone.** While a friend's connection is down, the Helper stops making acks for it. The game's own "SEND MESSAGE TIME EXPIRED" and drop path then still fire. Liveness ("PLAYER NOT RESPONDING") runs on the separate kind-0x20 heartbeats, which A doesn't touch.
3. **Games of three or more: keep today's ordering guarantee.** Stop-and-wait guarantees that a message to friend X has arrived before the sender's next message goes to anyone. Without that, X could hear a consequence from a third machine before its cause from the sender. The rule: when the recipients change (a message for Y while one for X is still not really acked), the Helper holds the new message and its ack until the real acks are in. In a two-player game the recipients never change, so this costs nothing there.
4. **A cap on messages in flight** (for example 16, about 30 KB). JACKAL's receive side has never had more than one message per sender in flight, and its queue limit is unknown. 16 messages at 2 KB per 70 ms is still more than the uplink carries.
5. **Resends.** The game may still resend if a made-up ack is slow, because its timer settles at about 26 ms. The Helper drops a resend whose original is in flight on the same stream (E2).
6. **Swallow only real acks that match** a made-up ack (friend, sequence number). Leave the setup's kind-0x14/0x12 exchange alone.
7. **A switch to turn it off,** and capture lines for made-up and swallowed acks, so a test game can be analysed.

**Risk under JACKAL semantics.**
- *Desync from content:* none. The friend's game gets exactly the bytes sent, in the same order, and the ack is byte-identical.
- *What changes is the meaning of an ack.* It now means "on the reliable stream", not "in the friend's JACKAL queue". That gap matters only on connection loss (rules 1 and 2), in 3+ player games (rule 3), and for queue depth (rule 4).
- *Untested territory:* the game has never run with acks arriving in 6 ms, or with more than one message per sender queued at the receiver. It needs a three-instance Wine test (the project's existing setup), a fault-injection test that drops the connection mid-resync, and two real games.

**Unmeasured upside.** Reliable messages sent during a turn (unit moves, and simultaneous moves, which the Apolyton reports single out) pay the same per-message round trip, and gain the same way.

### B. The DLL's poll

The DLL's thread checks `message_count()` over IPC every 10 ms and then signals the game's event (`directplay.rs:309`). Every inbound message waits about 6 ms at each end.

- **Alone,** a 1 ms sleep saves 2–5%. That's small, because the poll is mostly hidden inside the round trip.
- **With A it matters more:** the made-up ack is local, so the poll becomes most of what's left of each message's cost.
- **The cheapest version for A:** `Send` is a synchronous IPC request, and the Helper queues the made-up ack before it answers. So the DLL can simply signal the game's event straight after each `Send` returns. That's DLL-only, with no IPC change.
- **Inbound messages:** a 1 ms poll (DLL-only) or a blocking wait. The blocking wait changes the IPC version, so ADR-0002 makes it a minor release.
- **Windows caveat:** on Windows, a 1 ms sleep may round up to the 15.6 ms timer tick unless the game raised the timer resolution. Measure it there.

### C, D. Bytes

- **Before A,** compression saves the bytes share: 3% of a resync Turn sync, 6% of the game start, nothing on normal Turn syncs.
- **After A,** the resync is limited by the uplink, not by round trips. Compression then matters only if the link is slow. At the measured ~120 KB/s it gains nothing; at 40 KB/s it gains 0.8 s on Turn sync 1.
- **Delta** adds about 4 KB of savings (0.05 s) on top of compression, and it needs to know the game's message kinds.
- Both need a Peer protocol version bump, so both go in a minor release. Neither crosses ADR-0004's line.

### E. Spurious resends

- **The cause:** the 50 ms `dwLatency` starts a spiral. A resend shortens the next timing sample, because the sample is measured from the last copy, so the timer settles at about (ack time + 20)/2, which is below the ack time.
- **E1** starts it high, and the timer then settles at about ack time + 20 ms. It's one line, but the game sees a changed `GetCaps` value, and with A it's redundant.
- **E2** is invisible to both games and is needed by A anyway (rule 5).
- **The safety condition for E2:** drop a resend only if its original went out on the *same, still-open* stream. After a reconnect, the resend may be the only copy that arrives.

### F. Shorter round trip

- **Today,** a Relayed Turn sync costs 1.3–8.1 s more than a Direct one, and the game start 12.4 s more (projected).
- **After A,** it costs 0.2–0.9 s more, because only the changes of direction pay the RTT.
- So after A, our own relay or chasing more Direct connections buys under a second per Turn sync, for a server to run and pay for. ADR-0004 already deferred it.

---

## 3. Recommendation

**Pursue this mix:**

1. **A, with rules 1–7,** as the speed effort's main fix. It's Helper-only, so it can ship in a 0.1.x patch, and it helps even when only the host has upgraded.
2. **B as A's companion:** the DLL signals the game after each `Send`, plus a 1 ms poll for inbound messages, if the CPU cost is acceptable. It's DLL-only, timing only, and also a 0.1.x patch.
3. **E2 inside A.** No `dwLatency` change (E1).
4. **Drop delta encoding (D).**
5. **Defer compression (C).** It's worth building only if a Relayed capture taken after A shows the uplink or relay throughput makes a resync or the game start take ≥ 1 s more. If C is dropped, the speed work needs no Peer protocol change at all.
6. **No own relay (F).**

**Candidate target, against the winnable share** (reference setup: two players, Linux host on the hotspot, Direct):

- **A Turn sync's network time ≤ 1 s**, down from 1.2–3.3 s on a normal Turn sync and 7.8 s on a resync. The model predicts 0.4–0.9 s.
- **On a Relayed connection, ≤ 1.5 s** per Turn sync. The model predicts 0.8–1.5 s.
- **The game start's network time ≤ 2 s**, down from 21 s.

In whole-Turn-sync terms, that's about −30% on a resync turn like Turn sync 1, −13 to −16% on a normal one, and −50% on the game start. The 5–17 s of compute stays.

**How to measure it:** the analysis script's "round trip" bucket stops meaning network time once acks are local. It needs a "one-way hop" bucket, meaning the time between a message leaving one Helper and the reply it causes arriving. "Network time" is then hops plus polls.

**ADR-0004: supersede it, don't amend it.**
- **Its premise is contradicted:** it assumed bytes or the relay, and the data shows per-message round trips.
- **Its order is contradicted:** steps 2 and 3 drop to deferred and rejected.
- **Its central constraint has to be reworded,** and that's a change of decision, not a clarification.

A new ADR, "Internet play speed: Helpers answer JACKAL's acks", would carry steps 1 and 4 over unchanged and restate the constraint as:

> The friend's game receives exactly the bytes the game sent, in the order sent. The Helper never changes a game message. It may answer for the friend's JACKAL, with acks and by dropping redundant resends, because the stream already guarantees delivery. In return, it keeps every message until the friend's game has really acked it, and stops answering for a friend whose connection is down.

It should also record that the release split follows from this: there's no Peer protocol change unless compression comes back.

**Knock-on effects for other tickets:**
- What does the page show while a Turn sync runs? (`.scratch/internet-play-speed/issues/08-activity-on-the-page-during-turn-sync.md`): most of a Turn sync is a machine computing. "Your friend's game is working out the turn" is truer than "Receiving turn data… 340 KB".
- What does the Helper report? (`.scratch/internet-play-speed/issues/07-what-the-helper-reports.md`): A gives the Helper a direct count of hops and real ack times.

---

## 4. Open questions for the human, in order

1. **Can the Helper answer acks for the friend's game?** This crosses ADR-0004's "invisible to the game" line, but keeps "the game gets back exactly the bytes it sent".
   **Recommended: yes**, with rules 1–7. It's the only fix that wins most of the winnable share: 64–90% on Direct, against ≤ 6% for anything else. If no, the rest of the list shrinks to B, C and E, worth about 4–10% of a Turn sync together.
2. **Games of three or more: hold at each change of recipients (rule 3) from the start, or ship for two-player games only and add three-player later?**
   **Recommended: rule 3 from the start.** It costs nothing in two-player games, and it keeps the guarantee today's stop-and-wait gives. Test it on the three-instance Wine setup before release.
3. **Where does A ship?** A 0.1.x patch, on by default after the validation games, with a switch to turn it off? Or hold it for 0.2.0?
   **Recommended: 0.1.x, on by default, with an off switch.** It needs no friend to upgrade, and the host alone gets most of the gain. The validation is a Direct and a Relayed test game, each captured, plus the fault-injection test.
4. **May the speed work change the DLL's timing, which crosses "invisible to the DLL"?** That means signalling after Send and a 1 ms poll.
   **Recommended: yes for those two** (timing only; the game sees no different bytes). **No to changing `dwLatency`**, which A makes redundant and which the game does see.
5. **Bytes (ADR steps 2 and 3).**
   **Recommended: drop delta now.** Make compression conditional on the Relayed capture after A: build it only if bytes still add ≥ 1 s to a resync Turn sync or the game start.
6. **Running our own relay, or chasing Direct connections.**
   **Recommended: no.** After A, Relayed is projected within 1 s of Direct per Turn sync.
7. **The numeric target.**
   **Recommended:** network time per Turn sync ≤ 1 s Direct and ≤ 1.5 s Relayed, and ≤ 2 s at the game start, on the reference setup. It's measured by the analysis script with a one-way hop bucket. It's stated against the winnable share: the 5–17 s of compute is out of reach.
8. **ADR-0004: amend or supersede?**
   **Recommended: supersede** with a new ADR, with the wording in section 3. If question 5 lands on "no compression", it also says the speed work no longer needs a minor release.
9. *(Scope, lower priority.)* **Is the compute share worth its own look in a separate effort?** That's 67–78% of a Turn sync, and the Linux host under Wine was the slow machine in three of four. It's out of this map, since patching the game is out of scope.
   **Recommended:** note it in the map's fog. One cheap comparison would show whether it's Wine: the same kind of turn with Windows hosting.

---

## 5. Are a Relayed capture or the checksum mismatches needed to decide?

**A Relayed capture: not needed to decide. Needed to validate, and to settle compression.**
- Every option ranks the same on Relayed as on Direct; A's lead only grows (43% against 27% on Turn sync 1).
- The projection's main unknown is the relay's throughput. That affects only how much bytes matter after A, which is exactly what question 5's condition waits on.
- So take it as one of A's validation games, rather than as a separate session before deciding.
- Our N0 setup doesn't force relaying, and the hotspot came out Direct. The test needs a dev switch that disables direct paths, or a friend behind a network that blocks them.

**The checksum mismatches: not needed to choose fixes. Worth a short look before A ships.**
- A shrinks a resync's network cost from about 6 s to under 1 s, so how often resyncs happen no longer changes the choice.
- But A changes delivery semantics, and the A/B test games will be judged partly on "no new desyncs".
- Two of five checks already mismatched on stock 0.1.0, with no fix in place. Without a baseline, a mismatch after A can't be told apart from the existing ones.
- A short look at which records diverge (leaders, vehicles, bases) gives that baseline, and might rule our transport in or out. My guess is the game's own divergence, possibly floating point under Wine against Windows, but that's an inference.
- Recommended: make that look a prerequisite of A's validation, not of this decision.
