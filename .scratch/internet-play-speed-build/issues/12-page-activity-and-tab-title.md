# 12: Page activity and tab title (low priority)

**What to build:** While a **Turn sync** runs, the friend's row on the page says "Syncing turn N" and whose game is working ("Your friend's game is working out the turn" / "Your game is working…"), inferred from who sends next, with elapsed seconds always and bytes only on a resync or the game start. It follows the shared Turn sync detector, and ends with a brief "Turn synced". Never "Waiting for your friend". In games of three or more, it sits on the row of the friend whose game is working, by name. The tab title changes to plain "Syncing…" only once a sync passes about 10 s, and must fit with the parked hostable-Host title idea. Low priority: players don't look at the Helper mid-game, so build it cheaply. Decided in `.scratch/internet-play-speed/issues/08-activity-on-the-page-during-turn-sync.md`, with a prototype at tag `archive/prototype-turn-sync-activity`.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] The activity text appears and clears with the detector, with no flicker between bursts
- [ ] Elapsed seconds always, bytes only on a resync or the game start
- [ ] The tab title changes only after about 10 s
- [ ] UI tests cover it
