# 06: What makes a Turn sync slow, and what do we fix?

Type: grilling
Status: needs-triage
Blocked by: 05

## Question

Given [the capture's analysis](05-analyse-the-capture.md): what dominates a slow Turn sync (the relay, the bytes, round trips, or Helper queueing), and which fixes does 0.2.0 pursue? Choose from ADR-0004's compression (step 2) and sending only what changed (step 3), running our own relay, a latency fix, or a mix. Choose by expected gain on the measured budget, not by the ADR's order.

Also settle:

- the numeric target for a Turn sync on the reference setup, now that there's a baseline
- whether ADR-0004 needs amending or superseding

Each fix chosen graduates the map's "design of each fix" fog into its own ticket.
