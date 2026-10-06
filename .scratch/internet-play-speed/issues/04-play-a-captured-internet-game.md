# 04: Play a captured internet game

Type: task
Status: needs-triage
Blocked by: 03

HITL: the maintainer plays, and the agent prepares the build and checks the files.

## Question

We need one real internet game's traffic. The game is played with the capture on, on the reference setup in the map's Notes.

## Needed

- [ ] Agent: a local Linux build with [the capture](03-dev-only-traffic-capture.md), and the exact command to start it with the capture on, writing to `.scratch/internet-play-speed/captures/` (gitignored).
- [ ] Maintainer: the Linux machine runs that build, and the Windows machine runs stock `v0.1.0`. One machine is on the mobile-data hotspot. Play at least five turns, past the opening. Note roughly how long each Turn sync felt, and which turns felt slow.
- [ ] Maintainer, if it can be scheduled: the same with a friend abroad, the Linux side capturing.
- [ ] Agent: check the capture files are complete and readable, and record here what was played: the setup, the number of turns, the capture files, and the connection type the capture saw.

## Riding along: two questions from other efforts

The capture also records the session settings the game sets, the system messages it's handed, failed sends and lost connections (`docs/traffic-capture.md`, branch `capture/traffic-capture`). So this session can answer two open questions for the cost of a few extra minutes. Both are optional, and neither changes the speed game.

- **[Does SMAC mark a started game closed to new players?](../../post-0.1.0-polish/issues/05-does-smac-mark-a-started-game-closed.md)** costs nothing extra: starting the speed game from Multiplayer Setup is the experiment. The capture's `session_desc` lines (if Linux hosts) or `ctl` `SessionDescUpdate` lines (if Linux joins) show whether SMAC sets `DPSESSION_NEWPLAYERSDISABLED` or `DPSESSION_JOINDISABLED` when the game starts.
- **[Tell the game when a friend's connection drops](../../game-session-sync/issues/05-tell-the-game-when-a-friends-connection-drops.md)** needs its two reproductions, each after the speed game is over. The capturing Linux machine must be the side whose friend drops:
  - [ ] Maintainer, **Joiner's Helper goes away mid-game:** for the speed game, Linux hosts and Windows joins. After the speed turns, quit the Windows Helper. On Linux, wait about 30 s without acting and note what the game shows. Then make a move, note the messages, and choose Drop.
  - [ ] Maintainer, **killed host:** swap roles. Windows hosts, and Linux joins and waits in Multiplayer Setup. Kill the Windows game (Task Manager → End task). On Linux, wait about 30 s, note what the setup screen does, and then quit.
  - [ ] Agent: from the capture, record on that ticket what each game was handed (`sys` lines), and when, relative to the `lost` and `reconnect` lines and any `send_failed` lines.

  So the recommended setup for the speed game is **Linux hosts**. If a capture alone leaves a lead unconfirmed, that ticket still asks for both sides' `dplayx.log`, since the capture sees only the Linux side.

