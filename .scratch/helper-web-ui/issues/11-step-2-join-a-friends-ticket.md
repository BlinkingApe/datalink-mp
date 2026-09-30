# 11: Step 2: join a friend's Ticket from the page

**What to build:** A player pastes their friend's Ticket into the box beside their own, presses Connect, sees "Connecting to your friend…", and then "Connected to your friend". Text that is not a Ticket, or the player's own Ticket, is refused straight away with a banner that says which.

Scope, from the spec's "HTTP API", "Session model", "Shared status and banners", "Threading rules" and "The page" sections:

- **`POST /api/join`**, body `{ "ticket": "…" }`. The Ticket is trimmed (stray spaces and line breaks are accepted), parsed without I/O, and checked against our own.
  - A bad Ticket gets 400 with the banner code, and sets the banner.
  - A good one starts the dial and returns at once, with a `ticket_no_addresses` warning when the Ticket carries no addresses. The dial is still attempted.
  - The dial's outcome arrives through status.
  - 409 if a dial is already in progress: only one UI-started dial runs at a time.
- **States.** `joining`: a dial started from the UI is in progress. `joined`: our dial succeeded and at least one peer is connected. When the last peer goes away, the state returns to `ready`.
- **`invalid_ticket` banner**, an event banner: set when a join is given text that does not parse as a Ticket, or our own Ticket; cleared by the next join attempt. Text: "That doesn't look like a Ticket. Ask your friend to copy theirs again." For our own Ticket: "That's your own Ticket. Paste your friend's." The status or the join response must let the page tell the two cases apart.
- **Threading rules.** The blocking Transport join runs inside `spawn_blocking`, never directly on an async worker thread. The last reference to a Transport is never dropped on an async worker thread. Release builds abort on panic, so breaking either rule kills the Helper and the game's IPC connection with it.
- **Session controller** owns the join: the CLI `join` and the UI join go through the same code.
- **The page.** Step 2 shows the friend's Ticket box and Connect beside the player's own Ticket. Connect is disabled while the box is empty. In `joining` it shows "Connecting to your friend…"; in `joined`, "Connected to your friend". Step 2 is done when the state is `joined` or `hosting`. In step 3 the menu hint is narrowed to "Join Game" when the state is `joined`. The `ticket_no_addresses` warning is shown to the player. The footer's state words are "Connecting…" and "Joined".

A dial that fails returns the state to `ready` in this ticket; the banners for failed dials are ticket 12.

Tests use the seam 1 harness plus a friend Transport on loopback, and assert on HTTP responses, status fields and what the friend sees.

**Blocked by:** 10 (Step 4: hosting and connected Helpers)

**Status:** ready-for-agent

- [ ] Text that is not a Ticket gets 400 with `invalid_ticket`, and status carries the banner
- [ ] Our own Ticket gets 400 with `invalid_ticket`, distinguishable as the own-Ticket case
- [ ] A Ticket padded with whitespace and a newline is accepted
- [ ] A Ticket with no addresses returns the `ticket_no_addresses` warning and still starts a dial
- [ ] Joining a friend: the state goes through `joining` to `joined`, and the friend appears in the peer list on both sides
- [ ] A second join while `joining` gets 409
- [ ] A new join attempt clears `invalid_ticket`
- [ ] When the friend goes away after `joined`, the state returns to `ready`
- [ ] A join issued from the async handler does not abort the process (the nested-runtime hazard)
- [ ] `POST /api/join` without a token gets 403, and with a foreign `Origin` is rejected
- [ ] `datalink-mp join --ticket` still behaves as before and uses the same controller code
- [ ] The page disables Connect while the box is empty, shows the connecting and connected wording, and narrows the step 3 hint to "Join Game" in `joined`
