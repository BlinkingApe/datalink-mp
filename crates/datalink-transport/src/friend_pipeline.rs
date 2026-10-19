//! The **friend pipeline**: everything the Transport does with the game
//! messages that cross one friend's connection.
//!
//! One pipeline is held on each link in the friend registry (one link per
//! connection to a friend), so it is created and dropped with that link. The
//! registry calls it and carries out its outcomes. It sees each outbound
//! game message as it is queued, each inbound one as it is read, and the
//! lifecycle events (a Game session began, the connection was lost). Behind it
//! sit the friend's Turn sync detector and the capture lines for game messages,
//! written in one fixed order: the capture line first, then the detector.
//!
//! The pipeline only says what should happen. Its caller acts on the result
//! (queues on the outbox, adds to the inbox) after the call returns, so the
//! pipeline never takes another module's lock. Time and RTT are injected.

use crate::capture;
use crate::turn_sync::{self, Direction, Event, TurnSyncDetector};
use dp_types::DPID;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

/// What to do with an outbound game message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundOutcome {
    /// Queue it on the friend's outbox.
    Send,
}

/// What to do with an inbound game message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundOutcome {
    /// Hand it to the game's inbox.
    Deliver,
}

/// The pipeline's answer for an outbound game message.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboundResult {
    pub outcome: OutboundOutcome,
    /// The message's id in the traffic capture, if one is running.
    pub capture_id: Option<u64>,
    /// The Turn sync event this message caused, if any (already logged).
    pub sync_event: Option<Event>,
}

/// The pipeline's answer for an inbound game message.
#[derive(Debug, Clone, PartialEq)]
pub struct InboundResult {
    pub outcome: InboundOutcome,
    /// The message's id in the traffic capture, if one is running.
    pub capture_id: Option<u64>,
    /// The Turn sync event this message caused, if any (already logged).
    pub sync_event: Option<Event>,
}

struct State {
    detector: TurnSyncDetector,
    now: Box<dyn Fn() -> Instant + Send>,
    rtt: Box<dyn FnMut() -> Option<Duration> + Send>,
}

/// One friend's pipeline. See the module docs.
pub struct FriendPipeline {
    peer: [u8; 32],
    state: Mutex<State>,
}

impl std::fmt::Debug for FriendPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FriendPipeline").finish_non_exhaustive()
    }
}

impl FriendPipeline {
    /// A pipeline for `peer`. `now` gives the time a message is seen; `rtt` the
    /// friend's connection's current round-trip time.
    pub fn new(
        peer: [u8; 32],
        now: impl Fn() -> Instant + Send + 'static,
        rtt: impl FnMut() -> Option<Duration> + Send + 'static,
    ) -> Self {
        FriendPipeline {
            peer,
            state: Mutex::new(State {
                detector: TurnSyncDetector::new(),
                now: Box::new(now),
                rtt: Box::new(rtt),
            }),
        }
    }

