# UI flow: Host/Join toggle or one screen?

Type: prototype
Status: resolved
Blocked by: 01 (resolved)

## Question

How should the single page look and behave? Two candidate shapes:

- A **Host / Join toggle**, as in ADR-0001.
- **One screen**: "your Ticket" always shown with Copy, plus "paste a friend's Ticket → Connect". Because the Helper's host and join differ only in whether it dials a peer (the game itself does Create/Join session over IPC), this may map better onto what is really happening.

The prototype must also place the status panel, the five error banners, the setup self-check result, Linux override copy-strings, Stop, and Quit. It should also make clear what the user does next *in the game* (Multiplayer → Iroh P2P → Host/Join).

## Prototype

Branch `prototype/ui-flow` (commit `6399164`), file `crates/smac-helper/prototype/ui-flow-prototype.html`. Open it in a browser; `?variant=A|B|C` or the bottom bar / arrow keys switch variants, and the Scenario panel forces OS, self-check, session, DLL, peers and each banner.

- **A, Host / Join toggle** (the ADR draft): tabs, "Start hosting" reveals the Ticket.
- **B, One screen**: sticky status bar (state, game connected, peers, Stop, Quit); your Ticket always shown; "paste a friend's Ticket → Connect"; an "in the game" card.
- **C, Numbered steps**: 1 game folder → 2 share or paste a Ticket → 3 start the game (Multiplayer → Iroh P2P) → 4 play, with steps ticking off from live status.

## Answer

**Variant C, numbered steps, as is.** One page with four steps that tick off from live status:

1. **Game folder**: the self-check result. On Linux/macOS the Wine override copy-strings sit inside this step; if the check fails, the "not your game folder" banner shows here instead.
2. **Share or paste a Ticket**: your Ticket (always shown, Copy) side by side with the friend's Ticket box and Connect. Once you're connected, this step just shows "Connected to your friend".
3. **Start the game**: "In the game: Multiplayer → Iroh P2P → Host Game / Join Game", with the game-connected pill.
4. **Play**: the peer count with short IDs, and the per-OS "keep this open" line.

Placement: Stop (only while in a session) and Quit in the header; the five error banners go under the header; version (Release, with IPC and Peer protocol small print) and state in the footer line.

Consequences for the ADR revision:
- No Host/Join toggle and no "Start hosting" action. The Ticket exists from startup, so `POST /api/host` goes; hosting is simply "a peer dialled us".
- Stop gives a new Ticket (fresh Transport); the "can't reach host" banner tells the joiner to ask for the new one.

Decided by the user on 2026-09-30 after reviewing all three variants. The prototype stays on branch `prototype/ui-flow` (`6399164`) as the reference for `/to-spec`.
