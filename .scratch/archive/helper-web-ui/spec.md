# Spec: the Helper's embedded web UI (datalink-mp v1)

Status: resolved

Source decisions: [ADR-0001](../../../docs/adr/0001-web-ui-frontend.md) (the web UI), [ADR-0002](../../../docs/adr/0002-releases-versioning-pipeline-trust.md) section 1 (versioned ALPN) and [ADR-0003](../../../docs/adr/0003-standalone-project.md) (the `datalink-mp` name). Decision trail: [the web UI v1 map](../web-ui-v1/map.md). Vocabulary: `CONTEXT.md`.

Where this spec and an ADR disagree, the ADR wins, except for the items listed under "Decisions this spec adds", which the ADRs left to the spec.

## Problem Statement

A player who wants to play Sid Meier's Alpha Centauri with friends through this project has to use a terminal. They run the Helper with `host` to print a Ticket, or with `join --ticket` to connect, and only then start the game. Nothing tells them whether the DLL is in the right place, whether the game has found the Helper, whether their friend is connected, or why a connection failed. Changing from hosting to joining means killing the Helper and starting it again, and a version mismatch between two players shows up only as a line in a log file.

Most players of a 1999 strategy game will not type commands, so today the project is usable only by the people who build it.

## Solution

The player extracts one archive into their Game folder and double-clicks the Helper, now called `datalink-mp`. Their browser opens a single local page with four numbered steps that tick off as things happen:

1. **Game folder**: the Helper confirms it is sitting next to the DLL and the game.
2. **Share or paste a Ticket**: the player's own Ticket is there from the start, with a Copy button, beside a box for a friend's Ticket and a Connect button.
3. **Start the game**: the page says which menu entries to pick and shows when the game has connected to the Helper.
4. **Play**: the page shows who is connected.

Stop ends the current connections and gives a new Ticket without closing the Helper or disturbing the running game's link to it. Quit closes the Helper. When something goes wrong, a banner says what happened and what to do. No commands are typed on any OS. The `host` and `join` subcommands keep working as before for power users and scripts.

## User Stories

### Starting up

1. As a player, I want to double-click the Helper and have my browser open the page, so that I never need a terminal.
2. As a player, I want the page's address printed in the Helper's window too, so that I can open it myself if the browser did not open.
3. As a player who double-clicks the Helper a second time, I want the running Helper to make way for the one I just started, unless the game or a friend is using it, in which case I want its page to open and say why. That way I never end up with two Helpers fighting over the same port, and never keep running a stale build after extracting a new one.
4. As a player whose usual UI port is taken by another program, I want the Helper to pick the next free port by itself, so that it still starts.
5. As a player, I want the Helper to work from wherever my Game folder is, without a settings file or an install step, so that setup is only "extract and double-click".
6. As a Windows player, I want the Helper to open my browser without spawning a hidden PowerShell, so that my antivirus is less likely to flag it.
7. As a Mac player, I want the Helper to run in the Terminal window that opens when I double-click it, so that I get no local-network permission prompt.
8. As a player whose Helper cannot start its networking at all, I want the window to stay open with a plain explanation, so that I can read the error before it disappears.

### Game folder check

9. As a player, I want the page to confirm that the DLL and my game executable are next to the Helper, so that I know I extracted the archive into the right place.
10. As a player who ran the Helper from the Downloads folder, I want a banner telling me this is not my Game folder and showing where the Helper is running from, so that I can fix it.
11. As a player whose antivirus removed the DLL, I want the banner to say the DLL may have been quarantined, so that I know to restore it.
12. As a player using Thinker, I want the check to accept `thinker.exe`, so that I am not told PRACX is required.
13. As a player using PRACX, I want the check to accept `terran_PRACX.exe`, so that either supported setup passes.
14. As a Linux player whose game files have different capitalisation, I want the check to ignore case, so that I don't get a false "not your Game folder".
15. As a player who restores a quarantined DLL, I want the banner to clear without restarting the Helper, so that I can carry on.

### Wine setup (Linux and macOS)

