# 06: What the Joiner's step 3 shows

Type: grilling
Status: needs-triage
Blocked by: 05

## Question

What does the Joiner's step 3 (`crates/datalink-mp/src/page.html`, heading "Start the game" today) show in each state, and what does the tab title say? The design comes from [Tell Joiners when the Host is hostable](04-signal-joiners-when-host-is-hostable.md). Whether a "started" state exists depends on [Does SMAC mark a started game closed to new players?](05-does-smac-mark-a-started-game-closed.md). If SMAC doesn't flag a started game, any open Game session counts as hostable, and a late Joiner gets today's "game not found".

Starting point, proposed while grilling ticket 04:

- **Connected, Host not hostable:** heading "Wait for your friend's game". Body: "Your friend hasn't pressed Host Game yet. This changes as soon as they do."
- **Hostable:** heading "Start the game". Body: "Your friend's game is ready." The existing menu hint (Multiplayer → Iroh P2P → Join Game) is highlighted.
- **Started or full** (only if ticket 05 finds it detectable): "Your friend's game has already started. Ask them to host a new one."
- **Joiner's game in the Game session:** as today.
- **Tab title:** e.g. "● Friend is hosting" while hostable.
