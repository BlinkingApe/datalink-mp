# 02: The host's Close hands its Joiners a player leaving, not a lost session

Type: task
Status: resolved
Blocked by: None (can start immediately)

## What to build

When the host's game closes a Game session that's in progress, each Joiner's game is told the host's player left (`DESTROYPLAYERORGROUP`), not that the session was lost. JACKAL then picks a new host by itself, and the game carries on. Each Joiner's Helper moves its own host role to the same player: the lowest remaining DPID. From then on, that player's Helper is treated as the host everywhere the Helper asks who the host is (whose `SessionClosed` counts, who may say a player left).

"In progress" means the session desc carries `DPSESSION_JOINDISABLED`, which SMAC sets when the game starts. In Multiplayer Setup, nothing changes: the host's close still gives Joiners `SESSIONLOST`.

This reverses game-session-sync ticket 03's "only the host's close ends a session" for games in progress. Record that on that ticket. The evidence and the JACKAL behaviour it relies on are in the [map](../map.md).

## Acceptance criteria

- [ ] Host plus two or three Joiners, game in progress: the host closes, and each Joiner's game gets `DESTROYPLAYERORGROUP` for the host's player and no `SESSIONLOST`.
- [ ] Every remaining Helper agrees on the new host (the lowest remaining DPID), and only that Helper reports itself as host.
- [ ] The new host closing in turn migrates again, down to the last player. A lone remaining player whose host leaves gets the same handling, on the new host's authority.
- [ ] A `SessionClosed` or `PlayerLeft` from the old host's Helper, arriving after the migration, is ignored.
- [ ] Same setup in Multiplayer Setup (no `JOINDISALLOWED`): Joiners still get `SESSIONLOST`, as today.
- [ ] The throwaway probe test from the 2026-10-20 diagnosis (`debug_host_close_probe.rs`, uncommitted) is deleted, and its useful cases live on as real tests.

## Answer

Built. When a Joiner's Helper gets the host's `SessionClosed` and the session desc carries `DPSESSION_JOINDISABLED` (this ticket's `JOINDISALLOWED`; `dp_types::DPSESSION_JOINDISABLED`, `0x20`), it migrates the host role instead of losing the session. Otherwise (Multiplayer Setup) it queues `SESSIONLOST` as before. The host's wire message is unchanged.

- **Where host-role state lives:** `SessionManager` in `crates/datalink-transport/src/session.rs`. `Session::is_host` (read by `is_host()`) says whether this Helper hosts; `host_node_id` (read by `get_host_node_id()` and `is_node_host(node)`) says who does when it doesn't. `SessionManager::host_closed(sender) -> HostClosed` is the one place that moves the role: it returns `NotFromHost`, `SessionLost`, or `Migrated { departed, we_are_host }`. Everything that asks who the host is (`SessionClosed`, `PlayerLeft`'s "from the host" check, `RosterFacts::was_host`, `SendFacts`) already reads these, so they follow the new host.
- **Migration:** the sender's players leave the roster (and `friends.player_left` unroutes them); each is queued as `PlayerLeft` (`DESTROYPLAYERORGROUP`). The new host is the lowest remaining DPID in the roster, not the order of announcements, so it doesn't depend on ticket 01. If it is this Helper's player, `is_host` is set and `host_node_id` cleared; otherwise `host_node_id` becomes that player's node.
- **Stale messages:** after migration the old host's node is no longer `is_node_host`, and its players are gone from the roster, so its later `SessionClosed` or `PlayerLeft` is ignored.
- **Chained and lone:** the new host's Close sends `SessionClosed` like any host and migrates again; a lone remaining player becomes host.
- **Tests:** `mesh_networking.rs` (`test_host_closing_mid_game_...`, `test_new_host_closing_migrates_again_...`, `test_host_closing_in_multiplayer_setup_still_tells_joiners_the_session_is_lost`) and `session.rs` unit tests (`test_host_closed_*`). `debug_host_close_probe.rs` is deleted; its mid-game three-player case lives on as the first test.
- **For ticket 03:** a vanishing host's Helper should call `SessionManager::host_closed(&host_node)` where `FriendRegistry::link_ended` (`friend_registry.rs`) now queues `SessionLost` for `was_host`, and queue the same `PlayerLeft` events for `Migrated`. That needs the registry to reach the session manager (today it only gets `RosterFacts`).
