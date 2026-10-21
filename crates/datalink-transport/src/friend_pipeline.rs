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
use crate::turn_sync::{self, Activity, Direction, Event, SyncKind, TurnSyncDetector};
use dp_types::DPID;
use parking_lot::Mutex;
use crate::jackal::Jackal;
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

/// The most reliable messages in flight per friend that get an Early ack.
pub const EARLY_ACK_CAP: usize = 16;

/// An **Early ack** to hand the game: the first 12 bytes of the message it
/// acks with the kind word rewritten to `0x0002`. The caller adds it to the
/// game inbox from the friend's player, to `to` (our player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarlyAck {
    pub to: DPID,
    pub data: [u8; 12],
}

/// What to do with an outbound game message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundOutcome {
    /// Queue it on the friend's outbox.
    Send,
    /// Queue it on the outbox, then add this Early ack to the game inbox.
    SendWithEarlyAck(EarlyAck),
    /// Queue nothing: this connection already carried that seq.
    DropResend,
}

/// What to do with an inbound game message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundOutcome {
    /// Hand it to the game's inbox.
    Deliver,
    /// Deliver nothing: it is the real ack of a message already Early-acked.
    /// A freed slot lets withheld Early acks out: add them to the game inbox.
    Swallow { released: Vec<EarlyAck> },
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

/// The Early ack rules' state for one friend. See ADR-0005 section 2.
#[derive(Default)]
struct EarlyAcks {
    /// Seqs this connection has carried, for dropping resends.
    carried: HashSet<u32>,
    /// Seqs Early-acked, whose real acks (duplicates too) are swallowed.
    acked: HashSet<u32>,
    /// Early-acked seqs still waiting for their real ack (the slots in use).
    in_flight: HashSet<u32>,
    /// Sent, but the cap was full: their Early acks wait for a free slot.
    withheld: VecDeque<(u32, EarlyAck)>,
    /// The connection was lost: no more Early acks this Game session.
    lost: bool,
}

/// How long "Turn synced" stays on the page after a sync ends.
pub const SYNCED_SHOWN_FOR: Duration = Duration::from_secs(4);

/// What the page shows of one friend's syncs now.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncView {
    /// A sync is running.
    Syncing(Activity),
    /// One finished a moment ago.
    Synced { kind: SyncKind, turn: Option<u32> },
}

struct State {
    early: EarlyAcks,
    detector: TurnSyncDetector,
    /// The latest finished sync, for the short "Turn synced".
    finished: Option<(Instant, SyncKind, Option<u32>)>,
    now: Box<dyn Fn() -> Instant + Send>,
    rtt: Box<dyn FnMut() -> Option<Duration> + Send>,
}

