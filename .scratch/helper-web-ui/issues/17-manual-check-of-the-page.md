# 17: Manual check of the page on Windows and Linux

**What to build:** Nothing is built. A person checks by hand that the page looks and reads as specified and that two real machines can play together. There is no JavaScript test tooling, by decision, so the page's rendering and wording are covered only here. The automated tests cover the state and the banners as the Helper computes them.

Run the checks on Windows and on Linux (Faugus or the shell under GE-Proton), against ADR-0001's acceptance criteria 1, 2, 3 and 6. Record what was tested, on which OS and launcher, under a `## Comments` heading in this file, and open a new ticket for each defect found.

Reference for layout and wording: prototype variant C on branch `prototype/ui-flow` (commit `6399164`) and the spec's "The page" and "Wine setup text" sections.

**Blocked by:** 06 (Startup), 07 (HTTP security hardening), 08 (Step 1), 09 (Step 3), 10 (Step 4), 11 (Step 2), 12 (Dial failure banners), 13 (Stop), 14 (Quit), 15 (Single instance)

**Status:** ready-for-human

Criterion 1: first run

- [ ] Windows: with the Helper and DLL in the Game folder, double-clicking the Helper opens the browser to the page, the self-check passes and a Ticket is shown, with no commands typed
- [ ] Linux: the same, by double-click and Run
- [ ] The address bar shows no `?t=` after the page loads, and a reload keeps working
- [ ] Double-clicking the Helper a second time opens the running Helper's page and starts no second Helper

Criterion 2: two machines play

- [ ] A second machine (Windows, or Linux with the override set) pastes the Ticket, presses Connect, and sees "Connecting to your friend…" then "Connected to your friend"
- [ ] The host's page shows the friend under step 4 with a short ID
- [ ] Both games show "Game connected", and the two play a game together

Criterion 3: Stop

- [ ] Stop gives a new Ticket without quitting, and the page shows "Your Ticket changed. Share it again." until it is copied
- [ ] With the game open, Stop asks for confirmation first
- [ ] A game that stayed connected to the Helper works through a later session
- [ ] The friend's page drops the peer promptly

Criterion 6: each banner is triggered by its condition

- [ ] `not_game_folder`: run the Helper from the Downloads folder; the banner shows that folder
- [ ] `not_game_folder` with the quarantine sentence: remove only `dplayx.dll`; restoring it clears the banner without a restart
- [ ] `ipc_port_in_use`: hold the IPC port with another program, then start the Helper
- [ ] `ipc_version_mismatch`: use a DLL from a build with a different IPC version
- [ ] `invalid_ticket`: paste text that is not a Ticket, and paste your own Ticket
- [ ] `cant_reach_host`: paste a Ticket from a Helper that has since been Stopped; the banner arrives in about fifteen seconds
- [ ] `peer_version_mismatch`: join a build with a different Peer protocol version

Page and wording

- [ ] The page stays usable in a narrow window beside the game
- [ ] Copy works for the Ticket and for both Wine override strings; with clipboard access blocked, the text is selected instead
- [ ] Windows shows no Wine setup text; Linux shows it with the Faugus note
- [ ] The keep-open line matches the OS
- [ ] Quit asks for confirmation and leaves the "has quit" page; closing the Helper's window instead leads to "The Helper isn't running any more."
- [ ] A tab left over from an earlier run shows the earlier-run message
- [ ] The footer shows the Release version, IPC version and Peer protocol version
- [ ] With the network unplugged from the internet, the page still loads
