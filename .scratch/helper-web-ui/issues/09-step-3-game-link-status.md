# 09: Step 3: game link status

**What to build:** Step 3 of the page tells the player which menu entries to pick in the game and shows whether the game has connected to the Helper. When the game's DLL does not match the Helper, or another program holds the IPC port, a banner says what happened and what to do, and the page keeps working.

Scope, from the spec's "Shared status and banners" and "The page" sections:

- **Game connected.** A status flag that becomes true when a DLL handshake succeeds and false when that connection ends, including on error paths.
- **`ipc_version_mismatch` banner**, a condition banner: set when a DLL handshake carries a different IPC version, or when the first message on a DLL connection fails to decode; cleared when a later handshake succeeds. Text: "The game's `dplayx.dll` doesn't match this Helper. Extract the whole archive into your Game folder again, then restart the game."
- **`ipc_port_in_use` banner**, a condition banner: set when the IPC bind fails with address-in-use in UI mode; never cleared (a restart is needed). Text: "Another program is using port N, so the game can't reach the Helper. Close the other program and start datalink-mp again." In UI mode the Helper does not exit on this failure: the HTTP server and the page keep working. In `host` and `join` a failed IPC bind stays fatal.
- **The page.** Step 3 is done when the game is connected. It shows "In the game: Multiplayer → Iroh P2P → Host Game (you) or Join Game (your friends)" and a pill reading "Game connected" or "Game not running". On Linux and macOS, when the game is not connected, it shows the hint that the Wine override is the usual cause. Narrowing the menu hint to "Join Game" belongs to ticket 11.

This ticket introduces the banner list rendering under the header if no earlier ticket has.

Tests use the seam 1 harness and the fake DLL from ticket 04, and assert on status fields and IPC responses.

**Blocked by:** 05 (UI mode tracer bullet)

**Status:** resolved

- [ ] Status reports game connected as false on a fresh Helper
- [ ] Game connected turns true after a fake DLL handshake and false when the fake DLL disconnects
- [ ] Game connected turns false when the DLL connection ends on an error path, not only on a clean close
- [ ] A handshake with the wrong IPC version sets `ipc_version_mismatch`
- [ ] A garbage first message on a DLL connection sets `ipc_version_mismatch`
- [ ] A later good handshake clears `ipc_version_mismatch`
- [ ] Starting the Helper in UI mode while the test holds the IPC port gives a working UI whose status carries `ipc_port_in_use` and the port number
- [ ] `host` and `join` still exit with an error when the IPC bind fails
- [ ] The page ticks step 3 and shows "Game connected" when the flag is true, and "Game not running" otherwise
- [ ] The page shows both banners' texts, with the port number in the `ipc_port_in_use` text

## Comments

Implemented. Notes for the tickets that follow:

- **Banner codes** are the `Banner` enum in the controller; status still serialises them as a list of codes. The page has one hidden element per code (`data-banner="…"`) under the header, and `render` shows the ones status lists. A new banner is an enum variant plus one element in the page.
- **The port number** in the `ipc_port_in_use` text is status's `ipc_port`. When the bind fails, that field and `Helper::ipc_port()` carry the port that was asked for.
- **Only address-in-use is non-fatal** in UI mode, as the spec says. Any other bind error (for example the permission error Windows gives for a port in a reserved range) still ends startup with the old error, and the window closes. Not covered by any ticket.
- **Ticket 14 (Quit):** with no IPC server, `Helper::wait` parks the thread for good. Quit needs a way to end that wait too.
- **Ticket 15 (single instance):** the address-in-use arm in `start` (lib.rs) is where the probe for a running Helper goes, before falling through to the banner.
- **A first message that is not even framed as ours** (a length prefix beyond the 16 MB limit) counts as "fails to decode" and sets `ipc_version_mismatch`. A connection that closes before a whole first message arrives sets nothing.
- **The Wine hint in step 3** is hidden while either banner of this ticket is shown: both already say why the game is not connected, and with `ipc_version_mismatch` the DLL did load, so the override is not the cause. This is narrower than the ticket's "when the game is not connected".
- **Step 3 is highlighted as current** whenever it is not done, like step 2. Gating both on step 1 belongs with ticket 08.
- The page was checked in headless Firefox in each state (fresh, port in use, mismatch, connected, disconnected again). Ticket 17's manual check on Windows is still owed.
