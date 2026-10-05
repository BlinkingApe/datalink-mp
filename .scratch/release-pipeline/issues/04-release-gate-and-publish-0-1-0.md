# 04: Release gate and publish 0.1.0

**What to build:** Nothing is built. The maintainer proves the CI-built `-rc.N` archives work on real machines, scans them, and publishes them as `0.1.0`. Only the archives from the gated RC's CI run count; local builds don't.

Run ADR-0002 section 4 (the gate) and the per-release items of section 3 (VirusTotal, WDSI), with the spec's "Decisions this spec adds" for how the RC's bytes become the release. Record each check in `docs/releasing.md` as it's run: what was done, on which machine and launcher, and anything that surprised you. Keep it to steps and gotchas a future release needs, so it doesn't restate `release.yml`. Open a new ticket for each defect found; a defect means a new `-rc.N` and a fresh gate.

**Blocked by:** 03 (macOS archive in the same release)

**Status:** ready-for-human

The gate, on the archives of one `-rc.N` draft:

- [ ] Windows smoke test: extract the `.zip` into the Game folder, SmartScreen → More info → Run anyway, the page opens, a Ticket is shown
- [ ] Windows file properties of the exe and the DLL show `0.1.0`
- [ ] One real multiplayer game between Linux (Faugus, the musl `.tar.gz`) and the maintainer's own Windows machine, both sides running only the Helper and DLL from the archives
- [ ] The same game with a Windows-using friend over the internet

Trust checks:

- [ ] VirusTotal scan of the exe and the DLL; report links saved for the release notes
- [ ] If Defender flags either: a WDSI "software developer" submission is filed, and `docs/releasing.md` says how to file and follow one up

Publish:

- [ ] Tag `v0.1.0` on the gated RC's commit and push it
- [ ] In the draft that run creates, replace the archives and `SHA256SUMS` with the gated RC's (`gh release download` from the RC, `gh release upload --clobber` to `v0.1.0`)
- [ ] `sha256sum -c SHA256SUMS` and `gh attestation verify` pass on archives downloaded from the `v0.1.0` draft
- [ ] Release notes link the VirusTotal reports and say macOS is untested
- [ ] Publish the `v0.1.0` release by hand, not marked pre-release
- [ ] `docs/releasing.md` holds the whole gate and publish sequence as it was actually run

## Comments

**2026-10-01 (agent):** Checked before the gate; no gate box is ticked, because every one needs a person, a Windows machine, a VirusTotal account or a publish decision.

