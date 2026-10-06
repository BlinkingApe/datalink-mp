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
