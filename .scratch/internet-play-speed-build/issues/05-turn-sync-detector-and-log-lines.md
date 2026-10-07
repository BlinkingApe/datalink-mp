# 05: Turn sync detector and its log lines

**What to build:** One small shared detector finds a **Turn sync** from the game messages (starts at `0x8301`/`0x4301`, ends at `0x4309`), and the page activity will use it too, so the two can't disagree. The Helper logs at info level a line per finished Turn sync and one for the game start: turn number, duration, messages and bytes each way, total time with nothing in flight, and the connection's median RTT over it (from Iroh's `Path::stats()`). Raw figures only, no payloads; when a capture is on, the line names the capture file. Hop time and Early ack counts are added by a later ticket.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] The detector reports start and end of each Turn sync and of the game start on the captured game's messages
- [x] A log line per Turn sync and per game start with the fields above
- [x] The detector has unit tests driven from the captured sequence

## Comments

Built in `crates/datalink-transport/src/turn_sync.rs` (the pure `TurnSyncDetector`, one per friend, and the glue that logs). Hooks: `PeerConnection::enqueue`, `handle_peer_message`'s `GameMessage` arm, and register/unregister beside the path watcher in `spawn_connection_handler`; `capture::path()` names the file. Tests use two trimmed fixtures from the captured game under `crates/datalink-transport/tests/fixtures/`.

The log line is `turn sync finished` / `game start finished` with `peer`, `turn`, `duration_ms`, `out_msgs`, `out_bytes`, `in_msgs`, `in_bytes`, `idle_ms`, `rtt_median_ms` (-1 when no path is selected) and, when a capture is on, `capture`. Message and byte counts are every game message seen in the window, resent copies and acks included, so they are larger than the analysis's unique counts. The game start is found from the first `0x4101` after the lobby's setup messages, an inference from one captured game.
