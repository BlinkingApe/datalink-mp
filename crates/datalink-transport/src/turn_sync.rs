//! Finds a **Turn sync** and the game start from the game messages that cross
//! one friend's connection, and logs a line for each one that finishes.
//!
//! The detector ([`TurnSyncDetector`]) is pure: it is fed each game message with
//! the time it was seen, and says when a sync starts and ends. The page's
//! activity text is meant to use the same detector, so the two can't disagree.
//! The friend pipeline feeds it from the Helper's message path; this file also
//! logs the figures (raw, no payloads).
//!
//! The rules come from `.scratch/internet-play-speed/analysis/direct-game-capture.md`:
//!
//! - A **Turn sync** starts at the first `0x8301` or `0x4301` (the last mover's
//!   end of turn and the host's answer) and ends at the next `0x4309`. A
//!   `0x4309` outside a sync only hands the turn from one player to the next,
//!   and is ignored.
//! - The **game start** starts at the first `0x4101` (the initial state sync)
//!   after the lobby, and ends at the first `0x4309`. The lobby's setup
//!   messages (`0x4f02`, `0x4f03`, `0x4f04`, `0x0f0d`, `0x2f04`) arm it again.
//!   **[inference]** from one captured game; a game started by other means may
//!   not match.
//! - The **turn number** is the one the sync's barriers carry (`0x2303`,
//!   `0x4303`, `0x2305`, `0x4305`, at wire offset 28).
//! - **Nothing in flight** means no data message is waiting for its ack, in
//!   either direction. Resent copies (the game's JACKAL layer resends on a
//!   timer) are counted as messages and bytes but change nothing else.

use crate::capture;
use crate::jackal::Jackal;
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};
use tracing::info;

const START_TURN_OUT: u16 = 0x8301;
const START_TURN_IN: u16 = 0x4301;
const END: u16 = 0x4309;
const STATE_SYNC: u16 = 0x4101;
const BARRIERS: [u16; 4] = [0x2303, 0x4303, 0x2305, 0x4305];
const LOBBY: [u16; 5] = [0x4f02, 0x4f03, 0x4f04, 0x0f0d, 0x2f04];

/// How many recent sequence numbers per direction are remembered to tell a
/// resent copy from a new message.
const RECENT_SEQS: usize = 128;

/// Least time between two RTT samples during a sync.
const RTT_SAMPLE_EVERY: Duration = Duration::from_millis(250);

/// Which way a game message went, from this Helper's view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Queued for the friend.
    Out,
    /// Read from the friend.
    In,
}

impl Direction {
    fn other(self) -> Self {
        match self {
            Direction::Out => Direction::In,
            Direction::In => Direction::Out,
        }
    }
}

/// What kind of sync a [`Report`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncKind {
    GameStart,
    Turn,
}

/// The raw figures of one finished sync, for one friend.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub kind: SyncKind,
    /// The turn the sync's barriers carried, if any were seen.
    pub turn: Option<u32>,
    pub duration: Duration,
    pub out_msgs: u32,
    pub out_bytes: u64,
    pub in_msgs: u32,
    pub in_bytes: u64,
    /// Total time with no data message waiting for its ack.
    pub idle: Duration,
    /// The median of the RTT samples taken during the sync.
    pub rtt_median: Option<Duration>,
    /// How many times the traffic changed from our message to the friend's,
    /// and the time those changes took in all: from the last message we queued
    /// to the friend's next one read. The Helper sees "queued", not "written";
    /// the capture analysis uses the written time for the exact figure.
    pub hops: u32,
    pub hop_total: Duration,
}

/// How many Early acks the pipeline made, withheld and swallowed (the real
/// acks it kept from the game), over a sync or since the pipeline began.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EarlyAckCounts {
    /// Early acks given to the game.
    pub made: u64,
    /// Early acks held back because 16 were already in flight.
    pub withheld: u64,
    /// Real acks (duplicates too) not given to the game.
    pub swallowed: u64,
}

