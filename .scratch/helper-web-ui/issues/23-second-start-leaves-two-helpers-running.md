# 23: A refused second start leaves two Helpers running

**What happened:** Found during ticket 17's manual check of Criterion 1 (2026-10-02, Windows and Linux). Double-clicking the Helper a second time showed the amber "Already running" banner (the `restart_refused` path from [ticket 19](19-a-second-start-replaces-an-idle-helper.md)), but the new process didn't exit the way ticket 19 describes — it kept running as a second, fully usable Helper alongside the first. Confusing (two identical-looking windows/pages), though either one works fine on its own.

Per ticket 19: when the running Helper is in use, it should keep running and show `restart_refused` on its own page, while the *new* process prints the "already running and in use" message and exits with status 0. Only one Helper should remain. That's not what happened here.

**Blocked by:** None. Likely a regression or gap in ticket 19's `/api/replace` handling — worth checking whether this reproduces the idle-replace path (ticket 19's own test) or only the in-use-refuse path.

**Status:** resolved

- [X] Reproduce with the fake-DLL/friend harness from ticket 19's tests, with a peer or the DLL connected to the first Helper, and confirm whether the second process actually calls `/api/replace`, receives 409, and exits — or whether it's not reaching that exchange at all
- [X] Fix so the second process exits once refused
- [X] Confirm by hand: with a game or friend connected to a running Helper, double-click it again — only one Helper is left running, with the amber banner on the original

## Comments

**2026-10-04 (agent):** Investigated before changing anything. The second process already does exit correctly once refused: `/api/replace` → 409 → `StartError::AlreadyRunning` → the binary prints "already running" and exits with status 0, in every case tried — the existing automated coverage (`test_second_start_leaves_a_first_helper_the_game_is_connected_to_and_its_page_says_why` and the real-subprocess `test_binary_started_twice_on_the_same_ipc_port_says_already_running_and_exits_with_status_0`), a hand-run repro with a real peer joined over a real Transport, and a forced simultaneous double-start, all showed exactly one process surviving. Checked against the running machine live: at the time of writing, `ps`/`ss` showed exactly one `datalink-mp` process and one listener on the UI port, confirming there is no process-duplication bug.

What's real: the running Helper answers a refused replace by opening its own page again (by design, so a player who lost track of their tab can find it). If the player's original tab is still open, this leaves two tabs pointing at the identical single Helper, which look like two separate Helpers even though they're one — matching "two identical-looking windows/pages, though either one works fine" from the original report.

The idle-replace path doesn't have this problem: the first process actually quits, so its old tab's token stops being admitted (403) and the page shows "This tab is from an earlier run of datalink-mp." The in-use-refuse path kept the *same* process and *same* token running, so neither tab ever looked "old."

Fix: `POST /api/show` (called directly for the single-instance probe, and internally by `/api/replace`'s in-use branch) now rolls the HTTP token over before opening the browser at the fresh URL. The token is shared between the page's auth guard and the HTTP server's app state via `Arc<Mutex<String>>` (`http::SharedToken`). Any tab still holding the old token gets 403 on its next poll and shows the existing earlier-run message; the newly (re)opened tab carries the new token and keeps working. This makes the in-use case behave the same as every other second-start case: exactly one tab ends up current, and any other tab says so.

Updated `test_second_start_leaves_a_first_helper_the_game_is_connected_to_and_its_page_says_why`, `test_second_start_leaves_a_first_helper_a_friend_is_connected_to` and `test_show_needs_no_token_and_opens_the_browser_at_the_launch_url` to assert the old token now gets 403 and the freshly opened URL's token works. Full suite (113 tests) passes.

**2026-10-04 (agent):** Follow-up from the review before `v0.1.0-rc.5`. The roll-over happened before the browser was asked to open, and the opener reported nothing back. If the open failed (no `xdg-open`, or `ShellExecuteW` refusing), the player's existing tab got 403 and the printed launch URL went dead, with no new tab to replace them. While a game held the Helper, every later double-click was refused and rolled the token again, leaving no way back to the page. The opener now answers whether it started the browser (`BrowserOpener` returns `bool`). When it didn't, `show` puts the old token back, so the existing tab and the printed URL keep working. Test: `test_show_whose_browser_did_not_open_keeps_the_tab_the_player_already_has`. An opener that starts but fails later (an `xdg-open` that exits non-zero) still can't be detected; the spec's "Single instance" decision now says what a roll-over does to the printed URL.
