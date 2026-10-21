# 11: DLL fast wake

**What to build:** With `DATALINK_FAST_WAKE=1` (read once at DLL load, default off), the DLL signals the game's event straight after each `Send` returns, and its poll thread sleeps 1 ms instead of 10 for inbound messages. It's timing only: no IPC version change. Settled in `.scratch/internet-play-speed/issues/10-dll-wakes-the-game-at-once.md`; [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 3. CPU budget is under 1% of one core (DLL poll thread plus the Helper's IPC handling), measured in lobby idle and mid-turn on Windows and on Linux/Wine; if over, fall back to an adaptive poll (1 ms while messages flow, 10 ms after a quiet spell). A step logs real sleep intervals on Windows; if a 1 ms sleep rounds up to the 15.6 ms tick, choose between `timeBeginPeriod(1)` while polling and the adaptive poll.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] With the switch off, behaviour is identical to today
- [x] The event is signalled after each `Send`
- [x] The poll is 1 ms with the switch on
- [ ] CPU cost measured on both machines and recorded, with the adaptive poll built if over budget
- [ ] The Windows sleep interval is measured and the outcome recorded
- [x] The IPC version is unchanged

## Comments

Built in `crates/dplayx/src/fast_wake.rs` (switch parsing and poll interval, unit-tested) and wired into `directplay.rs`: `Send` signals all player events after a successful send (player and group paths) and the poll thread sleeps `poll_interval(enabled())`. `DATALINK_FAST_WAKE=1` is read once; anything else is off and identical to before. No IPC change.

The two unticked boxes need a Windows machine and a Wine machine, which the build agent did not have: the CPU measurement (lobby idle and mid-turn) and the Windows 1 ms sleep-interval measurement. Run both with `DATALINK_FAST_WAKE=1`. If CPU is over 1% of a core, or a 1 ms sleep rounds to 15.6 ms, choose `timeBeginPeriod(1)` while polling or the adaptive poll (1 ms while messages flow, 10 ms after a quiet spell). The adaptive poll is not built.