/// One reading of the friend's connection's selected path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// UDP datagrams sent on the path so far.
    pub datagrams_sent: u64,
    /// Packets the path has lost so far.
    pub lost_packets: u64,
    /// Congestion events on the path so far.
    pub congestion_events: u64,
    /// The congestion window now, in bytes.
    pub cwnd: u64,
}

/// What the link did over one sync: the counters' growth and the median
/// congestion window of the readings taken during it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkFigures {
    pub datagrams_sent: u64,
    pub lost_packets: u64,
    pub congestion_events: u64,
    pub cwnd_median: u64,
}

impl LinkFigures {
    /// The figures between the `start` and `end` readings. `cwnds` holds the
    /// congestion windows read during the sync (the end's included).
    pub fn between(start: LinkStats, end: LinkStats, cwnds: &mut [u64]) -> Self {
        cwnds.sort_unstable();
        let cwnd_median = match cwnds.len() {
            0 => end.cwnd,
            n if n % 2 == 1 => cwnds[n / 2],
            n => (cwnds[n / 2 - 1] + cwnds[n / 2]) / 2,
        };
        LinkFigures {
            datagrams_sent: end.datagrams_sent.saturating_sub(start.datagrams_sent),
            lost_packets: end.lost_packets.saturating_sub(start.lost_packets),
            congestion_events: end.congestion_events.saturating_sub(start.congestion_events),
            cwnd_median,
        }
    }
}