16. As a Linux player, I want the Wine override string on the page with a Copy button, so that I can paste it into my launcher without retyping it.
17. As a Faugus user, I want a Copy button for the unquoted form and a note about which field it goes in, so that I paste the right thing in the right place.
18. As a Steam or Lutris user, I want a line telling me what should work, marked as untested, so that I can try it knowing nobody has verified it.
19. As a Mac player, I want to be told that any Wine on macOS should work and how to reach the hidden Library folder, so that I can find my bottle.
20. As a Windows player, I don't want to see any Wine setup text, so that the page shows only what applies to me.
21. As a player, I want the Helper never to write files anywhere, so that I can trust it not to touch my system or my game.

### Hosting

22. As a player who is hosting, I want my Ticket to be on the page from the moment it opens, so that there is no "start hosting" step.
23. As a player who is hosting, I want a Copy button for my Ticket, so that I can paste it into a chat.
24. As a player whose browser blocks clipboard access, I want the Ticket to be selected when Copy fails, so that I can copy it by hand.
25. As a player who is hosting, I want the page to show when a friend's Helper has connected to mine, so that I know my Ticket worked.
26. As a player who is hosting, I want to see how many Helpers are connected and a short ID for each, so that I can tell whether everyone has arrived.

### Joining

27. As a player who is joining, I want to paste my friend's Ticket and press Connect, so that my Helper dials theirs.
28. As a player who pasted a Ticket with stray spaces or a line break, I want it accepted anyway, so that copy-paste accidents don't block me.
29. As a player who pasted something that is not a Ticket, I want an "invalid Ticket" banner straight away, so that I know to ask for it again.
30. As a player who pasted my own Ticket, I want to be told it is my own, so that I understand why nothing connected.
31. As a player whose friend's Ticket carries no addresses, I want a warning but still an attempt to connect, so that a Ticket that may still work is not thrown away.
32. As a player who is joining, I want to see a "Connecting…" state while the dial is in progress, so that I know the Helper is working.
33. As a player whose friend's Helper cannot be reached, I want to find out in about fifteen seconds instead of thirty, so that I am not left waiting.
34. As a player who can't reach the host, I want the banner to tell me to ask for their current Ticket, so that I know a Ticket goes stale after Stop or a restart.
35. As a player whose friend runs a different release, I want a banner saying both of us need the same release, so that we fix the real problem instead of guessing.
36. As a player who is joining, I want the step to read "Connected to your friend" once the dial succeeds, so that I know to move on to the game.
37. As a player who is joining, I want the in-game hint to say "Join Game" once I am connected, so that I pick the right menu entry.

### The game

38. As a player, I want the page to tell me which menu entries to choose in the game, so that I can find the right multiplayer option.
39. As a player, I want a pill that shows whether the game has connected to the Helper, so that I can tell whether the DLL is being loaded.
40. As a Linux or Mac player whose game never shows as connected, I want a hint that the Wine override is the usual cause, so that I know where to look.
41. As a player whose DLL does not match the running Helper, I want a banner telling me to extract the archive again, so that I can fix a half-updated Game folder.
42. As a player whose IPC port is taken by another program, I want a banner that says so while the page keeps working, so that the Helper does not just vanish.
43. As a player, I want to be told to keep the Helper's window open while I play, in words that fit my OS, so that I don't close it by accident.

### Stop and Quit

44. As a player in a session, I want a Stop button, so that I can leave without closing the Helper.
45. As a player who pressed Stop, I want to be told my Ticket has changed and must be shared again, so that my friends don't use the old one.
46. As a player who pressed Stop with the game still open, I want the game to stay connected to the Helper, so that I can host or join again without restarting the game.
47. As a player with the game open, I want Stop to warn me to return to the game's main menu first, so that I don't cut off a game in progress by mistake.
48. As a player who is not in a session, I don't want to see a Stop button, so that the page offers only what makes sense.
49. As a player, I want a Quit button on every OS, so that I can close the Helper from the page.
50. As a player, I want Quit to ask for confirmation, so that I don't disconnect my friends by accident.
51. As a player who has quit, I want the page to say the Helper has quit and how to start it again, so that I am not left with a dead page.
52. As a player who closed the Helper's window instead, I want the page to notice and say the Helper is no longer running, so that I don't keep pressing buttons that do nothing.
53. As a friend of a player who pressed Stop or Quit, I want my Helper to notice promptly, so that my peer list is accurate.

### Status and versions

