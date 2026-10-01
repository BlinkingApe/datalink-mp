# 06: Null pointers in the game's multiplayer setup dialogs

**What happened:** Found during the 0.1.0 gate, in the internet game with a friend (Linux and Windows, 2026-10-01). On one attempt the game's multiplayer setup dialogs showed null pointers when used, and some drop-downs didn't react at all. Another attempt worked: the game connected and played, slowly (ADR-0004).

The failing attempt used `v0.1.0-rc.3` and the working one `v0.1.0-rc.2`, but **the build isn't the cause**. The two DLLs differ in 6 bytes, all timestamps and the header checksum (PE `TimeDateStamp`, `CheckSum`, and the export directory's `TimeDateStamp`). Every section is the same size at the same place. No DLL source changed between the two RCs, and both CI runs used Rust 1.99.0 and the same mingw packages. So the difference was in the setup, the timing or the roles, and the bug can probably happen with either build.

**Blocked by:** None

**Status:** needs-info

Leads, unconfirmed:

- `GetSessionDesc` hands the game a session description whose `lpszSessionName` is null, marked "Caller must handle" (`crates/dplayx/src/directplay.rs`, around line 1468). A dialog that reads the session name from it gets a null pointer, but only on the code paths that call it. That could depend on who hosts.
- **Timing over a slow connection.** If a dialog asks for a player's name or the session's settings before the friend's Helper has sent them, the DLL may have nothing to give. On a LAN the data arrives first. Over a relayed connection it may not.
- **The wrong DLL loaded.** Without `WINEDLLOVERRIDES="dplayx=n,b"`, Wine uses its own, incomplete `dplayx.dll`. A stale Helper or DLL from another build can give confusing results too (one stale Helper was found on the Linux machine that day).

Needed from the maintainer:

- [ ] What "null pointers" looked like: a crash or error dialog, or "(null)" shown as text in a field? Which dialog, and which drop-downs?
- [ ] Which side showed it (host or joiner, Linux or Windows), and who hosted in each attempt
- [ ] Logs from a failing attempt, if it happens again: `DPLAYX_LOG_FILE=/path/dplayx.log` and `SMAC_HELPER_LOG_FILE=/path/helper.log` in the launcher's environment (Faugus: Game Arguments)

Then:

- [ ] Reproduce, if the timing lead holds, by adding latency between two local Helpers (for example `tc qdisc ... netem delay` on loopback)
- [ ] Fix in the DLL. That means a new `-rc.N` and a fresh gate (ticket 04)