impl EarlyAckCounts {
    /// The counts since `earlier`.
    pub fn since(self, earlier: EarlyAckCounts) -> EarlyAckCounts {
        EarlyAckCounts {
            made: self.made.saturating_sub(earlier.made),
            withheld: self.withheld.saturating_sub(earlier.withheld),
            swallowed: self.swallowed.saturating_sub(earlier.swallowed),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Started(SyncKind),
    Finished(Report),
}

#[derive(Debug)]
struct Active {
    kind: SyncKind,
    started: Instant,
    last: Instant,
    turn: Option<u32>,
    out_msgs: u32,
    out_bytes: u64,
    in_msgs: u32,
    in_bytes: u64,
    idle: Duration,
    waiting: HashSet<(Direction, u32)>,
    rtts: Vec<Duration>,
    last_sample: Option<Instant>,
    /// The direction of the last message, and when we last queued one.
    last_dir: Option<Direction>,
    last_out: Option<Instant>,
    hops: u32,
    hop_total: Duration,
}

/// Finds Turn syncs and the game start in one friend's game messages.
#[derive(Debug)]
pub struct TurnSyncDetector {
    active: Option<Active>,
    /// Whether the next `0x4101` is a game start.
    game_start_armed: bool,
    recent: [VecDeque<u32>; 2],
}

impl Default for TurnSyncDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnSyncDetector {
    pub fn new() -> Self {
        TurnSyncDetector {
            active: None,
            game_start_armed: true,
            recent: [VecDeque::new(), VecDeque::new()],
        }
    }

    /// Whether a sync is running.
    pub fn in_sync(&self) -> bool {
        self.active.is_some()
    }

    /// The kind of the sync running now.
    pub fn current(&self) -> Option<SyncKind> {
        self.active.as_ref().map(|a| a.kind)
    }

    /// Feed one game message, as seen at `now`. `rtt` is asked for the
    /// connection's current RTT when the detector wants a sample (at a sync's
    /// start and end, and at most every 250 ms between).
    pub fn observe(
        &mut self,
        now: Instant,
        dir: Direction,
        data: &[u8],
        rtt: &mut dyn FnMut() -> Option<Duration>,
    ) -> Option<Event> {
        let Some(header) = Jackal::decode(data) else {
            self.account(now, dir, data.len());
            return None;
        };
        let seq = header.seq;
        let is_data = header.carries_reliable_data();
        let is_ack = header.carries_ack();

        let fresh = if is_data { self.first_copy(dir, seq) } else { false };
        let game_type = header.game_type;

        // Time since the last message counts as idle if nothing was waiting.
        self.account(now, dir, data.len());

        let mut event = None;
        if let (true, Some(ty)) = (fresh, game_type) {
            if self.active.is_none() {
                event = self.maybe_start(now, ty);
                if event.is_some() {
                    // The message that starts a sync is its first.
                    self.account(now, dir, data.len());
                }
            } else {
                if BARRIERS.contains(&ty) {
                    if let Some(a) = self.active.as_mut() {
                        if a.turn.is_none() {
                            a.turn = data.get(28..32).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                        }
                    }
                }
                if ty == END {
                    event = Some(Event::Finished(self.finish(now, rtt)));
                }
            }
            if self.active.is_none() && LOBBY.contains(&ty) {
                self.game_start_armed = true;
            }
        }

        if let Some(a) = self.active.as_mut() {
            if is_data {
                a.waiting.insert((dir, seq));
            } else if is_ack {
                a.waiting.remove(&(dir.other(), seq));
            }
            if matches!(event, None | Some(Event::Started(_)))
                && a.last_sample.is_none_or(|t| now.duration_since(t) >= RTT_SAMPLE_EVERY)
            {
                a.last_sample = Some(now);
                a.rtts.extend(rtt());
            }
        }
        event
    }

    fn first_copy(&mut self, dir: Direction, seq: u32) -> bool {
        let recent = &mut self.recent[dir as usize];
        if recent.contains(&seq) {
            return false;
        }
        if recent.len() == RECENT_SEQS {
            recent.pop_front();
        }
        recent.push_back(seq);
        true
    }

    /// Add this message to the running sync's counts and idle time.
    fn account(&mut self, now: Instant, dir: Direction, len: usize) {
        if let Some(a) = self.active.as_mut() {
            if a.waiting.is_empty() {
                a.idle += now.saturating_duration_since(a.last);
            }
            a.last = now;
            if dir == Direction::In && a.last_dir == Some(Direction::Out) {
                if let Some(out) = a.last_out {
                    a.hops += 1;
                    a.hop_total += now.saturating_duration_since(out);
                }
            }
            if dir == Direction::Out {
                a.last_out = Some(now);
            }
            a.last_dir = Some(dir);
            match dir {
                Direction::Out => {
                    a.out_msgs += 1;
                    a.out_bytes += len as u64;
                }
                Direction::In => {
                    a.in_msgs += 1;
                    a.in_bytes += len as u64;
                }
            }
        }
    }

    fn maybe_start(&mut self, now: Instant, ty: u16) -> Option<Event> {
        let kind = if ty == START_TURN_OUT || ty == START_TURN_IN {
            SyncKind::Turn
        } else if ty == STATE_SYNC && self.game_start_armed {
            SyncKind::GameStart
        } else {
            return None;
        };
        self.game_start_armed = false;
        self.active = Some(Active {
            kind,
            started: now,
            last: now,
            turn: None,
            out_msgs: 0,
            out_bytes: 0,
            in_msgs: 0,
            in_bytes: 0,
            idle: Duration::ZERO,
            waiting: HashSet::new(),
            rtts: Vec::new(),
            last_sample: None,
            last_dir: None,
            last_out: None,
            hops: 0,
            hop_total: Duration::ZERO,
        });
        Some(Event::Started(kind))
    }

    fn finish(&mut self, now: Instant, rtt: &mut dyn FnMut() -> Option<Duration>) -> Report {
        let mut a = self.active.take().expect("finish needs a running sync");
        a.rtts.extend(rtt());
        a.rtts.sort();
        let rtt_median = match a.rtts.len() {
            0 => None,
            n if n % 2 == 1 => Some(a.rtts[n / 2]),
            n => Some((a.rtts[n / 2 - 1] + a.rtts[n / 2]) / 2),
        };
        Report {
            kind: a.kind,
            turn: a.turn,
            duration: now.saturating_duration_since(a.started),
            out_msgs: a.out_msgs,
            out_bytes: a.out_bytes,
            in_msgs: a.in_msgs,
            in_bytes: a.in_bytes,
            idle: a.idle,
            rtt_median,
            hops: a.hops,
            hop_total: a.hop_total,
        }
    }
}

// ---------------------------------------------------------------------------
// Logging: one line for each finished sync, called by the friend pipeline.

fn ms(d: Duration) -> u64 {
    d.as_millis() as u64
}

pub(crate) fn log_report(peer: &[u8; 32], r: &Report, early: EarlyAckCounts, link: Option<LinkFigures>) {
    let what = match r.kind {
        SyncKind::GameStart => "game start",
        SyncKind::Turn => "turn sync",
    };
    let peer = format!("{:08x}", u32::from_be_bytes([peer[0], peer[1], peer[2], peer[3]]));
    let turn = r.turn.map(i64::from).unwrap_or(-1);
    let rtt = r.rtt_median.map(ms).map(|v| v as i64).unwrap_or(-1);
    // -1 when the connection's stats couldn't be read.
    let fig = |f: fn(&LinkFigures) -> u64| link.as_ref().map(|l| f(l) as i64).unwrap_or(-1);
    let (datagrams, lost, congestion, cwnd) = (
        fig(|l| l.datagrams_sent),
        fig(|l| l.lost_packets),
        fig(|l| l.congestion_events),
        fig(|l| l.cwnd_median),
    );
    match capture::path() {
        Some(file) => info!(
            peer = %peer, turn, duration_ms = ms(r.duration),
            out_msgs = r.out_msgs, out_bytes = r.out_bytes, in_msgs = r.in_msgs, in_bytes = r.in_bytes,
            idle_ms = ms(r.idle), rtt_median_ms = rtt, hops = r.hops, hop_ms = ms(r.hop_total),
            early_acks_made = early.made, early_acks_withheld = early.withheld, early_acks_swallowed = early.swallowed,
            datagrams_sent = datagrams, packets_lost = lost, congestion_events = congestion, cwnd_median = cwnd,
            capture = %file.display(),
            "{what} finished"
        ),
        None => info!(
            peer = %peer, turn, duration_ms = ms(r.duration),
            out_msgs = r.out_msgs, out_bytes = r.out_bytes, in_msgs = r.in_msgs, in_bytes = r.in_bytes,
            idle_ms = ms(r.idle), rtt_median_ms = rtt, hops = r.hops, hop_ms = ms(r.hop_total),
            early_acks_made = early.made, early_acks_withheld = early.withheld, early_acks_swallowed = early.swallowed,
            datagrams_sent = datagrams, packets_lost = lost, congestion_events = congestion, cwnd_median = cwnd,
            "{what} finished"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A message as the fixtures hold it: `<t_us> <out|in> <size> <first bytes, hex>`.
    struct Row {
        t_us: u64,
        dir: Direction,
        payload: Vec<u8>,
    }

    fn rows(text: &str) -> Vec<Row> {
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let mut f = l.split(' ');
                let t_us = f.next().unwrap().parse().unwrap();
                let dir = if f.next().unwrap() == "out" { Direction::Out } else { Direction::In };
                let size: usize = f.next().unwrap().parse().unwrap();
                let mut payload = data_encoding::HEXLOWER.decode(f.next().unwrap().as_bytes()).unwrap();
                payload.resize(size, 0);
                Row { t_us, dir, payload }
            })
            .collect()
    }

    /// Run a fixture through the detector; the RTT is `rtt_ms` plus the number of
    /// samples taken so far, so the median can be checked.
    fn run(text: &str) -> (Vec<(u64, Event)>, usize) {
        let base = Instant::now();
        let mut d = TurnSyncDetector::new();
        let mut events = Vec::new();
        let mut samples = 0usize;
        for r in rows(text) {
            let mut rtt = || {
                samples += 1;
                Some(Duration::from_millis(50 + samples as u64))
            };
            if let Some(e) = d.observe(base + Duration::from_micros(r.t_us), r.dir, &r.payload, &mut rtt) {
                events.push((r.t_us, e));
            }
        }
        (events, samples)
    }

    const TURN: &str = include_str!("../tests/fixtures/turn_sync_turn.txt");
    const START: &str = include_str!("../tests/fixtures/turn_sync_game_start.txt");

    #[test]
    fn a_captured_turn_sync_starts_at_the_8301_and_ends_at_the_first_4309() {
        let (events, _) = run(TURN);
        // The Joiner's 0x8301 at 2782 ms into the capture, the host's first 0x4309 after it.
        let started: Vec<_> = events.iter().filter(|(_, e)| matches!(e, Event::Started(_))).collect();
        let finished: Vec<_> = events.iter().filter(|(_, e)| matches!(e, Event::Finished(_))).collect();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].1, Event::Started(SyncKind::Turn));
        assert_eq!(finished.len(), 1, "later 0x4309 copies and the 0x4301 resend must not make another");
        let Event::Finished(r) = &finished[0].1 else { unreachable!() };
        assert_eq!(r.kind, SyncKind::Turn);
        assert_eq!(r.duration, Duration::from_micros(24_684_017));
    }

