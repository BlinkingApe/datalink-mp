# 05: UI mode tracer bullet: the page shows a live Ticket

**What to build:** Running `datalink-mp` with no subcommand starts the web UI. The Helper prints a launch URL, the page at that URL shows the player's Ticket with a Copy button, and the page refreshes itself about once a second. This is the first path through every layer: command line, Session controller, HTTP server, status endpoint, embedded page and tests. Later tickets add one step or one action each.

Scope, from the spec's "Command line", "Modules", "HTTP API", "Session model" and "The page" sections (ADR-0001):

- **Command line.** No subcommand starts the UI. Options in this mode: `--ui-port` (also `SMAC_UI_PORT`; the flag wins), `--port` for the IPC port (also `SMAC_HELPER_PORT`, with today's precedence), and `--no-browser`. Default UI port 47700. If the UI port is taken, exit with a clear message for now; the walk over the next nine ports is ticket 06.
- **Startup output.** The Helper prints its name and Release version, the launch URL (which carries the token as `?t=`), and a line saying how to quit. The token is never logged at info level or above; the printed launch URL is the only place it appears. Logging to a file stays opt-in through `SMAC_HELPER_LOG_FILE`. The Helper writes no files in UI mode.
- **HTTP server.** axum on its own tokio runtime, owned by `main`, which stays synchronous. Bound to `127.0.0.1` only. The token is 32 bytes from the OS random source, hex-encoded.
- **Routes.** `GET /` serves the embedded page and needs no token. `GET /api/status` needs the token in the `X-Token` header; a missing or wrong token gets 403. The remaining security checks are ticket 07.
- **Status fields** in this ticket: Release version, IPC version, Peer protocol version, OS (`windows`, `linux` or `macos`), state (always `ready` here), the Ticket, the Ticket sequence number (1), the IPC port, an empty peer list and an empty banner list. Add the fields that later tickets fill (game-connected flag, self-check result) only when those tickets need them. Status requests use only non-blocking reads.
- **Session controller** gains "read a status snapshot".
- **Library configuration** gains the UI port, the token and the browser opener. The opener is passed in as a function so tests can replace it. Unless `--no-browser` is given, the opener is called once at startup with the launch URL. The binary may pass an opener that does nothing; the real per-OS openers are ticket 06. The handle exposes the bound UI port.
- **The page.** One static HTML file with inline CSS and JavaScript, embedded in the binary. No framework, no build step, no external assets. Reference for layout and wording: prototype variant C on branch `prototype/ui-flow` (commit `6399164`), with "smac-helper" replaced by "datalink-mp", the Steam Copy button dropped and the scenario panel removed. In this ticket the page has the header with the name, the four numbered steps as a skeleton, the Ticket in step 2 (read-only, selected on click, with Copy), and the footer "Release X.Y.Z (IPC vN · Peer protocol vN) · State: …". Copy uses the clipboard API and falls back to selecting the text. The page reads the token from `?t=` and polls status about once a second. It must work in a narrow window.
- The page renders what status says. The Helper computes the state and the banners; the page computes only which steps are ticked.

Tests (the spec's seam 1): start the library entry point inside the test process on free ports, with a recording stand-in for the browser opener, and drive it with an HTTP client. Assert only on HTTP responses and status fields. Return early when a Transport cannot be created.

**Blocked by:** 04 (Helper as a library: Session controller and IPC server)

**Status:** resolved

- [ ] `datalink-mp` with no subcommand starts the IPC server and the HTTP server and prints its name, Release version, launch URL and how to quit
- [ ] `--ui-port` and `SMAC_UI_PORT` choose the UI port, and the flag wins
- [ ] `--no-browser` prevents the opener being called; without it the opener is called once with the launch URL
- [ ] The HTTP server listens on `127.0.0.1` only
- [ ] A fresh Helper's status reports `ready`, a parseable Ticket, sequence number 1, the three versions, the OS, the IPC port and no peers
- [ ] `GET /api/status` without a token, or with a wrong one, gets 403
- [ ] `GET /` is served without a token and contains no `http://` or `https://` references to other hosts
- [ ] The page shows the Ticket with a working Copy button, the footer with the three versions and the state, and updates without a reload
- [ ] The token does not appear in log output at info level or above
- [ ] Smoke test of the built binary: with no subcommand and `--no-browser`, the launch URL is printed and the page is served at it
- [ ] `host` and `join --ticket` still behave as before and start no HTTP server
