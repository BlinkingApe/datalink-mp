# 03: The host's Helper vanishing mid-game migrates too

Type: task
Status: ready-for-agent
Blocked by: 02

## What to build

When the host goes away mid-game without its game closing the session, the Joiners' games carry on with a new host, exactly as ticket 02 does for a normal close. That covers the host pressing Stop, the host's game or Helper being killed, and the host's connection lost for good. Today a lost host connection gives each Joiner's game `SESSIONLOST`, and the game exits. That was game-session-sync 10a, set aside then because only Stop triggered it.

A brief network blip must not migrate the host. Decide when a lost host connection counts as gone. The likely rule is once the Helper's re-dials give up, but a shorter bound may be needed if the game's own "not responding" timeout fires first. Record the rule chosen and why on this ticket.

As in ticket 02, this applies only once the game is in progress. In Multiplayer Setup a vanished host keeps today's behaviour.

## Acceptance criteria

- [ ] Game in progress, the host's Helper shuts down or is dropped without a `SessionClosed`: each Joiner's game gets `DESTROYPLAYERORGROUP` for the host's player and no `SESSIONLOST`, and the Helpers agree on the new host as in ticket 02.
- [ ] A host connection that drops and comes back within the chosen bound migrates nothing, and the game sees no player leave.
- [ ] The Stop warning on the page still says what Stop mid-game can cost (game-session-sync ticket 04). Update its wording if it now overstates the harm.
- [ ] Add a comment to game-session-sync ticket 05 saying this ticket covers a vanished host mid-game, and that a vanished Joiner stays with ticket 05.
