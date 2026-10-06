# 04: Play a captured internet game

Type: task
Status: resolved
Blocked by: 03

HITL: the maintainer plays, and the agent prepares the build and checks the files.

## Question

We need one real internet game's traffic. The game is played with the capture on, on the reference setup in the map's Notes.

## Needed

- [x] Agent: a local Linux build with [the capture](03-dev-only-traffic-capture.md), and the exact command to start it with the capture on, writing to `.scratch/internet-play-speed/captures/` (gitignored).
- [x] Maintainer: the Linux machine runs that build, and the Windows machine runs stock `v0.1.0`. One machine is on the mobile-data hotspot. Play at least five turns, past the opening. Note roughly how long each Turn sync felt, and which turns felt slow.
- [ ] (skipped) Maintainer, if it can be scheduled: the same with a friend abroad, the Linux side capturing.
- [x] Agent: check the capture files are complete and readable, and record here what was played: the setup, the number of turns, the capture files, and the connection type the capture saw.

## Riding along: two questions from other efforts

The capture also records the session settings the game sets, the system messages it's handed, failed sends and lost connections (`docs/traffic-capture.md`, branch `capture/traffic-capture`). So this session can answer two open questions for the cost of a few extra minutes. Both are optional, and neither changes the speed game.

- **[Does SMAC mark a started game closed to new players?](../../post-0.1.0-polish/issues/05-does-smac-mark-a-started-game-closed.md)** costs nothing extra: starting the speed game from Multiplayer Setup is the experiment. The capture's `session_desc` lines (if Linux hosts) or `ctl` `SessionDescUpdate` lines (if Linux joins) show whether SMAC sets `DPSESSION_NEWPLAYERSDISABLED` or `DPSESSION_JOINDISABLED` when the game starts.
- **[Tell the game when a friend's connection drops](../../game-session-sync/issues/05-tell-the-game-when-a-friends-connection-drops.md)** needs its two reproductions, each after the speed game is over. The capturing Linux machine must be the side whose friend drops:
  - [x] Maintainer, **Joiner's Helper goes away mid-game:** for the speed game, Linux hosts and Windows joins. After the speed turns, quit the Windows Helper. On Linux, wait about 30 s without acting and note what the game shows. Then make a move, note the messages, and choose Drop.
  - [x] Maintainer, **killed host:** swap roles. Windows hosts, and Linux joins and waits in Multiplayer Setup. Kill the Windows game (Task Manager → End task). On Linux, wait about 30 s, note what the setup screen does, and then quit.
  - [x] Agent: from the capture, record on that ticket what each game was handed (`sys` lines), and when, relative to the `lost` and `reconnect` lines and any `send_failed` lines.

  So the recommended setup for the speed game is **Linux hosts**. If a capture alone leaves a lead unconfirmed, that ticket still asks for both sides' `dplayx.log`, since the capture sees only the Linux side.


## Answer

2026-10-06. The game was played and captured. The capture is complete and readable. The connection was **Direct for the whole game**, so no Relayed game has been captured yet.

### What was played

- **Setup:** Linux (Rocky Faugus) ran the capturing build from `capture/traffic-capture` on the mobile-data hotspot. Windows 11 ran stock `v0.1.0` on home Wi-Fi. Linux hosted a session named `TEST`. The game ran one player at a time. The game with a friend abroad was skipped.
- **Turns:** five, from the maintainer's notes. The capture puts the game's start at 17:34:05, when the first data message went out (the maintainer noted 17:33). Turn ends: 17:37 (turn 1, both players), then 17:39, 17:40, 17:41 and 17:43. No turn was recorded as slow (B9 and B10 were skipped), so the analysis has clock times, not felt slowness.
- **Afterwards:** both reproductions for game-session-sync 05 (see below). C3 is incomplete because the Joiner dropped during their own turn.

### Capture files (in `captures/`, gitignored)

- `capture-1791300326-1036127.jsonl`: **the capture**. 2.5 MB and 6,762 lines, 17:25:26 to 17:52:28, with all three sessions in one file: the speed game, the dropped Joiner and the killed host. The Helper was started from the game directory, so the relative `DATALINK_CAPTURE` path wrote it to `~/Games/AC-WTP_431/.scratch/internet-play-speed/captures/`. It was copied here, and the original is still there.
- `capture-1791300092-1035596.jsonl`: an earlier Helper run from the repository with no game. It holds only its `start` line, so it can be ignored.
- **No `helper-linux.log`.** `SMAC_HELPER_LOG_FILE` was also relative. The Helper opens the log before the capture creates the directory, and opening it doesn't create directories, so it most likely failed silently. No DLL logs were made (A4 was skipped). Next time, start the Helper with absolute paths.

### Checks

- Every line parses as JSON. Every `out` has its `written` (1,417 each), and every speed-game `in` was drained. The 37 undrained `in` lines are all from the two short sessions afterwards, when Linux was the Joiner.
- Two pairs of lines are out of `t_us` order, each by 4 µs. That's harmless.
- Speed game totals: 1,342 messages out and 1,315 in, with the peer `ec0d030a`. The connection's counters show 1.04 MB sent and 271 KB received, 6 lost packets and no congestion events.

### Connection type the capture saw

- **The whole game ran on a Direct connection:** one IPv6 path to the Windows machine, from 17:34:05 until the connection was lost at 17:44:04. In Multiplayer Setup, before the game, the direct path closed once and the connection fell back to the relay (`euc1-1.relay.n0.iroh.link`) for 18 s (17:31:39 to 17:31:57), then went Direct again.
- RTT during the game: median 68 ms, p90 124 ms, min 41 ms, max 509 ms. On the relay before the game: median 146 ms.
- So the hotspot did give a Direct connection, as the map's Notes warned it might. A Relayed game is still unmeasured.

### Seen in passing, for [Analyse the capture](05-analyse-the-capture.md)

These are pointers, not findings:
- Data messages go up to 2,688 bytes, larger than the research's ≤2 KB.
- At the game's start, the same data `seq` goes out 2 to 4 times, 50 to 150 ms apart.
- A 12-byte kind-32 message goes each way every 2 s, sent to `DPID_ALLPLAYERS`. Kinds 18 and 20 also appear.
- Every message has `guaranteed: false`.

### Riding along

- [Does SMAC mark a started game closed to new players?](../../post-0.1.0-polish/issues/05-does-smac-mark-a-started-game-closed.md): yes, with `DPSESSION_JOINDISABLED`, not `NEWPLAYERSDISABLED`. Details are in that ticket's comments.
- [Tell the game when a friend's connection drops](../../game-session-sync/issues/05-tell-the-game-when-a-friends-connection-drops.md): when a Joiner's Helper went away, the host's game was told nothing and got no send error. In the killed-host case, the Joiner's game was handed `SESSIONLOST` and left the session at once, but its setup screen didn't change. Details are in that ticket's comments.
