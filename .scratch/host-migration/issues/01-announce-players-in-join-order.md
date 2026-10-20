# 01: Announce players to each game in join order

Type: task
Status: resolved
Blocked by: None (can start immediately)

## What to build

Every game in a session hears about its players in the same order: ascending DPID, which is join order, with its own player in its place. Today a Joiner's Helper announces the players already in the session in whatever order its player map gives. In the 2026-10-20 logs the laptop's game heard Rocky before the host.

This matters because JACKAL picks a new host as the first remaining entry of its roster, and the roster follows announcement order. The order has to be the same on every machine before ticket 02 can rely on every game agreeing on the new host. See the [map](../map.md), "Why that's wrong".

## Acceptance criteria

- [x] A Joiner joining a session that already has several players hands its game `CREATEPLAYERORGROUP` in ascending DPID order, including its own player.
- [x] Players who join later are still announced as they arrive (they have the highest DPID, so the order holds).
- [x] A Transport-level test with at least three Joiners shows every Joiner's game got the same order, and fails against the current order.
- [x] No duplicate `CREATEPLAYERORGROUP` (the inbox's existing duplicate guard still holds).

## Answer

A Joiner's `join_session` now adds every existing player to the session, then queues `CREATEPLAYERORGROUP` for all of them plus its own player, sorted by ascending DPID, so the game hears host, earlier joiners, itself in join order. Players who join later arrive afterwards with higher DPIDs. The inbox's duplicate guard is untouched. The `current_players` count in each announcement is now the final roster count rather than a running one. Test: `test_every_joiners_game_hears_of_the_players_in_join_order` in `crates/datalink-transport/tests/mesh_networking.rs` (host plus four Joiners; it failed against the old `HashMap` order). Code: `crates/datalink-transport/src/runtime.rs`.
