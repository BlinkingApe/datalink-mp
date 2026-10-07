# The Stop warning says what can happen to the other players' games

Type: task
Status: resolved
Blocked by:

## Why

[Ticket 02](02-shared-root-cause-across-08-09-10.md) closed defect 10 as not our problem: pressing Stop mid-game, past the warning, costs the game in progress, and the other player's game can crash (10a, seen on `-rc.6`). The map says any defect closed that way owes a warning that says concretely what can happen. Today's warning only gives the instruction:

> Return to the game's main menu first. Stop disconnects everyone connected through you and gives you a new Ticket. Stop now?

It's needed in `0.1.0`, so it goes into `-rc.7`.

## What to build

- [ ] The confirm shown when Stop is pressed with the game connected (`crates/datalink-mp/src/page.html`, the `stopButton` handler) says that stopping during a game ends that game for everyone connected through you and can crash their game. Keep the instruction first and the dialog short. Suggested wording: "Return to the game's main menu first. Stopping during a game ends it for everyone connected through you, and can crash their game. Stop disconnects them and gives you a new Ticket. Stop now?"
- [ ] The README's paragraph on Stop (under "The four steps on the page") says the same, in the same words where they fit. `packaging/README.txt` follows the README.
- [ ] Any test that matches the old wording is updated, and `cargo test --workspace` passes.
