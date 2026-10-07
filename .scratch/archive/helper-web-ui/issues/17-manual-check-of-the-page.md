# 17: Manual check of the page on Windows and Linux

**What to build:** Nothing is built. A person checks by hand that the page looks and reads as specified and that two real machines can play together. There is no JavaScript test tooling, by decision, so the page's rendering and wording are covered only here. The automated tests cover the state and the banners as the Helper computes them.

Run the checks on Windows and on Linux (Faugus or the shell under GE-Proton), against ADR-0001's acceptance criteria 1, 2, 3 and 6. Record what was tested, on which OS and launcher, under a `## Comments` heading in this file, and open a new ticket for each defect found.

Reference for layout and wording: prototype variant C on branch `prototype/ui-flow` (commit `6399164`) and the spec's "The page" and "Wine setup text" sections.

**Blocked by:** 06 (Startup), 07 (HTTP security hardening), 08 (Step 1), 09 (Step 3), 10 (Step 4), 11 (Step 2), 12 (Dial failure banners), 13 (Stop), 14 (Quit), 15 (Single instance); for the Windows checks, also release-pipeline 02 (a CI-built Windows archive to test)

**Status:** resolved

Criterion 1: first run

- [X ] Windows: with the Helper and DLL in the Game folder, double-clicking the Helper opens the browser to the page, the self-check passes and a Ticket is shown, with no commands typed
- [X ] Linux: the same, by double-click and Run
- [ X] The address bar shows no `?t=` after the page loads, and a reload keeps working
- [ s] Double-clicking the Helper a second time opens the running Helper's page and starts no second Helper [EDIT: no longer true in rc4, behavior is now an amber banner appears //ALREADY RUNNING etc; however a second identical Helper appears, this is perhaps confusing, though either one can be used without issue] [now tracked by ticket 23] EDIT: Resolved

Criterion 2: two machines play

- [x] A second machine (Windows, or Linux with the override set) pastes the Ticket, presses Connect, and sees "Connecting to your friend…" then "Connected to your friend"
- [ x] The host's page shows the friend under step 4 with a short ID [now tracked by the post-0.1.0-polish map's carried-over ticket 01] Edit Resolved
- [x] Both games show "Game connected", and the two play a game together

Criterion 3: Stop

- [ X] Stop gives a new Ticket without quitting, and the page shows "Your Ticket changed. Share it again." until it is copied
- [ X] With the game open, Stop asks for confirmation first
- [x ] A game that stayed connected to the Helper works through a later session [Pressing stop in Host Helper and clicking through the warning immediately closed Joiner's game to desktop (crash?); re-connection with new Ticket possible, but not able to re-join ongoing game ("No game found"); Same test with Joiner pressing Stop while a connected game in-progress, White Net Underground "Send failed!" popup on Host after end of turn, then in-game dialogue "Player not responding"; Third test, Joiner presses Stop during on-going game, neither Host nor Joiner resond to the in-game "Drop the host/player" dialogue, new iroh conneciton is established, but nonetheless when turn ends, same White Net Underground "Send failed" popup ] [now tracked by the game-session-sync map]
- [ X] The friend's page drops the peer promptly

Criterion 6: each banner is triggered by its condition

- [ X] `not_game_folder`: run the Helper from the Downloads folder; the banner shows that folder
- [ X] `not_game_folder` with the quarantine sentence: remove only `dplayx.dll`; restoring it clears the banner without a restart [Works but may be confusingly worded; this IS my game folder, only without dxplay.dll, maybe the wording should say that specifically]
- [ X] `ipc_port_in_use`: hold the IPC port with another program, then start the Helper
- [X ] `ipc_version_mismatch`: use a DLL from a build with a different IPC version
- [ X] `invalid_ticket`: paste text that is not a Ticket, and paste your own Ticket
- [X ] `cant_reach_host`: paste a Ticket from a Helper that has since been Stopped; the banner arrives in about fifteen seconds
- [ X] `peer_version_mismatch`: join a build with a different Peer protocol version

Page and wording

- [X ] The page stays usable in a narrow window beside the game
- [ X] Copy works for the Ticket and for both Wine override strings; with clipboard access blocked, the text is selected instead
- [ X] Windows shows no Wine setup text; Linux shows it with the Faugus note
- [ X] The keep-open line matches the OS
- [ X] Quit asks for confirmation and leaves the "has quit" page; closing the Helper's window instead leads to "The Helper isn't running any more."
- [X ] A tab left over from an earlier run shows the earlier-run message
- [X] The footer shows the Release version, IPC version and Peer protocol version
- [ X] With the network unplugged from the internet, the page still loads

**2026-10-01 (agent):** The Windows checks are unblocked. The draft pre-release `v0.1.0-rc.2` (release-pipeline ticket 02) holds the CI-built `datalink-mp-0.1.0-windows-x86_64.zip`. Get it with `gh release download v0.1.0-rc.2 -R BlinkingApe/datalink-mp`; drafts are only visible to maintainers.

**2026-10-02 (maintainer, recorded by agent):** Criterion 2 checked against `v0.1.0-rc.4`'s archives (`docs/datalink-mp rc4 tests.md`), both Linux-Linux (Mint hosting, Rocky joining on a mobile hotspot) and Windows-Linux (the maintainer's own Windows 11 machine hosting, Rocky joining). The Joiner's page showed "Connecting to your friend…" then "Connected to your friend" each time, and full games were played (one Linux-Linux game to completion, one Windows-Linux game for several turns before an unrelated defect ended it — originally filed as release-pipeline ticket 09, since moved to the [game-session-sync map](../../../game-session-sync/map.md)). Not confirmed: whether the host's page shows the friend's short ID at step 4 — the log only mentions a peer count, which testers suggested also surfacing earlier (originally helper-web-ui ticket 20, since moved to the [post-0.1.0-polish map](../../../post-0.1.0-polish/map.md)). This same testing surfaced two more defects: the Joiner's in-game name shows blank, filed against release-pipeline as [ticket 07](../../release-pipeline/issues/07-joiner-name-blank-in-game-ui.md); and "game not found" persists after a Joiner cancels once, originally release-pipeline ticket 08, also since moved to the game-session-sync map.

