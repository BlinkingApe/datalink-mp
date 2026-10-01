# 06: Null pointers in the game's multiplayer setup dialogs

**What happened:** Found during the 0.1.0 gate, in the internet game with a friend (Linux and Windows, 2026-10-01). On one attempt the game's multiplayer setup dialogs showed null pointers when used, and some drop-downs didn't react at all. Another attempt worked: the game connected and played, slowly (ADR-0004).

The failing attempt used `v0.1.0-rc.3` and the working one `v0.1.0-rc.2`, but **the build isn't the cause**. The two DLLs differ in 6 bytes, all timestamps and the header checksum (PE `TimeDateStamp`, `CheckSum`, and the export directory's `TimeDateStamp`). Every section is the same size at the same place. No DLL source changed between the two RCs, and both CI runs used Rust 1.99.0 and the same mingw packages. So the difference was in the setup, the timing or the roles, and the bug can probably happen with either build.

**Blocked by:** None

**Status:** ready-for-human

Leads, unconfirmed:

- `GetSessionDesc` hands the game a session description whose `lpszSessionName` is null, marked "Caller must handle" (`crates/dplayx/src/directplay.rs`, around line 1468). A dialog that reads the session name from it gets a null pointer, but only on the code paths that call it. That could depend on who hosts.
- **Timing over a slow connection.** If a dialog asks for a player's name or the session's settings before the friend's Helper has sent them, the DLL may have nothing to give. On a LAN the data arrives first. Over a relayed connection it may not.
- **The wrong DLL loaded.** Without `WINEDLLOVERRIDES="dplayx=n,b"`, Wine uses its own, incomplete `dplayx.dll`. A stale Helper or DLL from another build can give confusing results too (one stale Helper was found on the Linux machine that day).

Needed from the maintainer:

- [x] What "null pointers" looked like: an error box titled `Net::send`, "Oh no! NULL pointer!!", over the game's Multiplayer Setup screen. The drop-downs on the host's own row (faction, difficulty) didn't react.
- [x] Which side showed it: the host, in both failing attempts
- [ ] Logs from a failing attempt, if it happens again: `DPLAYX_LOG_FILE=/path/dplayx.log` and `SMAC_HELPER_LOG_FILE=/path/helper.log` in the launcher's environment (Faugus: Game Arguments)

Then:

- [x] Reproduce: `test_game_that_left_without_closing_its_session_can_host_again` (`crates/datalink-mp/tests/ipc.rs`) failed with `Failed to create session (already in session?)`
- [x] Fix in the Helper: closing the session when the game disconnects
- [ ] Confirm in the real game, on a new `-rc.N`: host, close the game while in the setup screen, start it again with the same Helper, host again; no `Net::send` error, and the drop-downs react

## Comments

**2026-10-01 (agent):** Cause found, by reading the code and reproducing it at the IPC level. It's the Helper, not the DLL.

- `Net::send` and "Oh no! NULL pointer!!" are the game's own: the game tried to send a message and something it needed was missing.
- When the game's connection to the Helper ended, the Helper only marked the game as not connected. A game that crashed or was closed while hosting never sent CloseSession, so **its session stayed in the Helper**. The next Host Game got "Failed to create session (already in session?)", the DLL returned `DPERR_CANTCREATESESSION`, and the game carried on into Multiplayer Setup with no session behind it. The host's first change on its own row makes the game send the new settings, and `Net::send` fails.
- That explains why only the host saw it, why it repeated on the same Helper, and why a freshly started Helper worked. The rc.2/rc.3 difference was only which Helper was fresh.
- Fix: `SessionController::dll_disconnected` closes the session when the game leaves it open. Closing tells connected friends the session ended (`SessionClosed`) and keeps the connections, as CloseSession always did.
- The `GetSessionDesc` null session name and the timing lead above weren't needed to explain this. The null name stays a known oddity, not a cause.
