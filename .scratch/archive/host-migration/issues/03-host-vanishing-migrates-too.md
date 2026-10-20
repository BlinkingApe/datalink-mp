# 03: The host's Helper vanishing mid-game migrates too

Type: task
Status: resolved
Blocked by: 02

## What to build

When the host goes away mid-game without its game closing the session, the Joiners' games carry on with a new host, exactly as ticket 02 does for a normal close. That covers the host pressing Stop, the host's game or Helper being killed, and the host's connection lost for good. Today a lost host connection gives each Joiner's game `SESSIONLOST`, and the game exits. That was game-session-sync 10a, set aside then because only Stop triggered it.

A brief network blip must not migrate the host. Decide when a lost host connection counts as gone. The likely rule is once the Helper's re-dials give up, but a shorter bound may be needed if the game's own "not responding" timeout fires first. Record the rule chosen and why on this ticket.

As in ticket 02, this applies only once the game is in progress. In Multiplayer Setup a vanished host keeps today's behaviour.

## Acceptance criteria

- [x] Game in progress, the host's Helper shuts down or is dropped without a `SessionClosed`: each Joiner's game gets `DESTROYPLAYERORGROUP` for the host's player and no `SESSIONLOST`, and the Helpers agree on the new host as in ticket 02.
- [x] A host connection that drops and comes back within the chosen bound migrates nothing, and the game sees no player leave.
- [x] The Stop warning on the page still says what Stop mid-game can cost (game-session-sync ticket 04). Update its wording if it now overstates the harm.
- [x] Add a comment to game-session-sync ticket 05 saying this ticket covers a vanished host mid-game, and that a vanished Joiner stays with ticket 05.

## Answer

Built. A host lost for good mid-game (`DPSESSION_JOINDISABLED` set) migrates exactly as in ticket 02; in Multiplayer Setup a vanished host still gives `SESSIONLOST`.

**The "gone" rule:** a lost host of a game in progress is re-dialled by the same loop as any friend, but for at most `TransportOptions::host_gone_after`, default `DEFAULT_HOST_GONE_AFTER` = **10 s** from the loss. If it hasn't reconnected by then, it is gone. The rule is not "once the five re-dials give up", because that bound is far too long: the game-session-sync ticket 05 capture shows the five re-dials taking 82 s when each dial times out, and SMAC's own "Player ... not responding" popup appears at about 30 s. 10 s is long enough for a blip (the first re-dials fire at 0.5 s and 1.5 s) and short enough that Joiners have a new host well before the popup. If the re-dials run out sooner than 10 s, that counts as gone too. A host with no roster ticket cannot be re-dialled, so it is gone at once. Ticket 04 should check in a real game that 10 s is quick enough (and not so quick it migrates on a real blip).

**How:**
- `friend_registry.rs`: `RosterFacts` gains `in_progress`. `link_ended` on a host with `in_progress` no longer queues `SESSIONLOST`; it keeps the record and answers `Redial` (like a Joiner's friend, so sends to the host's player fail meanwhile), or `LinkEnded::HostGone` when there is no ticket. `redial_gave_up` now returns whether the friend was forgotten. The registry still never reads the session manager; the caller does.
- `connection.rs`: `spawn_redial` takes `is_host` and wraps the loop in `host_gone_after` for the host. On giving up with the host forgotten (or on `HostGone`), it calls the new `apply_host_closed`, which is the code ticket 02's `SessionClosed` handler had (`SessionManager::host_closed`, then `PlayerLeft` for each departed player). Both paths share it, so a vanished host and a closing host behave identically. A reconnect inside the bound migrates nothing and the game sees nothing.
- `session.rs`: `is_in_progress()`. `runtime.rs`: `TransportOptions::host_gone_after`.
- Tests: four registry unit tests; in `tests/mesh_networking.rs`, `test_host_vanishing_mid_game_migrates_like_its_closing` (host `shutdown()`, no `SessionClosed`) and `test_host_connection_that_drops_and_comes_back_migrates_nothing`.

**Stop warning:** the old wording ("ends it for everyone connected through you, and can crash their game") overstated the harm for a host and is replaced in `page.html`, `README.md` and `packaging/README.txt`: Stop mid-game takes you out of it; if you are hosting, the others carry on with a new host after about 10 seconds; if not, their games may not notice you have gone (the vanished-Joiner case, still game-session-sync ticket 05). `tests/ui.rs` checks the new wording.

Not changed: a vanished Joiner (ticket 05).
