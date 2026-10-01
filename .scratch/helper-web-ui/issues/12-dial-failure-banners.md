# 12: Dial failure banners

**What to build:** A player whose join fails is told why. If the friend's Helper cannot be reached, a banner says so within about fifteen seconds and tells them to ask for the current Ticket. If the friend runs a different release, a banner says both need the same one. The same banners appear when the join was started from inside the game.

Scope, from the spec's "Shared status and banners" section (ADR-0001 acceptance criterion 6):

- **`cant_reach_host` banner**, an event banner: set when a dial times out or cannot connect, from the UI or from the DLL's join request. Text: "Couldn't reach your friend's Helper. Ask them for their current Ticket: it changes every time they start the Helper or press Stop."
- **`peer_version_mismatch` banner**, an event banner: set when a dial is rejected for the ALPN, from the UI or from the DLL's join request. Text: "Your friend has a different release of datalink-mp. You both need the same one."
- Both come from the most recent join attempt and are cleared by the next join attempt. (Stop also clears them; that is asserted in ticket 13.)
- A DLL join request that fails for another reason (for example the host has no session yet) sets no banner.
- The host sees nothing when a mismatched build tries to connect; only the joiner gets the banner.
- After a failed dial the state is `ready`.
- The library configuration gains the Transport options from ticket 02, so tests can shorten the dial timeout and build a friend with a different Peer protocol version.

Tests use the seam 1 harness, a friend Transport on loopback and the fake DLL.

**Blocked by:** 02 (Transport: versioned ALPN, distinguishable dial errors, dial timeout), 11 (Step 2: join a friend's Ticket from the page)

**Status:** resolved

- [ ] A UI join to a Ticket whose Transport has been shut down gives `cant_reach_host` within the shortened timeout, and the state returns to `ready`
- [ ] A UI join to a friend built with a different Peer protocol version gives `peer_version_mismatch`, not `cant_reach_host`
- [ ] A fake DLL's join request to an unreachable Ticket sets `cant_reach_host`
- [ ] A fake DLL's join request to a friend with a different Peer protocol version sets `peer_version_mismatch`
- [ ] A fake DLL's join request that fails because the host has no session sets no banner
- [ ] A later join attempt clears both banners
- [ ] The friend that refused a mismatched dial reports no banner of its own
- [ ] The page shows each banner's text under the header