54. As a player, I want the page to update by itself about once a second, so that I never have to reload it.
55. As a player, I want to see the Release version, so that I can compare it with my friend's.
56. As a player helping a friend debug, I want the IPC version and Peer protocol version in small print, so that I can quote them if asked.
57. As a player with a narrow browser window, I want the page to stay usable, so that I can keep it beside the game.
58. As a player without an internet connection to third-party sites, I want the page to load with no external assets, so that it always works.
59. As a player with an old tab left over from an earlier run, I want the page to say it belongs to an earlier run, so that I use the new tab.

### Security

60. As a player, I want the page reachable only from my own computer, so that nobody on my network can control my Helper.
61. As a player, I want every action to need a secret that only my browser tab has, so that another program or web page cannot press my buttons.
62. As a player, I want a malicious website to be unable to reach my Helper through DNS tricks, so that browsing the web while playing is safe.
63. As a player, I want the secret kept out of my browser's address bar and history after the page loads, so that it is not shared by accident in a screenshot.

### Power users, scripts and maintainers

64. As a power user, I want `datalink-mp host` to print a Ticket on standard output and serve the game as before, so that my scripts keep working.
65. As a power user, I want `datalink-mp join --ticket` to behave as before, so that I can join without a browser.
66. As a power user, I want to choose the UI port with a flag or an environment variable, so that I can avoid a clash.
67. As a power user running a headless machine, I want a flag that stops the Helper opening a browser, so that I can open the printed address from elsewhere on the same machine.
68. As a tester running two games on one machine, I want two Helpers with different IPC ports to run side by side, each with its own page, so that I can test alone.
69. As a player, I want a build from another project with an incompatible wire protocol to be refused at connect, so that we never half-connect and corrupt a game.
70. As the maintainer, I want the UI and the CLI to drive the same session code, so that a fix in one is a fix in both.
71. As the maintainer, I want every banner's condition covered by an automated test, so that a refactor cannot silently break the error reporting.
72. As the maintainer, I want the README to claim only what has been tested, so that players are not misled about Steam, Lutris or macOS.

## Implementation Decisions

### Naming (ADR-0003)

- The Helper's package and binary are renamed to `datalink-mp`. The other crates keep their names.
- The environment variables keep their names (`SMAC_HELPER_PORT`, `SMAC_HELPER_LOG_FILE`, plus the new `SMAC_UI_PORT`), because the DLL reads `SMAC_HELPER_PORT` and existing setups use them. The Ticket keeps its `smac` prefix: the Ticket format is unchanged.
- The workspace `repository` field points at `BlinkingApe/datalink-mp`.
- User-visible strings that say "smac-helper" (the Helper's `--help`, log lines, and the DLL's "is the Helper running?" log lines) change to "datalink-mp". These are log text only and do not change the IPC version.

### Command line