/// One friend's pipeline. See the module docs.
pub struct FriendPipeline {
    peer: [u8; 32],
    early_acks: bool,
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
            early_acks: false,
            state: Mutex::new(State {
                early: EarlyAcks::default(),
                detector: TurnSyncDetector::new(),
                finished: None,
                now: Box::new(now),
                rtt: Box::new(rtt),
            }),
        }
    }

    /// Turn Early acks on or off for this pipeline (off unless asked).
    pub fn with_early_acks(mut self, on: bool) -> Self {
        self.early_acks = on;
        self
    }

    /// A game message is being queued for the friend. `two_players` is whether
    /// the Game session holds our player and exactly one remote player.
    pub fn outbound(
        &self,
        from: DPID,
        to: DPID,
        guaranteed: bool,
        data: &[u8],
        two_players: bool,
    ) -> OutboundResult {
        let outcome = if self.early_acks && two_players {
            self.early_outbound(from, data)
        } else {
            OutboundOutcome::Send
        };
        if outcome == OutboundOutcome::DropResend {
            return OutboundResult { outcome, capture_id: None, sync_event: None };
        }
        let capture_id = capture::outbound_game(&self.peer, from, to, guaranteed, data);
        let sync_event = self.observe(Direction::Out, data);
        OutboundResult { outcome, capture_id, sync_event }
    }

    fn early_outbound(&self, from: DPID, data: &[u8]) -> OutboundOutcome {
        let Some(j) = Jackal::decode(data).filter(Jackal::is_exactly_reliable_data) else {
            return OutboundOutcome::Send;
        };
        let Some(head) = data.get(..12) else { return OutboundOutcome::Send };
        let mut s = self.state.lock();
        let e = &mut s.early;
        if e.lost {
            return OutboundOutcome::Send;
        }
        if !e.carried.insert(j.seq) {
            return OutboundOutcome::DropResend;
        }
        let mut ack = EarlyAck { to: from, data: head.try_into().expect("12 bytes") };
        ack.data[0..2].copy_from_slice(&0x0002u16.to_le_bytes());
        if e.in_flight.len() >= EARLY_ACK_CAP {
            e.withheld.push_back((j.seq, ack));
            return OutboundOutcome::Send;
        }
        e.in_flight.insert(j.seq);
        e.acked.insert(j.seq);
        OutboundOutcome::SendWithEarlyAck(ack)
    }

    /// A game message was read from the friend.
    pub fn inbound(&self, from: DPID, to: DPID, guaranteed: bool, data: &[u8]) -> InboundResult {
        let capture_id = capture::inbound(&self.peer, from, to, guaranteed, data);
        let sync_event = self.observe(Direction::In, data);
        let outcome = if self.early_acks { self.early_inbound(data) } else { InboundOutcome::Deliver };
        InboundResult { outcome, capture_id, sync_event }
    }

    fn early_inbound(&self, data: &[u8]) -> InboundOutcome {
        let Some(j) = Jackal::decode(data).filter(Jackal::is_exactly_ack) else {
            return InboundOutcome::Deliver;
        };
        let mut s = self.state.lock();
        let e = &mut s.early;
        if e.acked.contains(&j.seq) {
            // Free the slot (once, for duplicates) and give withheld acks their turn.
            let mut released = Vec::new();
            if e.in_flight.remove(&j.seq) {
                while e.in_flight.len() < EARLY_ACK_CAP {
                    let Some((seq, ack)) = e.withheld.pop_front() else { break };
                    e.in_flight.insert(seq);
                    e.acked.insert(seq);
                    released.push(ack);
                }
            }
            return InboundOutcome::Swallow { released };
        }
        // The real ack of a message whose Early ack was withheld is the only
        // ack the game gets for it; the withheld one is no longer wanted.
        e.withheld.retain(|(seq, _)| *seq != j.seq);
        InboundOutcome::Deliver
    }

    /// A Game session began: seqs restart, so all state is cleared.
    pub fn game_session_began(&self) {
        self.state.lock().early = EarlyAcks::default();
    }

    /// The friend's connection was lost: state is cleared and no Early acks
    /// are made for the rest of the Game session.
    pub fn connection_lost(&self) {
        self.state.lock().early = EarlyAcks { lost: true, ..EarlyAcks::default() };
    }

    /// What the page shows of this friend's syncs: the one running, or the one
    /// that just finished.
    pub fn sync_view(&self) -> Option<SyncView> {
        let s = self.state.lock();
        let now = (s.now)();
        if let Some(a) = s.detector.activity(now) {
            return Some(SyncView::Syncing(a));
        }
        let (at, kind, turn) = s.finished?;
        (now.saturating_duration_since(at) < SYNCED_SHOWN_FOR).then_some(SyncView::Synced { kind, turn })
    }

    fn observe(&self, dir: Direction, data: &[u8]) -> Option<Event> {
        let event = {
            let mut s = self.state.lock();
            let s = &mut *s;
            let now = (s.now)();
            let event = s.detector.observe(now, dir, data, &mut s.rtt);
            if let Some(Event::Finished(r)) = &event {
                s.finished = Some((now, r.kind, r.turn));
            }
            event
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
                Direction::Out => pipe.outbound(1, 2, true, &p, false).sync_event,
                Direction::In => pipe.inbound(2, 1, true, &p).sync_event,
            };
            events.extend(e);
        }
        events
    }

    #[test]
    fn the_view_follows_the_sync_then_says_synced_for_a_few_seconds() {
        let base = Instant::now();
        let clock = Arc::new(Mutex::new(base));
        let c = clock.clone();
        let pipe = FriendPipeline::new([1; 32], move || *c.lock(), || None);
        let msg = |seq: u8, ty: u8| {
            let mut v = vec![0u8; 44];
            v[0] = 4;
            v[4] = seq;
            v[8] = ty;
            v[9] = 0x43;
            v
        };
        assert_eq!(pipe.sync_view(), None);
        pipe.inbound(2, 1, true, &msg(1, 0x01));
        assert!(matches!(pipe.sync_view(), Some(SyncView::Syncing(_))));
        *clock.lock() = base + Duration::from_secs(5);
        assert!(matches!(pipe.sync_view(), Some(SyncView::Syncing(a)) if a.elapsed == Duration::from_secs(5)));
        pipe.inbound(2, 1, true, &msg(2, 0x09));
        assert!(matches!(pipe.sync_view(), Some(SyncView::Synced { kind: SyncKind::Turn, .. })));
        *clock.lock() = base + Duration::from_secs(5) + SYNCED_SHOWN_FOR;
        assert_eq!(pipe.sync_view(), None);
    }

    #[test]
    fn outcomes_are_send_and_deliver() {
        let pipe = FriendPipeline::new([1; 32], Instant::now, || None);
        let data = [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(pipe.outbound(1, 2, true, &data, false).outcome, OutboundOutcome::Send);
        assert_eq!(pipe.inbound(2, 1, true, &data).outcome, InboundOutcome::Deliver);
        // Lifecycle events don't change the outcomes.
        pipe.game_session_began();
        pipe.connection_lost();
        assert_eq!(pipe.outbound(1, 2, true, &data, false).outcome, OutboundOutcome::Send);
    }

    #[test]
    fn capture_ids_pass_through() {
        // No capture runs in unit tests, so there is no id to pass through.
        let pipe = FriendPipeline::new([1; 32], Instant::now, || None);
        assert_eq!(pipe.outbound(1, 2, true, &[], false).capture_id, None);
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
        assert_eq!(a.outbound(1, 2, true, &start, false).sync_event, started);
        assert_eq!(b.outbound(1, 2, true, &start, false).sync_event, started);
    }
}

