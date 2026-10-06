# 05: Does SMAC mark a started game closed to new players?

Type: task
Status: needs-triage
Blocked by:

HITL: the maintainer runs the game, and the agent does the code change and reads the log.

## Question

[Tell Joiners when the Host is hostable](04-signal-joiners-when-host-is-hostable.md) defines a started game as not hostable. The Joiner's Helper can only tell that from the Game session's settings. The Host's Helper answers a session query with its Game session whatever its flags are (`Message::SessionQuery` in `crates/iroh-transport/src/connection.rs`). So: when the Host's game starts from Multiplayer Setup, does SMAC call `SetSessionDesc` with `DPSESSION_NEWPLAYERSDISABLED` or `DPSESSION_JOINDISABLED` set (`crates/dp-types/src/flags.rs`)? And does it fill or close the session some other way?

## Needed

- [ ] Agent: make `DirectPlay_SetSessionDesc` (`crates/dplayx/src/directplay.rs`) log the session's flags, plus its current and max players, at debug level. Today it logs only that it was called.
- [ ] Maintainer: on a build with that line, run the game as Host with `DPLAYX_LOG_FILE` set (a `Z:\…` path under Wine; see `docs/building.md`). Host Game, have a Joiner join (or play alone if SMAC allows it), then start the game. Keep the log.
- [ ] Agent: from the log, record whether SMAC calls `SetSessionDesc` when the game starts, and with which flags.

The result decides whether the Joiner's page can say "already started", or has to call any open session hostable. See [What the Joiner's step 3 shows](06-what-the-joiners-step-3-shows.md).

## Comments

2026-10-06: the traffic capture built for internet-play-speed can answer this without the DLL change. Every `SetSessionDesc` the game makes already reaches the Helper over IPC, and the capture now records it (`session_desc` lines: flags, `new_players_disabled`, `join_disabled`, player counts). A Joiner's capture records the host's `SessionDescUpdate` with the same fields. It's on branch `capture/traffic-capture`, documented in `docs/traffic-capture.md`. [Play a captured internet game](../../internet-play-speed/issues/04-play-a-captured-internet-game.md) starts a game from Multiplayer Setup with the capture on, so that session answers the question for free. If it does, the first and second Needed items can be dropped.

2026-10-06: answered by the capture from [Play a captured internet game](../../internet-play-speed/issues/04-play-a-captured-internet-game.md). Linux hosted, with stock `v0.1.0` Windows joining. **Yes: when the game starts from Multiplayer Setup, SMAC calls `SetSessionDesc` with `DPSESSION_JOINDISABLED` set, not `DPSESSION_NEWPLAYERSDISABLED`.**

| Time | When | `flags` | `join_disabled` | `new_players_disabled` | players |
|---|---|---|---|---|---|
| 17:31:56.891 | Host Game, as the session is created | `0x24` | true | false | 1 / 7 |
| 17:31:57.235 | 344 ms later | `0x04` | false | false | 1 / 7 |
| 17:34:05.436 | Game start, 160 ms after its first data message | `0x24` | true | false | 2 / 7 |

`0x04` is `DPSESSION_MIGRATEHOST` and `0x20` is `DPSESSION_JOINDISABLED`. Nothing else changed the session's settings for the rest of the game. That includes the Joiner dropping: there was no further `SetSessionDesc`. Player counts aren't used to close the game either: `current_players` stays 2 of 7.

One catch for the Joiner's page: SMAC also sets `JOINDISABLED` for about a third of a second while it creates the session. A Joiner's Helper sees that too. When the capturing Linux machine later joined a Windows host, it received both updates (`0x24`, then `0x04`) back to back as it joined. So the last `SessionDescUpdate` is the one to trust, and `JOINDISABLED` held after it means the game has started.

The DLL change and the `DPLAYX_LOG_FILE` run in Needed aren't required for this answer.