- No subcommand: start the UI. Options in this mode: `--ui-port` (also `SMAC_UI_PORT`; the flag wins), `--port` for the IPC port (also `SMAC_HELPER_PORT`, with today's precedence), and `--no-browser`.
- `host` and `join --ticket` keep their current behaviour exactly: the Ticket is the first line on standard output, the IPC port options are unchanged, a failed dial in `join` exits with an error, and a failed IPC bind is fatal. They start no HTTP server.
- In UI mode, the Helper prints its name and Release version, the launch URL, and a line saying how to quit. Logging to a file stays opt-in through `SMAC_HELPER_LOG_FILE`. The token is never logged at info level or above; printing the launch URL once at startup is the only place it appears. Once a second start has rolled the token over (see "Single instance" below), that printed URL is an old one, and the tab the second start opened is the way in.
- If the first Transport cannot be created in UI mode, the Helper prints the error in plain words and waits for Enter before exiting, so a double-clicked window does not vanish.

### Modules

The Helper becomes a library with a thin binary on top. Four modules:

- **Session controller.** Owns the current Transport and the shared status. Its interface is small: get the current Transport, read a status snapshot, join a Ticket, Stop, and shut down. Both the UI and the CLI subcommands use it; `host` and `join` become thin wrappers with no networking logic of their own.
- **IPC server.** The existing request handling moved behind the controller. The listener is bound once at startup and runs on a plain thread for the life of the process. It asks the controller for the current Transport on every request, not once per connection.
- **HTTP server.** axum on its own tokio runtime, owned by `main`, which stays synchronous. It holds the token, the security checks, the embedded page and the JSON API.
- **Platform.** The Game folder self-check and the browser opener. The browser opener is passed in as a function, so tests can replace it.

The library has one entry point that takes a configuration (Game folder path, IPC port, UI port, token, browser opener, Transport options) and returns a handle exposing the bound ports and a way to shut down. The binary fills that configuration from the command line and `current_exe()`; tests fill it with a temporary directory and free ports.

### Transport changes

- **`shutdown()`**: close every peer connection and the endpoint gracefully, bounded at 3 seconds.
- **`connected_peers()`**: the endpoint IDs of the connected Helpers. A plain lock read, safe to call from async code.
- **Versioned ALPN**: the ALPN is `datalink/<Peer protocol version>`, built from the one Peer protocol version constant, which is reset to 1. The dead, unused protocol version constant is removed. The stream preamble check stays as a second line of defence.
- **Distinguishable dial errors**: a dial that fails because the other side rejected the ALPN returns a Peer-protocol-mismatch error; a dial that times out or cannot connect returns a can't-reach error. Today both are flattened into one string.
- **Dial timeout**: 15 seconds, applied inside the Transport's dial so that the UI join, the CLI `join` and the DLL-driven join all get it.
- **Options for construction**: a way to build a Transport with a non-default Peer protocol version and dial timeout. Production code uses the defaults; tests use it to create a mismatched peer and a short timeout.

### Threading rules (from the transport-restart research)

- Every blocking Transport call made from HTTP code (join, Stop, shutdown) runs inside `spawn_blocking`.
- The last reference to a Transport is never dropped on an async worker thread. Release builds abort on panic, so breaking either rule kills the Helper and the game's IPC connection with it.
- Status requests use only non-blocking reads.

### Session model

- The Helper holds a live Transport, and therefore a Ticket, from startup. There is no "start hosting" action.
- **State** is one of four values, computed by the controller:

  | State | Meaning | Word on the page |
  |---|---|---|
  | `ready` | no peers, no dial in progress | Ready |
  | `joining` | a dial started from the UI is in progress | Connecting… |
  | `joined` | our dial succeeded and at least one peer is connected | Joined |
  | `hosting` | at least one peer is connected and we did not dial | Hosting |

  When the last peer goes away, the state returns to `ready`.
- **Stop** is in-process. The controller creates the new Transport first, swaps it in, and then shuts the old one down off the async threads. If creating the new Transport fails, nothing changes and Stop reports the error. IPC requests therefore always find a Transport. Stop increases a Ticket sequence number, which starts at 1.
- **Stop with the game open.** The game's IPC connection survives, but a game that is in a session will not be told the session ended. For v1 the page asks for confirmation when the DLL is connected ("Return to the game's main menu first…"). Delivering a session-lost message to the game is a possible follow-up, not part of this spec.
- **Quit** answers the request, shuts the Transport down gracefully, and exits the process with status 0.
- Only one UI-started dial runs at a time; a second join while `joining` is refused.

### Shared status and banners

One status struct, written at the existing sites and read by the status endpoint:

- **Game connected** becomes true when a DLL handshake succeeds and false when that connection ends, including on error paths.
- **Banners** are a list of codes; more than one can be active. Condition banners stay while the condition holds. Event banners come from the most recent join attempt and are cleared by the next join attempt and by Stop.

  | Code | Kind | Set when | Cleared when | Text on the page |
  |---|---|---|---|---|
  | `not_game_folder` | condition | the self-check fails | the self-check passes | This isn't your Game folder. Extract the whole archive into the folder that contains your game, then run datalink-mp from there. Shows the folder the Helper is running from. If only the DLL is missing, adds: your antivirus may have quarantined `dplayx.dll`; restore it or extract the archive again. |
  | `ipc_port_in_use` | condition | the IPC bind fails with address-in-use | never (restart needed) | Another program is using port N, so the game can't reach the Helper. Close the other program and start datalink-mp again. |
  | `ipc_version_mismatch` | condition | a DLL handshake carries a different IPC version, or the first message on a DLL connection fails to decode | a later handshake succeeds | The game's `dplayx.dll` doesn't match this Helper. Extract the whole archive into your Game folder again, then restart the game. |
  | `invalid_ticket` | event | a join is given text that does not parse as a Ticket, or our own Ticket | next join, Stop | That doesn't look like a Ticket. Ask your friend to copy theirs again. (For our own Ticket: That's your own Ticket. Paste your friend's.) |
  | `cant_reach_host` | event | a dial times out or cannot connect, from the UI or from the DLL's join request | next join, Stop | Couldn't reach your friend's Helper. Ask them for their current Ticket: it changes every time they start the Helper or press Stop. |
  | `peer_version_mismatch` | event | a dial is rejected for the ALPN, from the UI or from the DLL's join request | next join, Stop | Your friend has a different release of datalink-mp. You both need the same one. |

- A DLL join request that fails for another reason (for example the host has no session yet) sets no banner.
- The host sees nothing when a mismatched build tries to connect; only the joiner gets the banner.

### Game folder self-check

- The Game folder is the folder containing the Helper's own executable, found through `current_exe()`, never the working directory.
- The check passes when the folder contains `dplayx.dll` and at least one of `thinker.exe` or `terran_PRACX.exe`. File names are compared without regard to case.
- The result reports the folder path, whether the DLL was found, and which game executable was found. It is evaluated at startup and again on status requests, so restoring a file clears the banner without a restart.
- A failed check does not block anything: the Ticket is still shown and Connect still works.

### HTTP API

All API routes need the token in the `X-Token` header, except the two single-instance routes below. A missing or wrong token gets 403. The token is compared in constant time.

| Route | Purpose |
|---|---|
| `GET /` | The embedded page. No token needed: the page holds no secrets. |
| `GET /api/status` | The status snapshot (fields below). |
| `POST /api/join` | Body `{ "ticket": "…" }`. The Ticket is trimmed, parsed without I/O, and checked against our own. A bad Ticket gets 400 with the banner code, and sets the banner. A good one starts the dial and returns at once, with a `ticket_no_addresses` warning when the Ticket carries no addresses. The dial's outcome arrives through status. 409 if a dial is already in progress. |
| `POST /api/stop` | Stop. Returns once the new Ticket is in place. |
| `POST /api/quit` | Quit. |
| `GET /api/instance` | No token. Returns the application name, the Release version and the IPC port. |
| `POST /api/show` | No token. Makes this Helper open the browser at its own launch URL, under a fresh token. Limited to once every few seconds. |

Status fields: Release version, IPC version, Peer protocol version, OS (`windows`, `linux` or `macos`), state, the Ticket, the Ticket sequence number, game-connected flag, the list of peer short IDs (iroh's short form of the endpoint ID), the self-check result, the IPC port, and the banner list. No player names.

### Decisions this spec adds

The ADRs left these open.

- **UI port.** Default 47700. If the chosen port is taken, try the next nine (so 47700 to 47709 by default); if all ten are taken, exit with a clear message. The same walk applies to a port given by flag or environment variable.
- **Single instance, keyed on the IPC port, with no files.** At startup the Helper binds the IPC port. If that fails with address-in-use, it asks each port in the UI range for `GET /api/instance`. If a Helper answers with the same IPC port, the new process sends it `POST /api/replace`. A running Helper that nothing uses (no game connected, no friend connected, no join under way) answers 202 and quits as Quit does, and the new process binds the IPC port once it is free (within 10 s) and starts normally. One in use answers 409, opens its own page with the `restart_refused` banner for a minute, and keeps running; the new process prints "datalink-mp is already running and in use…" and exits with status 0. A running Helper from a release without `/api/replace` gets `POST /api/show` instead. The running Helper opens the browser itself, so the token never leaves it. Opening its own page this way rolls its token over, so a tab the player already had open stops being admitted (403) and shows the earlier-run message, the same as a tab left over from a Helper that quit: the player is left with exactly one working tab, whichever was just opened, instead of two identical-looking ones. If the browser does not open, the old token is put back, so the tab the player already has and the printed launch URL keep working. `POST /api/replace` needs no token: at worst it quits a Helper that nothing uses, which any local program could do by ending its process, and the Host, Origin and content-type rules keep web pages from sending it. Status, `GET /api/instance`, the page footer and the console all name the build (the short commit hash), so two builds of one Release version can be told apart. If no Helper answers, something else holds the port: the new process carries on and shows the `ipc_port_in_use` banner. Two Helpers on different IPC ports are separate instances and both run.
- **The two unauthenticated routes** are the one deviation from "every endpoint needs the token". They still get the `Host` check, and `POST /api/show` still rejects a foreign `Origin`. They reveal nothing secret. At worst, `POST /api/show` opens a browser tab and moves the token to it, so a tab that was already open shows the earlier-run message, and `POST /api/replace` quits a Helper that nothing uses.
- **`--no-browser`**, for tests and headless use.
- **Dial timeout** of 15 seconds.
- **Token handling in the page.** The page takes the token from `?t=`, keeps it in `sessionStorage` so a reload works, and removes it from the address bar with `history.replaceState`.
- **Archive layout.** The archive is flat, with no top-level folder, so extracting it puts the files straight into the Game folder. Files other than the two binaries carry a `datalink-mp-` prefix so they cannot overwrite the game's own readme or licence files: the Helper, `dplayx.dll`, `datalink-mp-README.txt`, `datalink-mp-LICENSE-MIT.txt`, `datalink-mp-LICENSE-APACHE.txt`. Building the archives belongs to the release pipeline spec.

### Security

- Bind `127.0.0.1` only.
- The token is 32 bytes from the OS random source, hex-encoded.
- Every request's `Host` header must be `127.0.0.1:<ui-port>` or `localhost:<ui-port>`, where the port is the one actually bound. Otherwise 403.
- State-changing routes are POST only, need `Content-Type: application/json`, and reject any `Origin` that is present and is not the Helper's own.
- No CORS headers are ever sent.
- Responses carry `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and a Content-Security-Policy that allows only the page's own inline script and style, the fonts the page carries inside it (`font-src data:`), connections to itself, and no framing.
- There are no file-writing routes, and the Helper writes no files in UI mode.

### Browser opening

- Windows: the `webbrowser` crate or `ShellExecuteW` through `windows-sys`. The `open` crate must not be used.
- Linux: `xdg-open`. macOS: `/usr/bin/open`.
- A failure to open the browser is not an error; the URL is always printed.

### The page

- One static HTML file with inline CSS and JavaScript, embedded in the binary. No framework, no build step, no external assets. It must work in a narrow window.
- Reference for layout and wording: prototype variant C on branch `prototype/ui-flow` (commit `6399164`). Replace the name "smac-helper" with "datalink-mp", drop the prototype's Steam Copy button, and remove the scenario panel.
- The page renders what status says. The Helper computes the state and the banners; the page computes only which steps are ticked:
  - Step 1 is done when the self-check passes.
  - Step 2 is done when the state is `joined` or `hosting`.
  - Step 3 is done when the game is connected.
  - Step 4 is active when steps 1 and 3 are done.
- Step 1 shows the self-check result and, when the OS is not Windows, the Wine setup text. If the check fails, the `not_game_folder` banner appears in this step instead.
- Step 2 shows the Ticket (read-only, selected on click, with Copy) beside the friend's Ticket box and Connect. Connect is disabled while the box is empty. In `joining` it shows "Connecting to your friend…"; in `joined`, "Connected to your friend". After Stop it shows "Your Ticket changed. Share it again." until the player copies the new Ticket.
- Step 3 shows "In the game: Multiplayer → Iroh P2P → Host Game (you) or Join Game (your friends)", narrowed to "Join Game" when the state is `joined`, and a pill reading "Game connected" or "Game not running".
- Step 4 shows the peer count with short IDs, and the keep-open line for the OS:
  - Windows: "Keep this browser tab and the console window open while you play."
  - Linux: "Keep this browser tab open while you play. Closing it doesn't stop the Helper; Quit does."
  - macOS: "Keep this tab and the Terminal window open while you play."
- Header: the name, Stop (hidden in `ready`) and Quit. Banners sit under the header. Footer: "Release X.Y.Z (IPC vN · Peer protocol vN) · State: …".
- Copy uses the clipboard API and falls back to selecting the text.
- Quit asks for confirmation, then replaces the page with "datalink-mp has quit. You can close this tab. Double-click datalink-mp to start it again."
- After three failed polls in a row the page shows "The Helper isn't running any more." If a poll gets 403, it shows "This tab is from an earlier run of datalink-mp. Use the new tab, or double-click datalink-mp again."

### Wine setup text (step 1, Linux and macOS)

- Intro: "Wine uses its own dplayx.dll unless you tell it to use ours. Add this to your launcher's environment."
- **Generic**, with Copy: `WINEDLLOVERRIDES="dplayx=n,b"`, placed before the launch command.
- **Faugus**, with Copy: `WINEDLLOVERRIDES=dplayx=n,b`, unquoted, pasted into the game's "Game Arguments" field, separated by a space from anything already there.
- One line with no button: "Other launchers (Steam, Lutris…) should work: add the override to the launch environment, e.g. Steam launch options `WINEDLLOVERRIDES="dplayx=n,b" %command%`."
- macOS adds: "Any Wine on macOS should work (e.g. CrossOver). Bottles live under the hidden Library folder: in Finder choose Go → Go to Folder (⇧⌘G)." and is labelled untested.
- Closing hint: "If the game never shows 'Game connected', this override is the usual cause."

### Documentation

- The repository README is rewritten for players, with the credit line "Based on smac-iroh by Henry de Valence" and a per-OS quickstart. The existing build-and-install walkthrough becomes the build-from-source document. The architecture document gets the new name.
- The same quickstart, as plain text, is the `datalink-mp-README.txt` that ships in the archives.
- Standing rule: claim only what was tested. Windows, and Linux through Faugus and the shell under GE-Proton, are tested. Steam, Lutris and plain system Wine are "should work". macOS is labelled untested throughout.
- Quickstart wording:
  - **All systems, first:** "You need Sid Meier's Alpha Centauri with Thinker or PRACX. Everyone playing needs the same release of datalink-mp."
  - **Windows:** (1) Extract the whole zip into your Game folder, the one containing `thinker.exe` or `terran_PRACX.exe`. (2) Double-click `datalink-mp.exe`. Windows will say it doesn't recognise the app: choose More info → Run anyway. If the firewall asks, allow access. (3) Your browser opens the datalink-mp page. Follow the four steps. (4) Keep the console window open while you play.
  - **Linux:** (1) Extract the whole `.tar.gz` into your Game folder. (2) In your launcher, add the Wine override: `WINEDLLOVERRIDES="dplayx=n,b"` (Faugus: paste it unquoted into Game Arguments). (3) Double-click `datalink-mp` and choose Run. (4) Your browser opens the page. Follow the four steps.
  - **macOS (untested):** (1) Extract the zip into your Game folder inside your Wine bottle. (2) Add the Wine override to your launcher. (3) Double-click `datalink-mp`. macOS will block it the first time: open System Settings → Privacy & Security, choose Open Anyway, enter your password, then double-click it again. (4) A Terminal window opens and must stay open. Your browser opens the page.
  - **Troubleshooting entries:** Smart App Control blocks unsigned apps and the only workaround is turning it off; how to restore a quarantined `dplayx.dll`; the game never shows "Game connected" (the override); your friend can't connect (ask for the current Ticket; check both have the same release).
- The SmartScreen screenshot and the release notes belong to the release pipeline spec.

## Testing Decisions

A good test here drives the Helper the way a browser, a game and a friend's Helper do, and asserts on what they can observe: HTTP responses, status fields, IPC responses, and what a peer sees. Tests do not reach into the controller, the status struct or the threading. A test that would still pass after the internals were rewritten is the kind wanted.

### Seam 1 (new, primary): the running Helper, over HTTP and IPC

The library's entry point is started inside the test process on free ports, with a temporary directory as the Game folder and a recording stand-in for the browser opener. The test then uses three real interfaces:

- an HTTP client against the JSON API;
- a fake DLL: a TCP client that speaks the IPC protocol;
- a friend: a second real Transport on loopback.

Behaviours to cover:

- A fresh Helper reports `ready`, a parseable Ticket, sequence number 1, the three versions, and no peers.
- The self-check: an empty folder, a folder with only the DLL, only a game executable, both, and differently-cased file names; and that adding the missing file clears the banner on a later status request.
- Game connected turns true after a fake DLL handshake and false when it disconnects.
- A handshake with the wrong IPC version, and a garbage first message, each set `ipc_version_mismatch`; a later good handshake clears it.
- Starting the Helper while the test holds the IPC port gives a working UI with `ipc_port_in_use`.
- Join: text that is not a Ticket, a Ticket padded with whitespace and a newline, our own Ticket, and a Ticket with no addresses.
- Join to a friend: the state goes through `joining` to `joined`, and the friend appears in the peer list on both sides.
- A friend dialling us gives `hosting`.
- Join to a Ticket whose Transport has been shut down gives `cant_reach_host` within the (shortened) timeout.
- Join to a friend built with a different Peer protocol version gives `peer_version_mismatch`.
- A fake DLL's join request to an unreachable Ticket sets `cant_reach_host`.
- Stop: the Ticket changes, the sequence number goes up, the state is `ready`, the friend sees the connection close, event banners clear, and the fake DLL's existing connection still gets answers afterwards. A join after Stop works.
- Stop and join issued from async handlers do not abort the process (the nested-runtime and runtime-drop hazards).
- Quit makes the Helper's handle finish.
- Security: no token, a wrong token, a `Host` of another name, a `Host` with another port, a foreign `Origin` on each POST, a GET on a POST route, and the absence of CORS headers. The page at `/` is served without a token and contains no `http://` or `https://` references to other hosts.
- Single instance: `GET /api/instance` reports the IPC port and the build; `POST /api/show` calls the browser opener with the launch URL; a second start on the same IPC port replaces an idle first Helper, and returns "already running" when the game or a friend is connected to the first, which then shows its page with the `restart_refused` banner; a second start on a different IPC port runs and takes the next UI port.

### Seam 2 (existing): the Transport's public API

Integration tests beside the existing mesh networking tests, in the same style:

- `shutdown()` returns within its bound, and a connected peer notices the close promptly.
- A new Transport can be created and dialled after an earlier one is shut down.
- `connected_peers()` reflects connects and disconnects.
- Two Transports with different Peer protocol versions refuse each other, and the dialler gets the mismatch error, not the can't-reach error.
- A dial to a dead Ticket fails with the can't-reach error at the configured timeout.

The existing end-to-end ordered-delivery test and the rest of the suite must keep passing unchanged, apart from the ALPN constant.

### Seam 3: the built binary

A small number of process-level smoke tests, for parity with the old CLI:

- `host` on a free IPC port prints a parseable Ticket as its first line of standard output and answers an IPC handshake.
- `join --ticket` with text that is not a Ticket exits with an error.
- With no subcommand and `--no-browser`, the launch URL is printed and the page is served at it.

### Not automated

The page's rendering and wording are checked by hand against ADR-0001's acceptance criteria 1, 2, 3 and 6, on Windows and Linux. There is no JavaScript test tooling, by decision; this is why the state and banners are computed by the Helper and covered at seam 1.

### Prior art and conventions

- The mesh networking integration tests: real Transports on loopback, a polling helper with a deadline in place of sleeps, and an early return when a Transport cannot be created (sandboxed environments). New tests at seams 1 and 2 follow all three.
- The IPC protocol's round-trip unit tests, as the model for the fake DLL's encoding.
- The Ticket unit tests, for parse cases.
- The Helper has no tests today, so seam 1 is where its first tests go.

## Out of Scope

- The release pipeline and trust work from ADR-0002: the release workflow, the three archives' construction, checksums, attestations, VERSIONINFO, the `unwind_stubs` build fix, VirusTotal, the SmartScreen screenshot and the release-candidate gate. These get their own spec. ADR-0001's acceptance criterion 8 is met there.
- A Windows "Launch game" button, a config file, Game folder auto-detection, a folder picker, and writing Wine registry overrides.
- Code signing, auto-update, a tray icon, and NAT or relay settings in the UI.
- Server-sent events. The page polls.
- A stable Ticket across Stop or relaunch.
- Player names in the UI.
- Telling the game its session ended when Stop is pressed mid-game.
- A banner on the host when a mismatched build tries to join.
- Wire-protocol or DirectPlay changes beyond the versioned ALPN, and any change to the IPC version or the Ticket format.
- Renaming the other crates or the environment variables.
- Intel Mac builds, Lutris-specific instructions, and testing on macOS.
- Automated browser tests of the page.

## Further Notes

- The new ALPN deliberately stops datalink-mp from talking to smac-iroh builds. The Peer protocol mismatch banner covers anyone who tries.
- The handshake reply to the DLL includes the Helper's Ticket, which goes stale after Stop. The DLL only logs it, so this is harmless.
- "Peers" on the page means connected Helpers, which can include a Helper connected only for session discovery. The page says "connected", not "players".
- The unauthenticated `POST /api/show` is the one route a local program without the token can use. It is rate-limited and only opens the running Helper's own page in the user's own browser, moving the token to that tab.
- macOS behaviour (Terminal window, `/usr/bin/open`, Gatekeeper steps) comes from documentation, not from a test. The macOS research notes list six things for the first Mac user to verify.
- Suggested build order for ticketing: Transport changes; controller and IPC server with the CLI on top; HTTP server and security; the page; single instance; documentation.
