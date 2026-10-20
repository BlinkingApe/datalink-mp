# 01: Announce players to each game in join order

Type: task
Status: ready-for-agent
Blocked by: None (can start immediately)

## What to build

Every game in a session hears about its players in the same order: ascending DPID, which is join order, with its own player in its place. Today a Joiner's Helper announces the players already in the session in whatever order its player map gives. In the 2026-10-20 logs the laptop's game heard Rocky before the host.

This matters because JACKAL picks a new host as the first remaining entry of its roster, and the roster follows announcement order. The order has to be the same on every machine before ticket 02 can rely on every game agreeing on the new host. See the [map](../map.md), "Why that's wrong".

## Acceptance criteria

- [ ] A Joiner joining a session that already has several players hands its game `CREATEPLAYERORGROUP` in ascending DPID order, including its own player.
- [ ] Players who join later are still announced as they arrive (they have the highest DPID, so the order holds).
- [ ] A Transport-level test with at least three Joiners shows every Joiner's game got the same order, and fails against the current order.
- [ ] No duplicate `CREATEPLAYERORGROUP` (the inbox's existing duplicate guard still holds).