    /// A game message is being queued for the friend.
    pub fn outbound(&self, from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> OutboundResult {
        let capture_id = capture::outbound_game(&self.peer, from, to, guaranteed, data);
        let sync_event = self.observe(Direction::Out, data);
        OutboundResult { outcome: OutboundOutcome::Send, capture_id, sync_event }
    }

    /// A game message was read from the friend.
    pub fn inbound(&self, from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> InboundResult {
        let capture_id = capture::inbound(&self.peer, from, to, guaranteed, data);
        let sync_event = self.observe(Direction::In, data);
        InboundResult { outcome: InboundOutcome::Deliver, capture_id, sync_event }
    }

    /// A Game session began. Nothing in the pipeline depends on it yet.
    pub fn game_session_began(&self) {}

    /// The friend's connection was lost. Nothing in the pipeline depends on it yet.
    pub fn connection_lost(&self) {}

    fn observe(&self, dir: Direction, data: &[u8]) -> Option<Event> {
        let event = {
            let mut s = self.state.lock();
            let s = &mut *s;
            let now = (s.now)();
            s.detector.observe(now, dir, data, &mut s.rtt)
        };
        if let Some(Event::Finished(report)) = &event {
            turn_sync::log_report(&self.peer, report);
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const TURN: &str = include_str!("../tests/fixtures/turn_sync_turn.txt");
    const START: &str = include_str!("../tests/fixtures/turn_sync_game_start.txt");

    /// `<t_us> <out|in> <size> <first bytes, hex>` rows.
    fn rows(text: &str) -> Vec<(u64, Direction, Vec<u8>)> {
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let mut f = l.split(' ');
                let t: u64 = f.next().unwrap().parse().unwrap();
                let dir = if f.next().unwrap() == "out" { Direction::Out } else { Direction::In };
                let size: usize = f.next().unwrap().parse().unwrap();
                let mut p = data_encoding::HEXLOWER.decode(f.next().unwrap().as_bytes()).unwrap();
                p.resize(size, 0);
                (t, dir, p)
            })
            .collect()
    }

    fn through_detector(text: &str) -> Vec<Event> {
        let base = Instant::now();
        let mut d = TurnSyncDetector::new();
        let mut rtt = || Some(Duration::from_millis(60));
        rows(text)
            .into_iter()
            .filter_map(|(t, dir, p)| d.observe(base + Duration::from_micros(t), dir, &p, &mut rtt))
            .collect()
    }

    /// Replay a fixture through a pipeline whose clock steps with the rows.
    fn through_pipeline(text: &str) -> Vec<Event> {
        let base = Instant::now();
        let clock = Arc::new(Mutex::new(base));
        let c = clock.clone();
        let pipe = FriendPipeline::new([1; 32], move || *c.lock(), || Some(Duration::from_millis(60)));
        let mut events = Vec::new();
        for (t, dir, p) in rows(text) {
            *clock.lock() = base + Duration::from_micros(t);
            let e = match dir {
                Direction::Out => pipe.outbound(1, 2, true, &p).sync_event,
                Direction::In => pipe.inbound(2, 1, true, &p).sync_event,
            };
            events.extend(e);
        }
        events
    }

    #[test]
    fn outcomes_are_send_and_deliver() {
        let pipe = FriendPipeline::new([1; 32], Instant::now, || None);
        let data = [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(pipe.outbound(1, 2, true, &data).outcome, OutboundOutcome::Send);
        assert_eq!(pipe.inbound(2, 1, true, &data).outcome, InboundOutcome::Deliver);
        // Lifecycle events don't change the outcomes.
        pipe.game_session_began();
        pipe.connection_lost();
        assert_eq!(pipe.outbound(1, 2, true, &data).outcome, OutboundOutcome::Send);
    }

    #[test]
    fn capture_ids_pass_through() {
        // No capture runs in unit tests, so there is no id to pass through.
        let pipe = FriendPipeline::new([1; 32], Instant::now, || None);
        assert_eq!(pipe.outbound(1, 2, true, &[]).capture_id, None);
        assert_eq!(pipe.inbound(2, 1, true, &[]).capture_id, None);
    }

    #[test]
    fn fixtures_give_the_same_reports_as_the_detector() {
        for fixture in [TURN, START] {
            let direct = through_detector(fixture);
            assert!(!direct.is_empty());
            assert_eq!(through_pipeline(fixture), direct);
        }
    }

    #[test]
    fn two_pipelines_for_one_friend_keep_separate_state() {
        let base = Instant::now();
        let a = FriendPipeline::new([9; 32], move || base, || None);
        let b = FriendPipeline::new([9; 32], move || base, || None);
        // A turn-start message (0x8301) begins a sync; with shared state the
        // second pipeline would see it as a resent copy and start nothing.
        let mut start = vec![4, 0, 0, 0, 1, 0, 0, 0, 0x01, 0x83, 0, 0, 0, 0, 0, 0];
        start.resize(32, 0);
        let started = Some(Event::Started(turn_sync::SyncKind::Turn));
        assert_eq!(a.outbound(1, 2, true, &start).sync_event, started);
        assert_eq!(b.outbound(1, 2, true, &start).sync_event, started);
    }
}
