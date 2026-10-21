# 12: Page activity and tab title (low priority)

**What to build:** While a **Turn sync** runs, the friend's row on the page says "Syncing turn N" and whose game is working ("Your friend's game is working out the turn" / "Your game is working…"), inferred from who sends next, with elapsed seconds always and bytes only on a resync or the game start. It follows the shared Turn sync detector, and ends with a brief "Turn synced". Never "Waiting for your friend". In games of three or more, it sits on the row of the friend whose game is working, by name. The tab title changes to plain "Syncing…" only once a sync passes about 10 s, and must fit with the parked hostable-Host title idea. Low priority: players don't look at the Helper mid-game, so build it cheaply. Decided in `.scratch/internet-play-speed/issues/08-activity-on-the-page-during-turn-sync.md`, with a prototype at tag `archive/prototype-turn-sync-activity`.

**Blocked by:** 05

**Status:** resolved

- [x] The activity text appears and clears with the detector, with no flicker between bursts
- [x] Elapsed seconds always, bytes only on a resync or the game start
- [x] The tab title changes only after about 10 s
- [x] UI tests cover it

## Comments

Built on the shared detector: `TurnSyncDetector::activity` (turn_sync.rs) gives kind, turn, elapsed, bytes (game start or a phase-7 resync only) and whose game is working (who sent last); `FriendPipeline::sync_view` adds the 4 s "synced" after a sync ends. Status carries `sync_activity` by short ID; page.html words it after the friend on the Helpers line and sets the tab title to "Syncing\u2026" only once a sync passes 10 s. The page decides the wording; the 10 s rule and the words are checked by page-content tests, the status data by a test that drives a real friend. Not run by hand in a browser. `cargo clippy --workspace` fails on warnings that were already there (datalink-fixes, session.rs).
