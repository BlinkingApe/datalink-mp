# 05: Turn sync detector and its log lines

**What to build:** One small shared detector finds a **Turn sync** from the game messages (starts at `0x8301`/`0x4301`, ends at `0x4309`), and the page activity will use it too, so the two can't disagree. The Helper logs at info level a line per finished Turn sync and one for the game start: turn number, duration, messages and bytes each way, total time with nothing in flight, and the connection's median RTT over it (from Iroh's `Path::stats()`). Raw figures only, no payloads; when a capture is on, the line names the capture file. Hop time and Early ack counts are added by a later ticket.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The detector reports start and end of each Turn sync and of the game start on the captured game's messages
- [ ] A log line per Turn sync and per game start with the fields above
- [ ] The detector has unit tests driven from the captured sequence
