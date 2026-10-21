//! Turns for the readers of one connection's ordered streams.
//!
//! Normally a friend opens one ordered stream. When its writer reopens after
//! an error (and replays in-flight frames, see `frame_sink`), a second stream
//! appears while the first one's reader may still be dispatching its tail. The
//! replay must not overtake that tail, so each reader takes a [`Turn`] when its
//! stream is accepted and dispatches nothing until the previous reader's turn
//! is over (the reader finished, which drops its `Turn`).

use std::time::Duration;
use tokio::sync::oneshot;

/// Hands out turns, in the order streams are accepted.
#[derive(Default)]
pub struct StreamTurns {
    last: Option<oneshot::Receiver<()>>,
}

/// One stream reader's turn. Dropping it ends the turn, but not before the
/// previous turn has ended: a reader that finishes without ever waiting (a bad
/// preamble) must not let the one after it overtake the one before it.
pub struct Turn {
    previous: Option<oneshot::Receiver<()>>,
    done: Option<oneshot::Sender<()>>,
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (Some(previous), Some(done)) = (self.previous.take(), self.done.take()) else { return };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = previous.await;
                drop(done);
            });
        }
    }
}

impl StreamTurns {
    pub fn new() -> Self {
        Self::default()
    }

    /// The turn of a stream just accepted, after every earlier one.
    pub fn next(&mut self) -> Turn {
        let (done, ended) = oneshot::channel();
        Turn { previous: self.last.replace(ended), done: Some(done) }
    }
}

impl Turn {
    /// Wait until the previous stream's reader has finished, or `limit` has
    /// passed (a stuck old stream must not stall the game for good). Says
    /// whether the previous reader finished.
    pub async fn wait_for_previous(&mut self, limit: Duration) -> bool {
        let Some(previous) = self.previous.take() else { return true };
        // The sender is only ever dropped, never sent on: `Ok` from the
        // timeout means the previous turn ended.
        tokio::time::timeout(limit, previous).await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::Arc;

    const LONG: Duration = Duration::from_secs(30);

    type Log = Arc<Mutex<Vec<&'static str>>>;

    /// A reader: waits its turn, dispatches `msgs` into `log`, then ends.
    fn reader(mut turn: Turn, log: Log, msgs: &'static [&'static str]) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            turn.wait_for_previous(LONG).await;
            for m in msgs {
                log.lock().push(m);
                tokio::task::yield_now().await;
            }
        })
    }

    async fn let_tasks_run() {
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn a_reopened_stream_dispatches_nothing_until_the_old_reader_has_finished() {
        let mut turns = StreamTurns::new();
        let log: Log = Default::default();
        let old_turn = turns.next();
        let new = reader(turns.next(), log.clone(), &["replay-a", "replay-b"]);
        // The new reader is up, but the old one is still reading.
        let_tasks_run().await;
        assert!(log.lock().is_empty());
        // The old reader dispatches its tail, then finishes.
        log.lock().push("old-tail");
        drop(old_turn);
        new.await.unwrap();
        assert_eq!(*log.lock(), vec!["old-tail", "replay-a", "replay-b"]);
    }

    #[tokio::test]
    async fn turns_are_taken_in_order_down_a_chain() {
        let mut turns = StreamTurns::new();
        let log: Log = Default::default();
        let first = turns.next();
        let second = turns.next();
        let third = reader(turns.next(), log.clone(), &["third"]);
        // The second ends at once; the third must still wait for the first.
        drop(second);
        let_tasks_run().await;
        assert!(log.lock().is_empty());
        drop(first);
        third.await.unwrap();
        assert_eq!(*log.lock(), vec!["third"]);
    }

    #[tokio::test]
    async fn the_first_stream_does_not_wait_and_a_stuck_one_is_given_up_on() {
        let mut turns = StreamTurns::new();
        let mut first = turns.next();
        assert!(first.wait_for_previous(LONG).await);
        let mut second = turns.next();
        assert!(!second.wait_for_previous(Duration::from_millis(20)).await);
    }
}