    #[test]
    fn a_captured_turn_sync_has_its_figures() {
        let (events, _) = run(TURN);
        let Some((_, Event::Finished(r))) = events.last() else { panic!("no finished sync") };
        // Figures worked out from the capture's lines by a separate script.
        assert_eq!(r.turn, Some(2));
        assert_eq!((r.out_msgs, r.out_bytes), (178, 178_420));
        assert_eq!((r.in_msgs, r.in_bytes), (176, 2_600));
        assert_eq!(r.idle, Duration::from_micros(17_109_468));
    }

    #[test]
    fn the_median_rtt_is_taken_over_the_samples_during_the_sync() {
        let (events, samples) = run(TURN);
        let Some((_, Event::Finished(r))) = events.last() else { panic!("no finished sync") };
        // Samples are 51, 52, ... ms in the order they were asked for; the sync is
        // sampled from its start to its end, so the median sits in the middle of them.
        let lo = r.rtt_median.unwrap();
        assert!(samples >= 20, "a 25 s sync is sampled many times, got {samples}");
        assert!(lo > Duration::from_millis(51) && lo < Duration::from_millis(50 + samples as u64));
        // Samples are throttled, not one per message.
        assert!(samples < 200);
    }

    #[test]
    fn a_captured_game_start_ends_at_the_first_4309_and_names_turn_1() {
        let (events, _) = run(START);
        assert_eq!(events.iter().filter(|(_, e)| matches!(e, Event::Started(SyncKind::GameStart))).count(), 1);
        let Some((_, Event::Finished(r))) = events.last() else { panic!("no finished game start") };
        assert_eq!(r.kind, SyncKind::GameStart);
        assert_eq!(r.turn, Some(1));
        assert_eq!(r.duration, Duration::from_micros(38_298_642));
        assert_eq!((r.out_msgs, r.out_bytes), (39, 18_316));
        assert_eq!((r.in_msgs, r.in_bytes), (35, 484));
    }

