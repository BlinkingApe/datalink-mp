# 08: What does the page show while a Turn sync runs?

Type: prototype
Status: needs-triage
Blocked by: 02, 05

## Question

ADR-0004's step 4 shows activity on the page, such as "Receiving turn data from your friend… 340 KB" or "Waiting for your friend", so a player knows the game is working during a Turn sync. The game's own sync box stays as it is. With [what a Turn sync looks like](05-analyse-the-capture.md) and [whether the game's messages can be read](02-what-is-known-about-jackal-and-turn-sync.md) known, prototype it on the page:

- what shows, where, and when it appears and clears (driven by the analysis's way of finding a Turn sync, so it doesn't flicker between bursts)
- whether it shows bytes, a rate, time elapsed, or only "receiving" against "waiting"
- whether it says whose turn it is: only if the game's messages can be read reliably
- whether the browser tab's title changes too, as it does for a hostable Host