- The RC to gate is `v0.1.0-rc.2` (commit `d4f9f09`, run https://github.com/BlinkingApe/datalink-mp/actions/runs/36881643024). It's the only tag on `origin` and its draft is the only release (draft, pre-release).
- Downloaded fresh from the draft: `sha256sum -c SHA256SUMS` passes for all three archives and `gh attestation verify` exits 0 for each.
- Hashes in the Windows `.zip`, for a VirusTotal hash search before uploading:
  - `datalink-mp.exe` `f5ef73aba1ece90811eda33bcc714d767278614ff0cf127fce98d1b6be50b725`
  - `dplayx.dll` `203a2ba0e63132759bc4dc095e6cb77650cb7434e60301219d868c326b9a097c` (the same in all three archives)
- `main` is one commit past the RC (`4c12b62`, ticket notes only). Tag `v0.1.0` on `d4f9f09`, the gated commit, not on `main`: `git tag v0.1.0 v0.1.0-rc.2^{}`.
- `docs/releasing.md` section 4's commands say `v0.1.0-rc.1` as an example; with this RC it's `gh release download v0.1.0-rc.2 -D gated`.

**2026-10-01 (maintainer, recorded by agent):** First gate results on `v0.1.0-rc.2`.

- A game between Linux and the maintainer's own Windows machine connected and started, both sides running only the RC's Helper and DLL. Box ticked.
- Smoke test, first try: the Windows files were copied over a USB stick, not downloaded. A Windows Security (firewall) alert appeared, was allowed, and the page opened. A USB copy has no download mark, so this doesn't test SmartScreen.
- Smoke test, second try: the `.zip` was downloaded from the GitHub draft and extracted over the first copy. There was no firewall alert (already allowed for that path) and **no SmartScreen prompt**. The cause isn't known yet: the extractor may have dropped the download mark, or SmartScreen may be off. The SmartScreen part of the smoke test is still unproven, so that box stays unticked.
- VirusTotal: 1 of about 70 engines flagged a file, Acronis (Static ML). Microsoft (Defender) shows Undetected, so no WDSI submission is needed. Still missing: which file was flagged and both report links for the release notes.
- Not yet run: file properties showing `0.1.0`, and the game with a friend over the internet.
- No defect found, so no new ticket or `-rc.N`.

**2026-10-01 (maintainer, recorded by agent):** More gate results.

- Windows file properties show `0.1.0` for the exe and the DLL. Box ticked.
- VirusTotal: the DLL is clean (0/71). The Acronis (Static ML) hit was on the exe. Report links still needed.
- SmartScreen, a third try: SmartScreen's app check is confirmed on. A fresh browser download was extracted into a new folder. The firewall alert came back, but there was still no SmartScreen prompt. Whether the extracted exe carries the download mark (Unblock / `Zone.Identifier`) hasn't been checked yet.
- The game with a friend over the internet is postponed. A substitute was proposed: the laptop on a phone's mobile data, the other machine on home Wi-Fi.

**2026-10-01 (maintainer, recorded by agent):** The smoke test passes.

- A later try showed the blue "Windows protected your PC" SmartScreen prompt, and More info → Run anyway got through. Box ticked. What made the earlier tries skip the prompt wasn't pinned down.
- The phone-hotspot internet test couldn't run: USB tethering from the phone's mobile data didn't give the laptop a connection. The internet game waits for the friend game, a few days out. The gate isn't complete until then.
- Still to come: the VirusTotal report links (exe: 1 hit, Acronis Static ML; DLL: 0/71).

**2026-10-01 (maintainer, recorded by agent):** VirusTotal reports, for the release notes. Box ticked.

- exe (1 hit, Acronis Static ML; Microsoft Undetected): https://www.virustotal.com/gui/file/f5ef73aba1ece90811eda33bcc714d767278614ff0cf127fce98d1b6be50b725
- DLL (0/71): https://www.virustotal.com/gui/file/203a2ba0e63132759bc4dc095e6cb77650cb7434e60301219d868c326b9a097c
- The WDSI box doesn't apply: Defender flagged neither file.
- The SmartScreen screenshots from this gate are now in `README.md`'s Windows quickstart (ADR-0002 section 3 asks for them).

**2026-10-01 (agent):** `v0.1.0-rc.2` is superseded; the gate starts again on `-rc.3`, so the boxes ticked for rc.2 are unticked.

- The maintainer chose to ship the new Datalink look in the first release (helper-web-ui ticket 18). The page is compiled into the Helper, so the exe's bytes change.
- Defect found while doing that: the archives' licence files lacked the `datalink-mp-` prefix the archive layout requires (ticket 05). Fixed in `release.yml` for rc.3.
- The DLL's source didn't change, but rc.3 rebuilds it, so its hash changes too. Both files need a new VirusTotal scan.
- Still valid from rc.2: the gotchas in `docs/releasing.md` (USB copies skip SmartScreen, the firewall alert is per path). The README's SmartScreen screenshots stay.

**2026-10-01 (agent):** `v0.1.0-rc.3` is up and checked, ready for the gate (run https://github.com/BlinkingApe/datalink-mp/actions/runs/36905778403, commit `401ba51`).

- `sha256sum -c SHA256SUMS` and `gh attestation verify` pass for all three archives. The archives hold the prefixed licence files (ticket 05).
- `dplayx.dll` is the same in all three archives: `5e9a8bfe64ec4f07f59e1ba6e1b5077837f90ac99c15f4cceda3e4d06398d65e`. `datalink-mp.exe`: `1dfa81c2bb415a65caf422cc269de8bf6041d0d53712813a538b97df57ad648c`. Both are new, so they need a new VirusTotal scan.
- The CI-built Linux Helper serves the themed page, with seven `@font-face` rules and `font-src data:`.
- The `v0.1.0-rc.2` draft and tag are still there, waiting for the maintainer's go-ahead to delete them.

**2026-10-01 (maintainer, recorded by agent):** Internet game with a friend.

- On `v0.1.0-rc.2` (both sides, as far as known), the game connected and worked reasonably well between Linux and Windows, but syncing between turns was very slow. Speed is the focus of 0.2.0 (ADR-0004), and 0.1.0 continues as planned.
- On `v0.1.0-rc.3`, the game's setup dialogs showed null pointers and some drop-downs didn't react. The rc.2 and rc.3 DLLs are identical apart from timestamps, so this isn't a build regression. It's a defect to investigate, ticket 06, which blocks the friend game and so the gate. If the fix touches the DLL, the gate moves to a new `-rc.N`.

**2026-10-01 (agent):** The gate moves to `v0.1.0-rc.4`, with two Helper changes from the friend game: ticket 06 (a game that left without closing its session broke the next Host Game) and helper-web-ui ticket 19 (a second start replaces an idle Helper; the footer names the build). The DLL's source is unchanged. Every gate check runs again on rc.4, including the friend game.

**2026-10-01 (agent):** `v0.1.0-rc.4` is up and checked (run https://github.com/BlinkingApe/datalink-mp/actions/runs/36923550814, commit `8fed7af`). The rc.3 pre-release and its tag are deleted, so `v0.1.0-rc.4` is the only tag.

- `sha256sum -c SHA256SUMS` and `gh attestation verify` pass for all three archives, and each holds the six expected files.
- `dplayx.dll` is the same in all three archives: `c36307b0e7b37958638cb281968993fa14e9f84ae0053de309f84c3419819f0d`. `datalink-mp.exe`: `ef16ed9aae69ab8ac8561e95752ddaf6e52c55407f10c3df33a1e23b5da1643b`. Both need a VirusTotal scan.
- The CI-built Linux Helper prints `datalink-mp 0.1.0 (build 8fed7af)`, and a second start of it made the idle first one quit and took its IPC port.

**2026-10-02 (maintainer, recorded by agent):** Real-game testing on `v0.1.0-rc.4`'s archives (`docs/datalink-mp rc4 tests.md`).

- Three Linux-Linux games (Mint hosting on wifi, Rocky joining on a mobile data hotspot) and one Windows-Linux game (the maintainer's own Windows 11 machine hosting on wifi, Rocky joining on the same hotspot). The maintainer is treating a second machine of their own on a real mobile-data path as satisfying the gate's internet-play intent for now, rather than waiting on a separate friend's computer each cycle — fast iteration while defects are still being found matters more than a literal third-party machine at this stage. Revisit an actual friend's machine once the core flow is solid.
- Linux-Linux, third attempt: a full "Simultaneous moves" game played to completion once `smooth_scrolling=0` was set in `thinker.ini` (a Host-side freeze on earlier attempts was confirmed unrelated to this project — it also happens in single-player on that box). Noticeably faster than the earlier Germany-based friend game on `rc.2` (ADR-0004 territory).
- Windows-Linux: connected, played several turns at a speed comparable to the Linux-Linux hotspot game. Ending it surfaced a new `Net::send` NULL pointer popup when the Host clicked "End Game" after the Joiner quit, apparently without the Host registering the disconnect first. Originally filed as ticket 09, now part of the [game-session-sync map](../../game-session-sync/map.md)'s [shared root cause investigation](../../game-session-sync/issues/02-shared-root-cause-across-08-09-10.md).
- Two more defects, not crashes but real: the Joiner's in-game name shows blank despite being set, on every test ([ticket 07](07-joiner-name-blank-in-game-ui.md)); and once a Joiner cancels out of a found game, "game not found" persists even after a full Joiner-side restart against the Host's still-live, never-interrupted session (originally ticket 08, also now part of the [shared root cause investigation](../../game-session-sync/issues/02-shared-root-cause-across-08-09-10.md)).
- Per this ticket's own rule, a defect found in the gate's real game means a new `-rc.N` and a fresh gate. Neither of the two multiplayer-game boxes above is ticked yet.
- Three non-blocking UX/documentation ideas also came out of this testing, originally filed as helper-web-ui tickets 20, 21 and 22; all three are now on the [post-0.1.0-polish map](../../post-0.1.0-polish/map.md).

**2026-10-02 (agent):** The crash-class defects found above, plus two more from helper-web-ui ticket 17's Criterion 3 checks (Stop mid-game breaking the session three ways) and the Joiner-cancel "no game found" defect (originally ticket 08), all look like the Game session falling out of sync with the Helper's connection state — the same shape as the already-root-caused ticket 06. Consolidated into one investigation rather than three separate tickets: the [game-session-sync map](../../game-session-sync/map.md). This gate now blocks on that map's resolution (specifically its [shared root cause ticket](../../game-session-sync/issues/02-shared-root-cause-across-08-09-10.md) and its [ticket 06 real-game confirm](../../game-session-sync/issues/01-confirm-ticket-06-fix-in-a-real-game.md)) rather than directly on the now-removed tickets 06, 08 and 09. Ticket 07 (Joiner name blank) stays here, unaffected.

**2026-10-04 (agent):** `v0.1.0-rc.5` is up and checked (run https://github.com/BlinkingApe/datalink-mp/actions/runs/37215950586, commit `8179657`). It supersedes `-rc.4` with two Helper fixes: [ticket 07](07-joiner-name-blank-in-game-ui.md) (the Joiner's in-game name was blank) and helper-web-ui [ticket 23](../../archive/helper-web-ui/issues/23-second-start-leaves-two-helpers-running.md) (a refused second start left an old tab that looked like a second Helper; the review before tagging added keeping the old token when the browser doesn't open). It also carries the ticket 06 fix from `-rc.4`, still unconfirmed in a real game (game-session-sync [ticket 01](../../game-session-sync/issues/01-confirm-ticket-06-fix-in-a-real-game.md)). The DLL's source is unchanged since `-rc.4`, but it was rebuilt, so both files need a new VirusTotal scan. Every gate check runs again on rc.5.

- `sha256sum -c SHA256SUMS` and `gh attestation verify` pass for all three archives, and each holds the six expected files.
- `dplayx.dll` is the same in all three archives: `8516d5045a5a38cac7ee6f442fe9ef9fe454b2b4e45c698e65489938700a8991`. `datalink-mp.exe`: `10b776e1c88df417ec4400d58432798661990a613f5b5adf2a7e8bdea06a9cbb`.
- The CI-built Linux Helper prints `datalink-mp 0.1.0 (build 8179657)`, and `/api/instance` names the same build.
- Worth checking in rc.5's games: the Joiner's name on its row on both screens (ticket 07's last box), a second double-click while in use leaving one working tab (ticket 23), and the ticket 06 re-host.
- The gate still waits on the [game-session-sync map](../../game-session-sync/map.md): rc.5 holds no fix for the `Net::send` popup, the stuck "game not found", or Stop mid-game.
- The `v0.1.0-rc.4` draft and tag are still there, waiting for the maintainer's go-ahead to delete them.

**2026-10-04 (maintainer, recorded by agent):** First results on `v0.1.0-rc.5`.

- [Ticket 07](07-joiner-name-blank-in-game-ui.md) is confirmed: the Joiner's name shows. Closed.
- helper-web-ui [ticket 23](../../archive/helper-web-ui/issues/23-second-start-leaves-two-helpers-running.md) is confirmed: a second double-click leaves one working page, and the old tab shows the earlier-run message.
- New defect: after both games left Multiplayer Setup and quit, with the Helpers still connected, the host hosted again and the Joiner found no game. A Joiner's Helper that closes tells the host's game its session was lost, and that message waits for the host's next game. Reproduced at the Transport level. The fix is game-session-sync [ticket 03](../../game-session-sync/issues/03-joiner-leaving-tells-the-host-its-session-was-lost.md), and it probably also explains the old ticket 08. A new `-rc.N` and a fresh gate follow once it is fixed.

**2026-10-05 (maintainer, recorded by agent):** Results on `v0.1.0-rc.6` (host Mint, Joiner Rocky, wifi; the rc.6 test checklist).

- Three gate boxes ticked, for rc.6. **Smoke test:** a browser download showed the SmartScreen prompt, More info → Run anyway worked, the firewall alert was allowed, the page opened and a Ticket was shown. **File properties:** the exe and the DLL show `0.1.0`. **Linux ↔ Windows game:** played several turns with Windows hosting and with Linux hosting. A full Linux ↔ Linux game with simultaneous moves also played to the end.
- `sha256sum -c` and `gh attestation verify` pass. The draft is pre-release and still says macOS is untested. The archives are flat, with the prefixed files.
- Not run: VirusTotal for rc.6's exe and DLL, the internet or friend game, mixed rc.5/rc.6 builds, and the Peer-protocol mismatch banner. The page footer's build hash matched rc.6, but the hash itself wasn't written down.
- game-session-sync [ticket 01](../../game-session-sync/issues/01-confirm-ticket-06-fix-in-a-real-game.md) is confirmed and closed. [Ticket 03](../../game-session-sync/issues/03-joiner-leaving-tells-the-host-its-session-was-lost.md) is confirmed in the real game, and the old ticket 08 with it.
- [Ticket 02](../../game-session-sync/issues/02-shared-root-cause-across-08-09-10.md) is still open. 09, 10b and 10c no longer show the `Net::send` NULL pointer: the host sees "Send failed!" and then the game's own "Player not responding", with no crash. **10a still crashes the Joiner's game** when the host presses Stop mid-game. One more finding, without a crash: a host game killed in Multiplayer Setup leaves the Joiner's game on the setup screen, never told the session ended.
- No defect new to rc.6, so no new `-rc.N` yet. The gate still waits on the map. By the map's standing preference, 10a's crash is in scope, so it decides whether rc.6 can ship.
- No Helper or DLL log was written on either machine. Ticket 02 has the likely causes. To get logs, start the Helper from the terminal that sets `SMAC_HELPER_LOG_FILE`, and give `DPLAYX_LOG_FILE` a `Z:\…` path under Wine.

**2026-10-05 (maintainer, recorded by agent):** Decided: the game-session-sync map no longer holds up `0.1.0` (its [ticket 02](../../game-session-sync/issues/02-shared-root-cause-across-08-09-10.md) closes with the decisions). 10, Stop mid-game, is not our problem, even where it crashes the other player's game. 09 and the killed-host case don't crash, so they ship as known issues.

- **The gate moves to `-rc.7`**, so the three boxes ticked for rc.6 are unticked. rc.7 carries two changes. The first is the Stop warning saying what can happen (game-session-sync [ticket 04](../../game-session-sync/issues/04-stop-warning-says-what-can-happen.md)), a page change. The second is the `Copyright (c) 2026 BlinkingApe` line in `LICENSE-MIT` (ADR-0003, committed after rc.6 was tagged), which the archives ship. The session code is unchanged from rc.6.
- The release notes list as known issues: 09 ("Send failed!" then "Player not responding" when a friend's Helper drops mid-game), the killed-host case (the Joiner stays on the setup screen), and Stop mid-game possibly crashing the other player's game.
- What's left is in `docs/datalink-mp-0.1.0-release-checklist.html`.