    fn data_msg(seq: u32, ty: u16, len: usize) -> Vec<u8> {
        let mut v = vec![0u8; len.max(32)];
        v[0] = 4;
        v[4..8].copy_from_slice(&seq.to_le_bytes());
        v[8..10].copy_from_slice(&ty.to_le_bytes());
        v
    }

    fn ack(seq: u32) -> Vec<u8> {
        let mut v = vec![0u8; 12];
        v[0] = 2;
        v[4..8].copy_from_slice(&seq.to_le_bytes());
        v
    }

    fn feed(d: &mut TurnSyncDetector, base: Instant, ms: u64, dir: Direction, data: &[u8]) -> Option<Event> {
        d.observe(base + Duration::from_millis(ms), dir, data, &mut || None)
    }

    #[test]
    fn a_hop_runs_from_our_last_message_to_the_friends_next_at_each_change_of_direction() {
        let mut d = TurnSyncDetector::new();
        let t = Instant::now();
        feed(&mut d, t, 0, Direction::Out, &data_msg(1, 0x4301, 44));
        feed(&mut d, t, 10, Direction::Out, &data_msg(2, 0x4303, 44));
        // Hop 1: from the last out (10 ms) to the first in (70 ms) = 60 ms.
        feed(&mut d, t, 70, Direction::In, &ack(1));
        feed(&mut d, t, 75, Direction::In, &ack(2));
        // A change from in to out is the friend's turnaround, not a hop.
        feed(&mut d, t, 100, Direction::Out, &data_msg(3, 0x4305, 44));
        // Hop 2: 100 -> 150 = 50 ms; it ends on the finishing message.
        let Some(Event::Finished(r)) = feed(&mut d, t, 150, Direction::In, &data_msg(9, 0x4309, 44)) else {
            panic!("no report")
        };
        assert_eq!((r.hops, r.hop_total), (2, Duration::from_millis(110)));
    }