**2026-10-02 (maintainer, recorded by agent):** Criteria 1, 3 and 6, and the page/wording section, checked on both Windows and Linux. Almost everything passes as specified. Three findings:

- Criterion 1's second-start item: now that [ticket 19](19-a-second-start-replaces-an-idle-helper.md) is in, double-clicking the Helper a second time while it's in use shows the amber "Already running" banner as designed, but the new process doesn't exit the way ticket 19 describes — a second, fully usable Helper is left running alongside the first. Filed as [ticket 23](23-second-start-leaves-two-helpers-running.md).
- Criterion 3's "a game that stayed connected works through a later session": three attempts, all mid-game, found the opposite. Host-side Stop crashed the Joiner's game to desktop and left the Joiner unable to rejoin the same session ("No game found") despite a fresh Ticket; Joiner-side Stop produced a "Net Underground" "Send failed!" popup on the Host at the next end of turn, with "Player not responding" following; a third attempt (neither side acknowledging the in-game drop-player dialogue) got a new iroh connection but the same "Send failed" recurred. Originally filed as release-pipeline ticket 10; now consolidated, along with release-pipeline tickets 06, 08 and 09, into the [game-session-sync map](../../../game-session-sync/map.md) as one investigation rather than four separate tickets.
- The `not_game_folder` quarantine banner works but reads like a generic wrong-folder message when the folder is actually right and only the DLL is missing. Filed as a wording idea, originally ticket 24, now on the [post-0.1.0-polish map](../../../post-0.1.0-polish/map.md); not a blocker.

**2026-10-02 (maintainer, recorded by agent):** Criterion 3's remaining item, "the friend's page drops the peer promptly", tested and confirmed. Only unresolved box left in this ticket is Criterion 2's host-side short-ID display, which waits on originally-ticket-20, now on the [post-0.1.0-polish map](../../../post-0.1.0-polish/map.md).

**2026-10-02 (agent):** Closing. Every check against ADR-0001's acceptance criteria 1, 2, 3 and 6 that could be run by hand has been, on both Windows and Linux, and every defect or idea it surfaced has its own ticket. The three boxes left unchecked above aren't abandoned — each is now tracked elsewhere and will be ticked off by that ticket's own resolution, not by reopening this one:

- Criterion 1's second-start item → [ticket 23](23-second-start-leaves-two-helpers-running.md), here in helper-web-ui (near-term defect).
- Criterion 2's short-ID-at-step-4 item → the [post-0.1.0-polish map](../../../post-0.1.0-polish/map.md)'s carried-over ticket 01 (deferred, non-blocking).
- Criterion 3's "works through a later session" item → the [game-session-sync map](../../../game-session-sync/map.md) (open investigation, severity still being decided per-case).
