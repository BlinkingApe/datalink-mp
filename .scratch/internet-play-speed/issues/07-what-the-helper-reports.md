# 07: What does the Helper report about each connection, to whom, and where?

Type: grilling
Status: needs-triage
Blocked by: 01, 05

## Question

ADR-0004's step 1 has the Helper report whether each connection is a Direct or Relayed connection, and the traffic per friend, per Turn sync. With [what Iroh exposes](01-iroh-connection-type-and-stats.md) and [what a Turn sync looks like](05-analyse-the-capture.md) known:

- **Audience:** is each figure for the player (on the page) or for diagnosing (in the log, or on a debug view)? A player can act on "Relayed", but probably not on message counts.
- **Where and how:** where on the page (for example, on step 4's friend row), in what words, and what the log line holds.
- **What counts per Turn sync:** the messages, bytes, duration, and round-trip time, using the analysis's way of finding a Turn sync.
- Whether any of it waits for 0.2.0, or all of it ships in a `0.1.x` patch as the ADR says.
