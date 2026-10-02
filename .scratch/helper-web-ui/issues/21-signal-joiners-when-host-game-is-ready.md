# 21: Tell Joiners to wait for the Host's game to be ready

**What to build:** Idea from the 0.1.0 gate testing. A Joiner who reaches the game's "Join Game" entry before the Host has actually gotten there gets "game not found" with no explanation. Two angles, either or both:

- Can the Host's Helper tell a connected Joiner's Helper once the Host's game has reached the point where it's actually hostable (or can the Host player signal this manually some other way)? Testers noted they can already pass Tickets to each other through some other channel, so a manual host-side signal is a realistic fallback if automatic detection isn't feasible.
- Regardless, the Joiner's step 3 text currently reads "Start the game" as soon as the Helpers are connected. It should say something like "Wait for your friend to say their game is ready, then start the game", so a Joiner doesn't assume connected-Helpers means go-ahead-in-game.

**Blocked by:** None

**Status:** needs-triage

Not a blocker for 0.1.0.
