# 02: The host's Close hands its Joiners a player leaving, not a lost session

Type: task
Status: ready-for-agent
Blocked by: None (can start immediately)

## What to build

When the host's game closes a Game session that's in progress, each Joiner's game is told the host's player left (`DESTROYPLAYERORGROUP`), not that the session was lost. JACKAL then picks a new host by itself, and the game carries on. Each Joiner's Helper moves its own host role to the same player: the lowest remaining DPID. From then on, that player's Helper is treated as the host everywhere the Helper asks who the host is (whose `SessionClosed` counts, who may say a player left).

"In progress" means the session desc carries `DPSESSION_JOINDISALLOWED`, which SMAC sets when the game starts. In Multiplayer Setup, nothing changes: the host's close still gives Joiners `SESSIONLOST`.

This reverses game-session-sync ticket 03's "only the host's close ends a session" for games in progress. Record that on that ticket. The evidence and the JACKAL behaviour it relies on are in the [map](../map.md).

## Acceptance criteria

- [ ] Host plus two or three Joiners, game in progress: the host closes, and each Joiner's game gets `DESTROYPLAYERORGROUP` for the host's player and no `SESSIONLOST`.
- [ ] Every remaining Helper agrees on the new host (the lowest remaining DPID), and only that Helper reports itself as host.
- [ ] The new host closing in turn migrates again, down to the last player. A lone remaining player whose host leaves gets the same handling, on the new host's authority.
- [ ] A `SessionClosed` or `PlayerLeft` from the old host's Helper, arriving after the migration, is ignored.
- [ ] Same setup in Multiplayer Setup (no `JOINDISALLOWED`): Joiners still get `SESSIONLOST`, as today.
- [ ] The throwaway probe test from the 2026-10-20 diagnosis (`debug_host_close_probe.rs`, uncommitted) is deleted, and its useful cases live on as real tests.
