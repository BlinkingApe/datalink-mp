# Traffic capture (dev only)

A Helper can record every game message it passes on, in both directions, plus
each connection's route and stats. It exists to measure what makes a **Turn
sync** slow (`.scratch/internet-play-speed`), and isn't a player feature. It
also records what explains a session's life: the session settings the game
sets, the system messages the game is handed, failed sends and lost
connections. That answers whether SMAC closes a started game to new players
(`.scratch/post-0.1.0-polish` 05) and what the game is told when a friend's
connection drops (`.scratch/game-session-sync` 05). It's
off unless you turn it on, and changes nothing on the wire: a capturing Helper
plays against a stock `v0.1.0` Helper.

## Turning it on

Set `DATALINK_CAPTURE` to a directory when you start the Helper:

```bash
cargo build --release -p datalink-mp
DATALINK_CAPTURE=.scratch/internet-play-speed/captures ./target/release/datalink-mp
```

The DLL stays as it is (the IPC is unchanged). The Helper creates the
directory if needed and logs `traffic capture on` with the file's path. If it
can't open the file, it logs an error and plays on without a capture. Unset or
empty, the capture is off and the Helper does nothing extra.

To analyse a capture, run `python3 .scratch/internet-play-speed/analysis/analyse_capture.py <capture.jsonl>`;
its findings for the first captured game are in
`.scratch/internet-play-speed/analysis/direct-game-capture.md`.

Each Helper process writes one file, `capture-<unix secs>-<pid>.jsonl`. It holds
every game from that process, one after another. In a two-player game one
capturing Helper sees all the traffic, so only one side needs it.

Captures contain game state and can be large (a few hundred bytes per message).
Keep them in `.scratch/internet-play-speed/captures/`, which is gitignored, and
commit only summaries.

Lines are flushed to the file as soon as none are waiting, so a Helper that's
closed or killed loses at most the last moment's lines.

## File format

JSON Lines: one JSON object per line, in the order the Helper recorded them.
Every line has `ev` (what happened) and `t_us`: microseconds since the capture
started, from a monotonic clock. Compare times within one file only. Key order
within a line isn't meaningful. `peer` is the friend's endpoint id, shortened to
8 hex digits as in the Helper's log.

This is format `1`. The `start` line carries the number, and it changes if a
field changes meaning.

### `start`

The first line. `format`, `unix_ms` (the wall-clock time at `t_us` 0) and `pid`.

### Game messages: `out`, `in`, `loop`

One line per `GameMessage`, with an `id` that the later `written` and `drain`
lines refer to.

| `ev` | When it's recorded |
|---|---|
| `out` | The game sent it and the Helper queued it for `peer`. A broadcast gets one `out` line per friend, each with its own `id`. |
| `in` | Its frame was read from `peer`'s stream and decoded. |
| `loop` | The host's game sent it to its own server player. It doesn't leave the machine, and `peer` is `null`. |

Fields:

- `from`, `to`: the DPIDs, as numbers.
- `guaranteed`: whether the game asked for `DPSEND_GUARANTEED`. JACKAL is expected to send everything with flags `0`.
- `size`: the payload length in bytes.
- `hex`: the whole payload, lowercase hex. This is what an analysis replays.
- `kind`, `seq`: the JACKAL header, read from payloads of 8 bytes or more. `kind` is the u16 at offset 0 (4 = reliable data, 2 = ack), `seq` the u32 at offset 4.
- `type`, `faction`: only when `kind` has bit `0x4` (data). `type` is the game's message type, the u16 at offset 8, as a hex string such as `"0x2303"`. `faction` is the sender's faction, the u32 at offset 12. Acks have neither, because the bytes after an ack's header are never written.

All values are little-endian. These offsets come from
`docs/research/smac-jackal-turn-sync.md` (on branch
`research/smac-jackal-turn-sync`) and still need checking against a real
capture. `hex` keeps the raw bytes, so a different reading can be applied later.

To find retransmits, look for the same sender (`peer` for `in`, the capturing
side for `out`), `kind` 4 and `seq` more than once. To pair a data message with
its ack, match the ack's `seq` going the other way.

### `written`

`id`, `t_us`: an `out` message's frame was written to the friend's QUIC stream.
"Written" means QUIC accepted it into its send buffer, not that it left the
machine. A gap after `out` means the Helper's own queue to that friend was
backed up, for example by QUIC flow control. An `out` with no `written` never
went out: the friend's connection had closed.

### `drain`

