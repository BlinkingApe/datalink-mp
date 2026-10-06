# 10: How does the DLL wake the game at once?

Type: grilling
Status: needs-triage
Blocked by: 06

## Question

[What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md) chose a DLL timing fix to go with the Helper acking for the friend's game. The DLL signals the game's event straight after each `Send` returns, since the Helper queues the made-up ack before it answers, and polls the Helper every 1 ms instead of 10 for inbound messages. See [the decision brief](../analysis/turn-sync-fixes.md), section 2B. It's DLL-only and timing only, with no IPC version change, for a 0.1.x patch.

Settle:
- whether it ships with, before, or independently of the Helper's acks, and whether it shares their opt-in switch
- the CPU cost of a 1 ms poll on the reference machines
- the Windows caveat: a 1 ms sleep may round up to the 15.6 ms timer tick unless the game raised the timer resolution, so measure it on Windows
- whether a blocking IPC wait, which changes the IPC version and so is a minor release, is ever worth it over the poll