    #[test]
    fn early_ack_counts_since_an_earlier_snapshot() {
        let a = EarlyAckCounts { made: 5, withheld: 2, swallowed: 4 };
        let b = EarlyAckCounts { made: 9, withheld: 2, swallowed: 7 };
        assert_eq!(b.since(a), EarlyAckCounts { made: 4, withheld: 0, swallowed: 3 });
    }

    #[test]
    fn a_4309_outside_a_sync_is_ignored() {
        let mut d = TurnSyncDetector::new();
        let t = Instant::now();
        assert_eq!(feed(&mut d, t, 0, Direction::Out, &data_msg(1, 0x4309, 44)), None);
        assert!(!d.in_sync());
    }

    #[test]
    fn a_turn_sync_waits_for_the_ack_and_counts_idle_only_when_nothing_is_in_flight() {
        let mut d = TurnSyncDetector::new();
        let t = Instant::now();
        feed(&mut d, t, 0, Direction::In, &data_msg(1, 0x8301, 44));
        feed(&mut d, t, 10, Direction::Out, &ack(1)); // nothing in flight from 10 ms
        feed(&mut d, t, 110, Direction::Out, &data_msg(7, 0x4303, 44)); // 100 ms idle, then in flight
        feed(&mut d, t, 160, Direction::In, &ack(7)); // 50 ms in flight
        let Some(Event::Finished(r)) = feed(&mut d, t, 200, Direction::Out, &data_msg(8, 0x4309, 44)) else {
            panic!("0x4309 should end the sync")
        };
        // From 0 to 10 ms the 0x8301 waited for its ack, 10 to 110 idle, 110 to 160 in flight, 160 to 200 idle.
        assert_eq!(r.idle, Duration::from_millis(140));
        assert_eq!(r.duration, Duration::from_millis(200));
    }

    #[test]
    fn a_resent_4301_after_the_sync_does_not_start_another() {
        let mut d = TurnSyncDetector::new();
        let t = Instant::now();
        feed(&mut d, t, 0, Direction::Out, &data_msg(5, 0x4301, 44));
        feed(&mut d, t, 10, Direction::Out, &data_msg(6, 0x4309, 44));
        assert!(!d.in_sync());
        assert_eq!(feed(&mut d, t, 20, Direction::Out, &data_msg(5, 0x4301, 44)), None);
        assert!(!d.in_sync());
    }

    #[test]
    fn the_lobby_arms_the_next_game_start() {
        let mut d = TurnSyncDetector::new();
        let t = Instant::now();
        feed(&mut d, t, 0, Direction::Out, &data_msg(1, 0x4101, 100));
        feed(&mut d, t, 1, Direction::Out, &data_msg(2, 0x4309, 44));
        // A state sync mid-game is no game start.
        assert_eq!(feed(&mut d, t, 2, Direction::Out, &data_msg(3, 0x4101, 100)), None);
        // A new lobby, then a new game.
        feed(&mut d, t, 3, Direction::Out, &data_msg(4, 0x4f04, 100));
        assert_eq!(
            feed(&mut d, t, 4, Direction::Out, &data_msg(5, 0x4101, 100)),
            Some(Event::Started(SyncKind::GameStart))
        );
    }
}