`id`, `t_us`: the DLL took an `in` or `loop` message from the Helper. The DLL
takes everything waiting whenever the game calls `Receive` or
`GetMessageCount`, so this is when the game started reading its inbox. The
game reads the message during that call or a following `Receive` from the
DLL's own queue. The gap from `in` to `drain` is time spent waiting on the
game, not on the network.

### `path`

One line per path change on a friend's connection, from iroh's
`Connection::path_events()`. `change` is `opened`, `closed`, `selected` or
`lagged`.

- `opened`, `closed`, `selected` carry `path` (iroh's path id), `route` (`direct` for an IP path, `relayed` for a relay path) and `addr` (the IP address and port, or the relay's URL).
- `closed` also has the path's final `rtt_us` and `lost_packets`.
- `lagged` has `missed`: iroh dropped that many events. The next `stats` line shows the current state.

A connection is a **Direct connection** while its selected path is `direct`.
The relay path usually stays open as a backup, so an `opened` relay path
doesn't mean the connection is relayed.

### `stats`

The selected path of each friend's connection, sampled once a second:

- `route`, `addr`, `path`: as in `path`. `route` is `none` when no path is selected yet.
- `rtt_us`: iroh's RTT estimate.
- `cwnd`: the congestion window, in bytes.
- `tx_datagrams`, `tx_bytes`, `rx_datagrams`, `rx_bytes`: UDP traffic on this path so far.
- `lost_packets`, `lost_bytes`, `congestion_events`: loss on this path so far. iroh has no retransmit counter, so these are the closest signals.
- `mtu`: the path's current maximum UDP payload.
- `open_paths`: how many paths the connection has open.
- `conn_tx_bytes`, `conn_rx_bytes`, `conn_lost_packets`: the same counters summed over every path the connection has had.

Counters only grow, so take differences between samples for rates.

### `closed`

`peer`: that friend's connection closed, and its `stats` lines stop. A
reconnect starts new `path` and `stats` lines for the same `peer`.

### `ctl`

A Peer protocol message other than a game message, sent to or received from
`peer` on the ordered stream. `dir` is `out` or `in`, and `msg` is the
message's name: `PlayerLeft`, `SessionClosed`, `SessionDescUpdate`,
`PlayerDataUpdate` and so on. Most also have `player` (the DPID the message is
about). A `SessionDescUpdate` has the session fields listed under
`session_desc`. A `PlayerDataUpdate` has the data's `size`. The join and
session-query exchanges, which use their own short streams, aren't recorded.

### `session_desc`

The game called `SetSessionDesc`. `host` says whether this Helper is the host:
only the host's settings are applied and sent on to friends, as a
`SessionDescUpdate`. Fields:

- `flags`: the session's flags as a hex string.
- `new_players_disabled`, `join_disabled`: whether `DPSESSION_NEWPLAYERSDISABLED` (`0x1`) and `DPSESSION_JOINDISABLED` (`0x20`) are set.
- `current_players`, `max_players`, `name`.

### `sys`

The DLL drained a DirectPlay system message for the game, at the same moment a
`drain` line would be written. `sys` is its name, for example
`CREATEPLAYERORGROUP`, `DESTROYPLAYERORGROUP`, `SESSIONLOST`,
`SETPLAYERORGROUPDATA` or `SETSESSIONDESC` (`other` for the rest), and
`sys_type` its number. Messages about one player have `player` (its DPID).
`size` and `hex` are as for game messages. A system message the Helper queued
but the game never drained has no line.

### `send_failed`

The game sent a message that the Helper couldn't queue to anyone, for example
to a player whose connection was lost. It has the fields of an `out` line
except `peer`, plus `error`.

### `lost` and `reconnect`

`lost`: the Helper learned that `peer`'s connection was lost. `host` says
whether that friend was the session's host. Losing the host means the game is
handed `SESSIONLOST`. Losing anyone else starts up to five re-dials, each
recorded as a `reconnect` line with `attempt` (1–5) and `ok`.

## The Helper's log

Independent of the capture, the Helper logs at info a `turn sync finished` or `game start finished` line per friend, with raw figures (`.scratch/internet-play-speed-build` 05). With a capture on, the line names the capture file.

## Where it lives

`crates/iroh-transport/src/capture.rs`. The message path calls it at four
points: queueing to a friend (`ConnectionManager`), writing the frame (the
ordered writer task), reading a frame (`handle_peer_message`) and the DLL's
drain (`Transport::receive`). `Transport` also calls it for the game's
session settings, failed sends, lost connections and re-dials. Each call
returns at once when no capture is running.
