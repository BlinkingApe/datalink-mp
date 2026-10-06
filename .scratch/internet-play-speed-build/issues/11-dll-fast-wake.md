# 11: DLL fast wake

**What to build:** With `DATALINK_FAST_WAKE=1` (read once at DLL load, default off), the DLL signals the game's event straight after each `Send` returns, and its poll thread sleeps 1 ms instead of 10 for inbound messages. It's timing only: no IPC version change. Settled in `.scratch/internet-play-speed/issues/10-dll-wakes-the-game-at-once.md`; [ADR-0005](../../../docs/adr/0005-internet-play-speed-helpers-answer-acks.md) section 3. CPU budget is under 1% of one core (DLL poll thread plus the Helper's IPC handling), measured in lobby idle and mid-turn on Windows and on Linux/Wine; if over, fall back to an adaptive poll (1 ms while messages flow, 10 ms after a quiet spell). A step logs real sleep intervals on Windows; if a 1 ms sleep rounds up to the 15.6 ms tick, choose between `timeBeginPeriod(1)` while polling and the adaptive poll.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] With the switch off, behaviour is identical to today
- [ ] The event is signalled after each `Send`
- [ ] The poll is 1 ms with the switch on
- [ ] CPU cost measured on both machines and recorded, with the adaptive poll built if over budget
- [ ] The Windows sleep interval is measured and the outcome recorded
- [ ] The IPC version is unchanged
