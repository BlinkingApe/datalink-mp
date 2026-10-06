# 03: A dev-only traffic capture in the Helper

Type: task
Status: resolved
Blocked by: 01

AFK: the agent builds it. It's the one piece of building this map allows (see the map's Notes), and isn't the shipped step 1 feature.

Build: branch `capture/traffic-capture` (commit `b90a677`), code in `crates/iroh-transport/src/capture.rs`, docs in `docs/traffic-capture.md`.

## Question

What exactly crosses the wire during a Turn sync? Nothing records it today. Add a capture to the Helper that's off by default and turned on by an environment variable (for example `DATALINK_CAPTURE=<dir>`). For each `GameMessage`, in both directions, it records:

- the time, the direction, the friend, the DPIDs, the flags (guaranteed or not), and the payload's size
- for an inbound message, a second time: when the DLL handed it to the game (the IPC drain)
- the raw payload bytes, in a file the analysis can replay offline
- the connection's type (Direct or Relayed), and its round-trip time and traffic stats sampled every second or so, using what [ticket 01](01-iroh-connection-type-and-stats.md) found

Decode the game's bytes as [the JACKAL research](02-what-is-known-about-jackal-and-turn-sync.md) found them:
- the kind word is at wire offset 0 (4 = data, 2 = ack), and the sequence number at offset 4
- the game's message type and the sender's faction start at offset 8, **not** offset 0 as our existing debug labels assume

Log those fields per message too, so the analysis can pair each data message with its ack and spot retransmits (the same sender and sequence number seen more than once).

Keep it simple and out of the shipped path: no page changes, no Peer protocol change. It must still play against a stock `v0.1.0` Helper. Write down how to turn it on, and the capture's file format, for [Play a captured internet game](04-play-a-captured-internet-game.md) and [Analyse the capture](05-analyse-the-capture.md).

## Answer

Built 2026-10-06 on branch `capture/traffic-capture` (commit `b90a677`). How to turn it on and the full file format are in `docs/traffic-capture.md` on that branch.

- **Turning it on:** start the Helper with `DATALINK_CAPTURE=<dir>`. Only the Helper changes: the DLL and IPC are untouched, and the Peer protocol is unchanged, so the other machine stays on stock `v0.1.0`. Each Helper process writes one `capture-<unix secs>-<pid>.jsonl`. Captures go in `captures/` here, which this directory's `.gitignore` already covers.
- **Format:** JSON Lines, each line with `ev` and `t_us` (microseconds on a monotonic clock).
  - `out`, `in` and `loop` lines hold each `GameMessage`. `loop` is the host's game sending to its own server player. Each line has an `id`, the friend (`peer`), `from` and `to`, `guaranteed`, `size`, the whole payload as `hex`, and the JACKAL `kind` and `seq`. For data (kind bit `0x4`) they also have the game's `type` and the sender's `faction`, read at wire offsets 8 and 12.
  - `written` (the frame went into the friend's QUIC stream) and `drain` (the DLL took it for the game) refer back to an `id`.
  - `path` lines record iroh's path events: opened, closed, selected or lagged, each with its route (direct or relayed) and address.
  - `stats` lines sample the selected path once a second: route, RTT, congestion window, bytes and datagrams each way, losses, congestion events and MTU.
- **Timing, one more stamp than the ticket asked for.** An outbound message has two times: when the Helper queued it (the game's `Send`), and when the writer task handed its frame to QUIC. A gap between them is Helper queueing, which a latency diagnosis needs to rule in or out. An inbound message's `drain` happens when the game calls `Receive` or `GetMessageCount`, because the DLL then takes everything waiting.
- **Checked:** `cargo test --workspace` passes, including new unit tests for the header decoding and the line format. A run of the two-endpoint mesh test with the capture on recorded all 200 messages with every event, paired by id: `out`, `written`, `in` and `drain`. It also logged path opened, selected and closed events with direct routes, and stats lines with RTT and byte counts.
- **Not yet checked:** a real game. The header offsets from the JACKAL research, the `dwFlags = 0` claim and Relayed routes are first seen in [Play a captured internet game](04-play-a-captured-internet-game.md).

What this changes downstream:

- [Play a captured internet game](04-play-a-captured-internet-game.md): the Linux machine runs this branch's release Helper (`cargo build --release -p datalink-mp`) with `DATALINK_CAPTURE` set. The Windows machine changes nothing.
- [Analyse the capture](05-analyse-the-capture.md): pair data with acks by `seq` going the other way; a retransmit is the same sender and `seq` seen twice. The gaps between `out` and `written` (Helper queue), `in` and `drain` (waiting on the game), and from data to its ack (network) separate the three delays.

## Comments

2026-10-06, after resolving: extended on the same branch (commit `fb1d579`) so a captured game also informs two questions from other efforts: [Does SMAC mark a started game closed to new players?](../../post-0.1.0-polish/issues/05-does-smac-mark-a-started-game-closed.md) and [Tell the game when a friend's connection drops](../../game-session-sync/issues/05-tell-the-game-when-a-friends-connection-drops.md). New lines:
- `ctl`: the Peer protocol's other messages, in and out;
- `session_desc`: each `SetSessionDesc`, with whether joining is disabled;
- `sys`: each DirectPlay system message the DLL drains for the game;
- `send_failed`: game sends the Helper couldn't queue;
- `lost` and `reconnect`: a lost connection and each re-dial.

The writer now flushes as soon as no lines are waiting, so a killed Helper keeps its last lines. Nothing on the wire changed. [Play a captured internet game](04-play-a-captured-internet-game.md) lists the extra steps.

