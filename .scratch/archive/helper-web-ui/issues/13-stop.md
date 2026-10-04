# 13: Stop

**What to build:** A player in a session presses Stop to end the Helper's current connections without quitting it. The Helper comes back with a new Ticket, the page tells the player to share it again, and a game that is still open stays connected to the Helper, so they can host or join again without restarting the game.

Scope, from the spec's "Session model", "Threading rules", "HTTP API" and "The page" sections (ADR-0001 acceptance criterion 3):

- **`POST /api/stop`** returns once the new Ticket is in place.
- **Stop is in-process.** The controller creates the new Transport first, swaps it in, and then shuts the old one down off the async threads. If creating the new Transport fails, nothing changes and Stop reports the error. IPC requests therefore always find a Transport.
- **Ticket sequence number.** Stop increases it; it starts at 1.
- **Banners.** Stop clears the event banners (`invalid_ticket`, `cant_reach_host`, `peer_version_mismatch`). Condition banners are unaffected.
- **Threading rules.** The blocking Transport calls run inside `spawn_blocking`. The last reference to the old Transport is never dropped on an async worker thread. Release builds abort on panic, so breaking either rule kills the Helper and the game's IPC connection with it.
- **Stop with the game open.** The game's IPC connection survives, but a game that is in a session will not be told the session ended. The page asks for confirmation when the game is connected ("Return to the game's main menu first…"). Delivering a session-lost message to the game is out of scope.
- **The page.** The header shows Stop, hidden in `ready`. After Stop, step 2 shows "Your Ticket changed. Share it again." until the player copies the new Ticket; the page detects the change from the Ticket sequence number.
- The handshake reply the DLL received earlier carries the old Ticket. The DLL only logs it, so this is harmless and needs no change.

Tests use the seam 1 harness, a friend Transport on loopback and the fake DLL.

**Blocked by:** 03 (Transport: `shutdown()` and `connected_peers()`), 09 (Step 3: game link status), 11 (Step 2: join a friend's Ticket from the page)

**Status:** resolved

- [ ] After Stop the Ticket has changed, the sequence number has gone up by one, and the state is `ready`
- [ ] A connected friend sees the connection close promptly
- [ ] Event banners present before Stop are gone after it
- [ ] A fake DLL connection opened before Stop still gets answers afterwards, and game connected stays true
- [ ] A friend can dial the new Ticket after Stop, and a UI join after Stop works
- [ ] If the new Transport cannot be created, Stop reports an error and the Ticket, sequence number and connections are unchanged
- [ ] Stop issued from the async handler does not abort the process (the nested-runtime and runtime-drop hazards)
- [ ] `POST /api/stop` without a token gets 403, and with a foreign `Origin` is rejected
- [ ] The Stop button is hidden in `ready` and shown in `joining`, `joined` and `hosting`
- [ ] With the game connected, the page asks for confirmation before sending Stop
- [ ] After Stop the page shows "Your Ticket changed. Share it again." until the new Ticket is copied
