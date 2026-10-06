# 10: How does the DLL wake the game at once?

Type: grilling
Status: resolved
Blocked by: 06

## Question

[What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md) chose a DLL timing fix to go with the Helper acking for the friend's game. The DLL signals the game's event straight after each `Send` returns, since the Helper queues the **Early ack** before it answers (confirmed by [How does the Helper ack for the friend's game?](09-helper-acks-for-the-friends-game.md): it's made at enqueue, inside `Transport::send`), and polls the Helper every 1 ms instead of 10 for inbound messages. See [the decision brief](../analysis/turn-sync-fixes.md), section 2B. It's DLL-only and timing only, with no IPC version change, for a 0.1.x patch.

Settle:
- whether it ships with, before, or independently of the Helper's acks, and whether it shares their opt-in switch (`DATALINK_EARLY_ACKS`, an environment variable on the Helper; the DLL would need its own or ask the Helper)
- the CPU cost of a 1 ms poll on the reference machines
- the Windows caveat: a 1 ms sleep may round up to the 15.6 ms timer tick unless the game raised the timer resolution, so measure it on Windows
- whether a blocking IPC wait, which changes the IPC version and so is a minor release, is ever worth it over the poll

## Answer

Decided with the maintainer.

- **Ships with Early acks, in the same 0.1.x patch,** but behind its own DLL switch: `DATALINK_FAST_WAKE=1`, read once at DLL load, default off. This mirrors the Helper's `DATALINK_EARLY_ACKS=1`, so each can be tested alone. Both flip to on by default in the release after they're validated together. It's DLL-only timing, with no IPC change.
- **Two parts.** (1) The DLL signals the game's event straight after each `Send` returns, which costs nothing. (2) The poll thread's sleep drops from 10 ms to 1 ms for inbound messages (`directplay.rs`, the polling thread).
- **Blocking IPC wait: not worth it.** It would bump `PROTOCOL_VERSION` 3 → 4 and make this a minor release (ADR-0002), for a gain the 1 ms poll mostly gets. It returns only if the poll's CPU cost can't be brought under budget.
- **CPU budget: under 1% of one core** (DLL poll thread plus the Helper's IPC handling), measured in lobby idle and mid-turn on both the Windows machine and Linux/Wine. If over budget, fall back to an **adaptive poll**: 1 ms while messages are flowing, back to 10 ms after a quiet spell. Part (1) ships either way.
- **Windows 15.6 ms timer tick: measure first, decide in the build.** The plan includes a step that logs real sleep intervals on the Windows machine. If a 1 ms sleep rounds up, the build chooses between `timeBeginPeriod(1)` while the poll thread runs and the adaptive poll. Not pre-decided.
- **Validation:** part of the same six-step validation as Early acks. Add the measured Turn sync network time with `DATALINK_FAST_WAKE` on and off, against the ≤ 1 s Direct target.
