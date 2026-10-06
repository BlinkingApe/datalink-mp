# 07: What does the Helper report about each connection, to whom, and where?

Type: grilling
Status: resolved
Blocked by: 01, 05

## Question

ADR-0004's step 1 has the Helper report whether each connection is a Direct or Relayed connection, and the traffic per friend, per Turn sync. With [what Iroh exposes](01-iroh-connection-type-and-stats.md) and [what a Turn sync looks like](05-analyse-the-capture.md) known:

- **Audience:** is each figure for the player (on the page) or for diagnosing (in the log, or on a debug view)? A player can act on "Relayed", but probably not on message counts.
- **Where and how:** where on the page (for example, on step 4's friend row), in what words, and what the log line holds.
- **What counts per Turn sync:** the messages, bytes, duration, and round-trip time, using the analysis's way of finding a Turn sync.
- Whether any of it waits for 0.2.0, or all of it ships in a `0.1.x` patch as the ADR says.

## Comments

2026-10-06, from resolving [What does the page show while a Turn sync runs?](08-activity-on-the-page-during-turn-sync.md) and [What makes a Turn sync slow…](06-what-makes-turn-sync-slow.md):

- The maintainer notes that **players don't look at the Helper during a game**, so a per-Turn-sync figure on the page is unlikely to be seen. That bears on this ticket's audience question.
- The page's activity lives on each friend's row in step 4, the natural place for a Direct/Relayed badge.
- With the Helper acking for the friend's game, the Helper knows the real ack time of every message, and the "round trip" figure needs redefining as one-way hops ([the decision brief](../analysis/turn-sync-fixes.md), section 3).

## Answer

2026-10-06. Only **Direct/Relayed** is for the player. Everything else is for diagnosing, in the log.

**The page:**
- A badge on each friend's row in step 4, beside the activity text from [What does the page show while a Turn sync runs?](08-activity-on-the-page-during-turn-sync.md).
- Always shown, as plain text with no alarm colour: "Direct" or "Relayed (slower)". Nothing until a path is selected. It changes when the connection changes path.
- No RTT or other figures on the page.

**The log**, at info level, always on (no environment variable), no payloads:
- **A line per path change:** peer, Direct or Relayed, and the remote address kind (IPv4, IPv6, relay).
- **A line per finished Turn sync:** turn number, duration, messages and bytes each way, total time with nothing in flight, total hop time (ticket 09's one-way hop bucket), the connection's median RTT over the Turn sync (from Iroh's `Path::stats()`), and the Early acks made, withheld and swallowed. When a capture is on, it names the capture file.
- **A "game start" line** with the same fields, since its target (≤ 2 s) differs.
- No periodic stats line: path stats stay in the dev capture.

**What counts:** raw figures only. The Helper doesn't compute the idle share or network-time ratios; the analysis script does. A Turn sync starts at `0x8301`/`0x4301` and ends at `0x4309`, found by one small shared detector that ticket 08's page activity also uses, so the two can't disagree.

**Release:** nothing waits for 0.2.0. It ships in the same 0.1.x patch as the opt-in Early acks, because validation steps 5 and 6 and the tester games need these logs to judge the target. The page's "Syncing turn N" text can ride along or follow. It's Helper-only, with no IPC or Peer protocol change.

**Logs vs the capture:** testers send the log by default. The maintainer asks for a capture only when a game needs full analysis.
