# Map: The game carries on when the host leaves mid-game

## Destination

When the host's game leaves a game in progress, whether it quits normally, is killed, or its Helper stops, the Joiners' games carry on with a new host, as they would under real DirectPlay. Today every Joiner's game exits to the desktop.

## Notes

- Domain: `GLOSSARY.md` (Helper, DLL, **Game session**, Joiner). Related: [game-session-sync](../game-session-sync/map.md). Its ticket 03 decided "only the host's close ends a session". Ticket 02 set 10a (host presses Stop mid-game, the Joiner's game closes) aside as past the Stop warning. This effort reverses the first decision for games in progress, and fixes 10a along the way.
- Evidence: logs from a Windows host and two Linux Joiners on a LAN, all running `wtp.exe` (which loads `terranx.exe`), 2026-10-20. They're local only, in `captures/2026-10-20-wtp-lan/`. The Linux clocks run about 1.92 s ahead of the Windows host.

### What the logs show (host clock)

- 07:17:55.529: the host's game broadcasts one 44-byte JACKAL type-4 message, and both Joiners ack it.
- 07:17:55.562: the host's game calls `Close`, and its Helper sends `SessionClosed`.
- About 07:17:55.595: each Joiner's Helper hands its game `DPSYS_SESSIONLOST`. Within 1.5 ms the game calls `Close`, then `Release`, then exits through a normal shutdown (`DLL_PROCESS_DETACH`, "cleanup complete"). It wasn't a crash; the game chose to quit.

### Why that's wrong

- **The game asked for host migration.** The session's `dwFlags` is `0x04` (`DPSESSION_MIGRATEHOST`), and `0x24` once the game starts (`0x20`, `DPSESSION_JOINDISALLOWED`). With migration on, real DirectPlay gives the other players `DESTROYPLAYERORGROUP` for the leaving host's player, never `SESSIONLOST`.
- **JACKAL migrates the host itself** (`terranx.exe` v2.0; its system-message handler is at `0x632db0`):
  - `SESSIONLOST` (`0x31`): calls the network object's virtual `[+0x10]`, then `0x62e010`, which closes and releases DirectPlay. The game then exits.
  - `DESTROYPLAYERORGROUP` (`0x05`, player type): calls `0x632350(dpid, 1)`. In that function's "player gone" path (`0x6325f6`), if the departing DPID is the host's (`[this+0x764]`), it stores the old host at `0x9be5fc`, makes the first remaining entry of its roster (`[this+0x154]`, stride `0x58`) the host, and calls the virtual `[+0x14]`.
  - `DPSYS_HOST` (`0x101`): not handled.
- **Each game's roster follows the order it heard `CREATEPLAYERORGROUP` in**, and the Helper announces players in `HashMap` order. In this run Rocky's game heard host, Rocky, laptop, while the laptop's heard Rocky, host, laptop. Both would still have picked Rocky, but only by luck. The roster-order reading is an inference from the disassembly; the real-game ticket confirms it.

### Decided while charting

- **Mid-game only.** The Helper treats the session as in progress once the session desc carries `JOINDISALLOWED`. In Multiplayer Setup the host leaving keeps today's behaviour: Joiners get `SESSIONLOST`, which the Setup screen quietly ignores (game-session-sync ticket 05).
- **New host = the lowest remaining DPID**, which is join order. The Helpers and every game pick the same player once ticket 01 makes every roster the same order.
- **A host that vanishes takes the same path** (ticket 03 here). That covers Stop, a killed game, or a connection lost for good. It's the same mechanism, so it stays in this effort rather than in game-session-sync ticket 05, which is about a *Joiner's* connection dropping.

## Tickets

1. [Announce players to each game in join order](issues/01-announce-players-in-join-order.md)
2. [The host's Close hands its Joiners a player leaving, not a lost session](issues/02-host-close-migrates-the-host.md)
3. [The host's Helper vanishing mid-game migrates too](issues/03-host-vanishing-migrates-too.md)
4. [Confirm in a real game](issues/04-confirm-in-a-real-game.md)

## Decisions so far

<!-- one line per closed ticket -->
- Ticket 01: every Joiner's game is announced its players in ascending DPID, itself included, so rosters agree ([ticket](issues/01-announce-players-in-join-order.md)).