#[cfg(test)]
mod early_ack_tests {
    use super::*;

    fn msg(kind: u16, seq: u32) -> Vec<u8> {
        let mut d = vec![0u8; 24];
        d[0..2].copy_from_slice(&kind.to_le_bytes());
        d[4..8].copy_from_slice(&seq.to_le_bytes());
        d[8] = 0xab;
        d[20] = 0xcd;
        d
    }

    fn pipe() -> FriendPipeline {
        FriendPipeline::new([1; 32], Instant::now, || None).with_early_acks(true)
    }

    fn send(p: &FriendPipeline, kind: u16, seq: u32) -> OutboundOutcome {
        p.outbound(1, 2, true, &msg(kind, seq), true).outcome
    }

    fn ack_of(seq: u32) -> EarlyAck {
        let mut data = [0u8; 12];
        data[0] = 2;
        data[4..8].copy_from_slice(&seq.to_le_bytes());
        data[8] = 0xab;
        EarlyAck { to: 1, data }
    }

    #[test]
    fn a_reliable_message_gets_an_early_ack_of_its_first_12_bytes_with_kind_2() {
        let p = pipe();
        assert_eq!(send(&p, 4, 7), OutboundOutcome::SendWithEarlyAck(ack_of(7)));
    }

    #[test]
    fn switch_off_or_not_two_players_changes_nothing() {
        let off = FriendPipeline::new([1; 32], Instant::now, || None);
        assert_eq!(send(&off, 4, 1), OutboundOutcome::Send);
        assert_eq!(send(&off, 4, 1), OutboundOutcome::Send, "resends are not dropped");
        assert_eq!(off.inbound(2, 1, true, &msg(2, 1)).outcome, InboundOutcome::Deliver);
        let three = pipe();
        assert_eq!(three.outbound(1, 2, true, &msg(4, 1), false).outcome, OutboundOutcome::Send);
        assert_eq!(three.outbound(1, 2, true, &msg(4, 1), false).outcome, OutboundOutcome::Send);
    }

