# 08: What does the page show while a Turn sync runs?

Type: prototype
Status: resolved
Blocked by: 02, 05

## Question

ADR-0004's step 4 shows activity on the page, such as "Receiving turn data from your friend… 340 KB" or "Waiting for your friend", so a player knows the game is working during a Turn sync. The game's own sync box stays as it is. With [what a Turn sync looks like](05-analyse-the-capture.md) and [whether the game's messages can be read](02-what-is-known-about-jackal-and-turn-sync.md) known, prototype it on the page:

- what shows, where, and when it appears and clears (driven by the analysis's way of finding a Turn sync, so it doesn't flicker between bursts)
- whether it shows bytes, a rate, time elapsed, or only "receiving" against "waiting"
- whether it says whose turn it is: only if the game's messages can be read reliably
- whether the browser tab's title changes too, as it does for a hostable Host

## Answer

2026-10-06. Decided from the prototype on branch `prototype/turn-sync-activity` (`prototype/turn-sync-activity/serve.py`, five variants over a scripted Turn sync on the real `page.html`).

**Priority is low.** The maintainer's view: players are in the game during a Turn sync, and never look at the Helper. So build the decided version below cheaply, and don't let it hold up the speed fixes. Of everything here, the tab title is the part most likely to be seen.

- **No "Waiting for your friend".** In the capture, 67–78% of a Turn sync had nothing in flight because a game was computing, usually the host's own. The page says what's known: "Syncing turn 17", and whose game is working ("Your friend's game is working out the turn", "Your game is working…"), inferred from who sends next.
- **When:** it follows the Helper's Turn sync detection, from `0x8301`/`0x4301` to `0x4309`. Raw bursts never drive the page. It ends with a short "Turn synced" for a few seconds.
- **Numbers:** elapsed seconds always, and bytes only during a resync or the game start. No rate.
- **Whose turn:** "you" or "your friend", never faction names. The wording for a simultaneous-moves game is unproven, since none has been captured.
- **Where:** on the friend's own row in step 4 (prototype variant C), carrying variant D's sentence about whose game is working. No banner. In games of three or more, the activity sits on the row of the friend whose game is working, by name. That row is also where [What does the Helper report…](07-what-the-helper-reports.md) would put a Direct/Relayed badge.
- **Tab title:** it changes only once a sync passes about 10 s, to plain text ("Syncing…"). The page has no title code today; the hostable-Host title is only a parked post-0.1.0 polish ticket, and the two must fit together.

With [the Helper acking for the friend's game](09-helper-acks-for-the-friends-game.md), round trips shrink further, so even more of a Turn sync is a game computing, and "whose game is working" stays the honest message.
