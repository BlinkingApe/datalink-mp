# 10: Step 4: hosting and connected Helpers

**What to build:** A player who shared their Ticket sees on the page when a friend's Helper has connected, how many Helpers are connected, and a short ID for each. Step 4 also tells them, in words that fit their OS, to keep the Helper open while they play.

Scope, from the spec's "Session model", "HTTP API" and "The page" sections:

- **Peer list.** Status carries the list of peer short IDs (iroh's short form of the endpoint ID), read from the Transport's `connected_peers()`. No player names.
- **State.** The controller computes the state. This ticket adds `hosting`: at least one peer is connected and we did not dial. When the last peer goes away, the state returns to `ready`. The `joining` and `joined` states are ticket 11.
- **The page.**
  - Step 2 is done when the state is `hosting` (or `joined`, from ticket 11).
  - Step 4 is active when steps 1 and 3 are done. It shows the peer count with short IDs, and the keep-open line for the OS:
    - Windows: "Keep this browser tab and the console window open while you play."
    - Linux: "Keep this browser tab open while you play. Closing it doesn't stop the Helper; Quit does."
    - macOS: "Keep this tab and the Terminal window open while you play."
  - The page says "connected", not "players": a peer can be a Helper connected only for session discovery.
  - The footer's state word reads "Hosting".
- The host sees nothing when a mismatched build tries to connect; that is out of scope.

Tests use the seam 1 harness plus a friend: a second real Transport on loopback that dials the Helper's Ticket. Use a polling helper with a deadline in place of sleeps, and return early when a Transport cannot be created.

**Blocked by:** 03 (Transport: `shutdown()` and `connected_peers()`), 05 (UI mode tracer bullet)

**Status:** ready-for-agent

- [ ] A friend dialling the Helper's Ticket moves the state to `hosting`
- [ ] The friend's short ID appears in the status peer list, and the Helper appears in the friend's connected peers
- [ ] When the friend goes away, the peer list empties and the state returns to `ready`
- [ ] Two friends give two entries in the peer list
- [ ] Status requests stay non-blocking while peers connect and disconnect
- [ ] The page ticks step 2 in `hosting` and shows the peer count with short IDs in step 4
- [ ] Step 4 is active only when steps 1 and 3 are done
- [ ] The keep-open line matches the OS reported by status