    #[test]
    fn setup_kinds_and_other_kinds_are_left_alone() {
        let p = pipe();
        for kind in [0x14, 0x12, 0x06, 0x02, 0x00] {
            assert_eq!(send(&p, kind, 1), OutboundOutcome::Send, "kind {kind:#x}");
            assert_eq!(p.inbound(2, 1, true, &msg(kind, 1)).outcome, InboundOutcome::Deliver);
        }
        assert_eq!(p.outbound(1, 2, true, &[4, 0, 0, 0, 1], true).outcome, OutboundOutcome::Send);
    }

    #[test]
    fn the_real_ack_is_swallowed_and_so_are_duplicates() {
        let p = pipe();
        send(&p, 4, 5);
        let swallow = InboundOutcome::Swallow { released: vec![] };
        assert_eq!(p.inbound(2, 1, true, &msg(2, 5)).outcome, swallow);
        assert_eq!(p.inbound(2, 1, true, &msg(2, 5)).outcome, swallow);
        // An ack for a seq never early-acked, and a setup ack, pass through.
        assert_eq!(p.inbound(2, 1, true, &msg(2, 6)).outcome, InboundOutcome::Deliver);
        assert_eq!(p.inbound(2, 1, true, &msg(0x12, 5)).outcome, InboundOutcome::Deliver);
    }

    #[test]
    fn a_resend_is_dropped() {
        let p = pipe();
        assert!(matches!(send(&p, 4, 3), OutboundOutcome::SendWithEarlyAck(_)));
        assert_eq!(send(&p, 4, 3), OutboundOutcome::DropResend);
        // Even after its real ack: the connection carried it.
        p.inbound(2, 1, true, &msg(2, 3));
        assert_eq!(send(&p, 4, 3), OutboundOutcome::DropResend);
    }

    #[test]
    fn the_17th_early_ack_is_withheld_until_a_real_ack_frees_a_slot() {
        let p = pipe();
        for seq in 1..=16 {
            assert!(matches!(send(&p, 4, seq), OutboundOutcome::SendWithEarlyAck(_)));
        }
        assert_eq!(send(&p, 4, 17), OutboundOutcome::Send);
        assert_eq!(send(&p, 4, 18), OutboundOutcome::Send);
        let r = p.inbound(2, 1, true, &msg(2, 1)).outcome;
        assert_eq!(r, InboundOutcome::Swallow { released: vec![ack_of(17)] });
        // A duplicate of that real ack frees nothing more.
        let r = p.inbound(2, 1, true, &msg(2, 1)).outcome;
        assert_eq!(r, InboundOutcome::Swallow { released: vec![] });
        let r = p.inbound(2, 1, true, &msg(2, 2)).outcome;
        assert_eq!(r, InboundOutcome::Swallow { released: vec![ack_of(18)] });
    }

    #[test]
    fn the_real_ack_of_a_withheld_message_is_delivered_and_its_early_ack_never_comes() {
        let p = pipe();
        for seq in 1..=17 {
            send(&p, 4, seq);
        }
        assert_eq!(p.inbound(2, 1, true, &msg(2, 17)).outcome, InboundOutcome::Deliver);
        let r = p.inbound(2, 1, true, &msg(2, 1)).outcome;
        assert_eq!(r, InboundOutcome::Swallow { released: vec![] });
    }

    #[test]
    fn a_game_session_clears_everything() {
        let p = pipe();
        send(&p, 4, 1);
        p.game_session_began();
        assert!(matches!(send(&p, 4, 1), OutboundOutcome::SendWithEarlyAck(_)));
        p.game_session_began();
        assert_eq!(p.inbound(2, 1, true, &msg(2, 1)).outcome, InboundOutcome::Deliver);
    }

    #[test]
    fn after_a_lost_connection_no_early_acks_until_the_next_game_session() {
        let p = pipe();
        send(&p, 4, 1);
        p.connection_lost();
        assert_eq!(send(&p, 4, 2), OutboundOutcome::Send);
        assert_eq!(send(&p, 4, 1), OutboundOutcome::Send);
        assert_eq!(p.inbound(2, 1, true, &msg(2, 1)).outcome, InboundOutcome::Deliver);
        p.game_session_began();
        assert!(matches!(send(&p, 4, 2), OutboundOutcome::SendWithEarlyAck(_)));
    }
}
