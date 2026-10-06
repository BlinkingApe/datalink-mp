# 01: Recognise wtp.exe as a game executable

**What to build:** The Helper's game-folder self-check accepts `wtp.exe` as a game executable, the same way it accepts `thinker.exe` and the PRACX exe. A **Game folder** holding the DLL and `wtp.exe` passes the self-check and reports `wtp.exe` as its game executable. Matching is case-insensitive, as for the others. The **Game folder** entry in `CONTEXT.md` and the README wording name it too.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A folder with `dplayx.dll` and `wtp.exe` passes the self-check and reports `wtp.exe`
- [ ] Different letter case still matches
- [ ] A test sits beside the Thinker and PRACX ones
- [ ] `CONTEXT.md` and the README mention it
