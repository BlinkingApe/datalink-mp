# 14: Quit and the dead-Helper page

**What to build:** A player closes the Helper from the page with a Quit button, on every OS. Quit asks for confirmation, their friends' Helpers notice promptly, and the page then says the Helper has quit and how to start it again. If the player closes the Helper's window instead, the page notices and says the Helper is no longer running.

Scope, from the spec's "Session model", "HTTP API", "Threading rules" and "The page" sections:

- **`POST /api/quit`** answers the request, shuts the Transport down gracefully, and exits the process with status 0.
- **Session controller** gains "shut down". The library handle's shutdown uses it, so tests can observe Quit by waiting for the handle to finish without the test process exiting.
- **Threading rules.** The blocking shutdown runs inside `spawn_blocking`, and the last reference to the Transport is never dropped on an async worker thread.
- **The page.**
  - The header shows Quit in every state.
  - Quit asks for confirmation, then replaces the page with "datalink-mp has quit. You can close this tab. Double-click datalink-mp to start it again."
  - After three failed polls in a row the page shows "The Helper isn't running any more."
- The startup line saying how to quit (ticket 05) should mention the Quit button.

Tests use the seam 1 harness and a friend Transport on loopback.

**Blocked by:** 03 (Transport: `shutdown()` and `connected_peers()`), 05 (UI mode tracer bullet)

**Status:** resolved

- [ ] `POST /api/quit` gets a successful response before the Helper goes away
- [ ] After Quit the Helper's handle finishes, and the HTTP and IPC ports are released
- [ ] A connected friend sees the connection close promptly after Quit
- [ ] The built binary exits with status 0 after Quit
- [ ] Quit issued from the async handler does not abort the process
- [ ] `POST /api/quit` without a token gets 403, and with a foreign `Origin` is rejected
- [ ] The page asks for confirmation before sending Quit, then shows the "has quit" text and stops polling
- [ ] After three failed polls in a row, without a Quit, the page shows "The Helper isn't running any more."

## Comments

Implemented. Notes for the tickets that follow:

- **How Quit ends the Helper.** `POST /api/quit` answers 204 and runs the controller's `shutdown()` inside `spawn_blocking`. `shutdown()` closes the Transport and wakes whoever is in `Helper::wait`, which then stops the HTTP server and the IPC server and returns. The binary's `main` returns after `wait`, which is the exit with status 0. The library never calls `process::exit`.
- **The IPC server can now be stopped**, which the parent spec's "for the life of the process" did not foresee. The ticket's "the HTTP and IPC ports are released" needs it. Stopping ends the DLL connection being served, as a process exit would. `host` and `join` behave as before: they have no Quit, so their `wait` still blocks for good.
- **The reply to Quit.** The HTTP server now finishes the requests it has taken up before its runtime ends (up to 1 s). Without that, the binary could exit before the reply was written. The tests showed this once the reply was delayed by 50 ms.
- **Ticket 13 (Stop):** after `shutdown()` the controller's Transport is closed for good. Stop should refuse, or do nothing, once the controller has shut down. `IpcServer::stop` and `Helper::stop_servers` are about the servers, not the glossary's Stop.
- **An IPC server that dies on a panic** still ends the Helper with that panic, as before (unwind builds only; release builds abort). It now does so by shutting the controller down.
- **Page wording.** The two texts are the spec's, word for word. The confirmation reads "Quit datalink-mp? Anyone connected through you will be disconnected.", from the prototype.
- **Two cases the spec does not describe**, on pressing Quit: if the request gets no answer at all, the page shows "The Helper isn't running any more." at once; if it gets 403, the page shows the earlier-run text.
- **A failed poll** is one that gets no status: no answer, or any answer other than 200 and 403. The count goes back to zero on a good poll.
- **The glossary has no entry for Quit.** It is used as a term in the ADR, the spec and now the code. Worth adding through `/domain-modeling`.
- The page was checked in headless Firefox: declining and accepting the confirmation, the "has quit" text staying after the Helper is gone, a killed Helper giving the dead-Helper text after three polls and not after one, a wrong token, and a 340 px wide window. Ticket 17's manual check on Windows is still owed.
