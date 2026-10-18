# 10: dev-faults cargo feature

**What to build:** A dev-only cargo feature `dev-faults`, absent from release builds, lets a tester inject faults during a game: `DATALINK_FAULT=drop:N` closes the friend's connection after N early acks, and `reset:N` resets the stream. They are for the fault-injection validation, run during the game start's state sync.

**Blocked by:** 07, 08

**Status:** ready-for-agent

- [ ] Release builds don't contain the feature (checked)
- [ ] `drop:N` and `reset:N` behave as described in a Transport test
- [ ] They log when they fire

## Comments

- 2026-10-18: The dev-faults adapter is a third adapter at the frame-sink seam from [transport-message-path](../../transport-message-path/spec.md), wrapping the iroh one.
