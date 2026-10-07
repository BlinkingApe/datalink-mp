# 04: Tell Joiners when the Host is hostable

Type: grilling
Status: resolved
Blocked by:

Carried over from helper-web-ui ticket 21 onto the [post-0.1.0-polish map](../map.md). It stayed parked in "Not yet specified" until the [game-session-sync map](../../game-session-sync/map.md) settled. That map has now closed all its decisions, and its one open item, [Tell the game when a friend's connection drops](../../game-session-sync/issues/05-tell-the-game-when-a-friends-connection-drops.md), is a game-facing fix that adds no Helper-to-Helper status channel. Graduated to a live ticket 2026-10-06, after `0.1.0` shipped.

## Question

A Joiner who reaches the game's "Join Game" before the Host has pressed Host Game gets "game not found", with no explanation. How should the Joiner find out that **the Host is hostable** (the Host's game has a Game session a Join can find)?

- **Automatic:** the Host's Helper tells connected Joiners' Helpers when its Game session opens and closes, and the Joiner's page shows it.
- **Manual:** the players tell each other, the way they already pass Tickets. The page only changes its wording.
- **Wording only, or wording as well:** whatever the signal, the Joiner's step 3 currently reads "Start the game" as soon as the Helpers connect. It should not suggest that connected Helpers means go ahead in the game.

Decide which one ships, and for the automatic one, what the Joiner's page shows and when.

**Term note:** use "the Host is hostable" for this condition, not "ready". `ready` is already a value of the Helper's connection state and means something else.

## Facts from the code (2026-10-06)

- The peer protocol already has `Message::SessionAnnounce { session: SessionInfo }`, documented as "host broadcasts this" (`crates/datalink-transport/src/protocol.rs`). A receiving Helper already handles it: it records the session as discovered and raises `ReceivedMessage::SessionDiscovered` (`crates/datalink-transport/src/connection.rs`). Nothing sends it today, except a test. There is no message for "Game session closed while the Helpers stay connected"; `SessionClosed` is what the Host's Helper sends when its game's session ends.
- A Joiner's Join Game today runs `enum_sessions`, which queries every connected peer with `SessionQuery` and waits for `SessionList` (`crates/datalink-transport/src/runtime.rs`). So a Joiner's Helper can already ask the Host's Helper whether a Game session exists. It just doesn't, until the game asks.
- The Joiner's step 3 is `<h2>Start the game</h2>` with the menu hint "Multiplayer → Iroh P2P → Join Game" (`crates/datalink-mp/src/page.html`).

## Answer

**2026-10-06 (maintainer, recorded by agent):**

- **Hostable** means a Join Game from this Joiner would find the session and could join it right now: from Host Game until the game starts, the session closes, or it fills. A started game is not hostable. Added to `CONTEXT.md`.
- **The signal is automatic.** The Joiner's Helper asks the Host's Helper every 2 seconds or so with the existing session query (`SessionQuery` / `SessionList`). It asks while it is connected and its own game isn't in a Game session. No new message and no Peer protocol version bump, so it also works against a Host still on `0.1.0`. A Host push (`SessionAnnounce`) was turned down: "no longer hostable" would need a new message, and with it a version bump that splits `0.1.0` players from the next release.
- **The Host's page doesn't change.**
- **The Joiner's page also changes its tab title** (e.g. "● Friend is hosting"), so a Joiner sitting in the game sees it in the taskbar. No browser notification. SMAC's Join Game can't be told anything, since it runs one search and then shows its "game not found".
- **Still open, as separate tickets:** whether SMAC marks a started game closed to new players ([Does SMAC mark a started game closed to new players?](05-does-smac-mark-a-started-game-closed.md)), and, from that, what the Joiner's step 3 shows ([What the Joiner's step 3 shows](06-what-the-joiners-step-3-shows.md)).
