//! The **friend registry**: one per Transport, and everything sent to or
//! received from friends goes through it.
//!
//! It keeps one record per friend: the friend's endpoint id, its ticket, its
//! players' DPIDs (these are the routes), its current **link**, and a slot for
//! facts scoped to the Game session that must outlive a link.
//!
//! Like the friend pipeline, the registry only says what should happen with
//! anything async. The connection manager does the iroh work (dialling, the
//! readers, the ordered writers), builds a [`Link`] for each connection and
//! tells the registry about links opening and ending. The registry never
//! touches iroh and never reads the session manager: roster facts are passed
//! in by the caller.
//!
//! Locking (ADR-0001 §2): one short synchronous lock. It is never held across
//! an await, or while calling a pipeline, the inbox or the capture. The
//! registry copies out the links it needs, releases its lock, then calls the
//! pipeline and acts on its outcome.

use crate::friend_pipeline::{
    FriendPipeline, InboundOutcome, InboundResult, OutboundOutcome, OutboundResult,
};
use crate::inbox::GameInbox;
use crate::protocol::{encode_message, Message};
use crate::{TransportError, TransportResult};
use dp_types::{dpid, DPID};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, debug_span, warn};

/// A friend's endpoint id, as bytes.
pub type FriendId = [u8; 32];

/// One encoded frame and its capture id, as the ordered writer takes it.
pub type Frame = (Vec<u8>, Option<u64>);

/// The outbox into one connection's ordered writer.
pub type Outbox = mpsc::UnboundedSender<Frame>;

/// Tells two links to the same friend apart. Given by the registry when a
/// link opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkId(u64);

/// One connection to a friend, as the registry sees it.
pub struct Link<H> {
    id: LinkId,
    outbox: Outbox,
    pipeline: FriendPipeline,
    handle: H,
}

impl<H> Link<H> {
    /// A link from the outbox into its connection's ordered writer, the
    /// friend pipeline for this connection, and an opaque connection handle
    /// the registry hands back for request/response streams, path reads and
    /// closing.
    pub fn new(outbox: Outbox, pipeline: FriendPipeline, handle: H) -> Self {
        Link {
            id: LinkId(0),
            outbox,
            pipeline,
            handle,
        }
    }
}

/// What the Transport's Send knows when it hands a game message over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendFacts {
    /// Whether we host the Game session.
    pub is_host: bool,
    /// The host's endpoint id, when we joined.
    pub host: Option<FriendId>,
    /// Our own player, once the game created it.
    pub local_player: Option<DPID>,
}

/// Facts about a friend scoped to the Game session that must outlive a link.
/// Cleared when a Game session begins. Empty for now.
#[derive(Debug, Default)]
struct GameSessionFacts {}

struct Record<H> {
    /// From our dial or the friend's Hello. Nothing reads it yet: it is kept
    /// for re-dialling the friend.
    #[allow(dead_code)]
    ticket: Option<String>,
    /// The friend's players: the routes.
    players: Vec<DPID>,
    link: Option<Arc<Link<H>>>,
    /// Nothing reads it yet: it is the home for facts that outlive a link.
    #[allow(dead_code)]
    session: GameSessionFacts,
}

impl<H> Default for Record<H> {
    fn default() -> Self {
        Record {
            ticket: None,
            players: Vec::new(),
            link: None,
            session: GameSessionFacts::default(),
        }
    }
}

struct State<H> {
    friends: HashMap<FriendId, Record<H>>,
    next_link: u64,
}

/// See the module docs.
pub struct FriendRegistry<H> {
    state: Mutex<State<H>>,
    inbox: GameInbox,
}

impl<H: Clone> FriendRegistry<H> {
    /// An empty registry that delivers into `inbox`.
    pub fn new(inbox: GameInbox) -> Self {
        FriendRegistry {
            state: Mutex::new(State {
                friends: HashMap::new(),
                next_link: 0,
            }),
            inbox,
        }
    }

    /// A link to `friend` opened. `ticket` is the one we dialled with, or
    /// `None` for a connection the friend made to us. Returns the link's id,
    /// for [`link_ended`](Self::link_ended).
    ///
    /// For now, as before the registry: a link we dialled replaces the
    /// friend's link, and a link the friend made is kept only if the friend
    /// has none. A link that isn't kept is dropped, which ends its writer.
    pub fn link_opened(&self, friend: FriendId, ticket: Option<String>, link: Link<H>) -> LinkId {
        let mut s = self.state.lock();
        s.next_link += 1;
        let id = LinkId(s.next_link);
        let record = s.friends.entry(friend).or_default();
        let dialled = ticket.is_some();
        if dialled {
            record.ticket = ticket;
        }
        if dialled || record.link.is_none() {
            record.link = Some(Arc::new(Link { id, ..link }));
        }
        id
    }

    /// A link to `friend` ended. For now, as before the registry: whichever
    /// of the friend's links it was, the friend is forgotten, routes and all,
    /// and its link's pipeline hears "connection lost". The caller then acts
    /// on the loss.
    pub fn link_ended(&self, friend: FriendId, ended: LinkId) {
        let removed = self.state.lock().friends.remove(&friend);
        if let Some(link) = removed.and_then(|r| r.link) {
            debug!(endpoint = %short(&friend), current = link.id == ended, "link ended");
            link.pipeline.connection_lost();
        }
    }

    /// `player` is behind `friend` (a player joined, or the host accepted its
    /// join): route its messages there.
    pub fn player_joined(&self, player: DPID, friend: FriendId) {
        let mut s = self.state.lock();
        for record in s.friends.values_mut() {
            record.players.retain(|p| *p != player);
        }
        s.friends.entry(friend).or_default().players.push(player);
    }

    /// `friend` said Hello for its `player`: route the player there and keep
    /// the friend's ticket.
    pub fn hello(&self, friend: FriendId, player: DPID, ticket: String) {
        self.player_joined(player, friend);
        if !ticket.is_empty() {
            if let Some(record) = self.state.lock().friends.get_mut(&friend) {
                record.ticket = Some(ticket);
            }
        }
    }

    /// `player` left: it is no longer routed anywhere.
    pub fn player_left(&self, player: DPID) {
        let mut s = self.state.lock();
        for record in s.friends.values_mut() {
            record.players.retain(|p| *p != player);
        }
        // A friend with no link and no players is nothing to keep.
        s.friends
            .retain(|_, r| r.link.is_some() || !r.players.is_empty());
    }

    /// A Game session began: every link's pipeline hears it, and each
    /// friend's facts scoped to the Game session are cleared.
    pub fn game_session_began(&self) {
        let links: Vec<Arc<Link<H>>> = {
            let mut s = self.state.lock();
            s.friends
                .values_mut()
                .filter_map(|r| {
                    r.session = GameSessionFacts::default();
                    r.link.clone()
                })
                .collect()
        };
        for link in links {
            link.pipeline.game_session_began();
        }
    }

    /// Forget `friend`, routes and all. Returns its link's connection handle,
    /// for the caller to close; the loss is handled when the link ends.
    pub fn disconnect(&self, friend: FriendId) -> Option<H> {
        let removed = self.state.lock().friends.remove(&friend)?;
        removed.link.map(|l| l.handle.clone())
    }

    /// Forget every friend. Returns their links' connection handles, for the
    /// caller to close.
    pub fn disconnect_all(&self) -> Vec<H> {
        let removed: Vec<_> = self.state.lock().friends.drain().collect();
        removed
            .into_iter()
            .filter_map(|(_, r)| Some(r.link?.handle.clone()))
            .collect()
    }

    /// Whether `friend` has a link.
    pub fn is_connected(&self, friend: FriendId) -> bool {
        self.link_handle(friend).is_some()
    }

    /// The connection handle of `friend`'s link, for request/response
    /// streams.
    pub fn link_handle(&self, friend: FriendId) -> Option<H> {
        let s = self.state.lock();
        Some(s.friends.get(&friend)?.link.as_ref()?.handle.clone())
    }

    /// Every friend with a link, and that link's connection handle, for
    /// reading who is connected and how.
    pub fn connected_friends(&self) -> Vec<(FriendId, H)> {
        let s = self.state.lock();
        s.friends
            .iter()
            .filter_map(|(id, r)| Some((*id, r.link.as_ref()?.handle.clone())))
            .collect()
    }

    /// Send a game message from the local game. Routes by DPID, calls each
    /// target friend's pipeline and carries out its outcome:
    /// - all players: every friend with a link, once each, never back to us
    /// - the server player, when we host: the loopback into the game inbox
    /// - the server player, when we joined: the host's friend
    /// - any other DPID: the friend whose record lists it
    ///
    /// A broadcast attempts every friend and fails with the last error. A
    /// failed send writes the capture's `send_failed` line.
    pub fn send_game(
        &self,
        from: DPID,
        to: DPID,
        guaranteed: bool,
        data: Vec<u8>,
        facts: &SendFacts,
    ) -> TransportResult<()> {
        if to == dpid::DPID_SERVERPLAYER && facts.is_host {
            // The host's game sending to its own server player: it never
            // leaves the Helper, and comes back through the game's inbox.
            let capture_id = crate::capture::loopback(from, to, guaranteed, &data);
            self.inbox
                .add_loopback(from, to, data, guaranteed, capture_id);
            debug!("queued local loopback for host->SERVERPLAYER");
            return Ok(());
        }
        let flags = if guaranteed {
            dp_types::DPSEND_GUARANTEED
        } else {
            0
        };
        let msg = Message::GameMessage {
            from,
            to,
            flags,
            data,
        };
        let Message::GameMessage { data, .. } = &msg else {
            unreachable!()
        };
        let result = self.game_targets(to, facts).and_then(|targets| {
            let encoded = encode_message(&msg)?;
            let mut last_error = None;
            for (friend, link) in targets {
                if let Err(e) = queue_game(&link, from, to, guaranteed, data, encoded.clone()) {
                    warn!(endpoint = %short(&friend), error = %e, "enqueue failed (peer dead)");
                    last_error = Some(e);
                }
            }
            last_error.map_or(Ok(()), Err)
        });
        if let Err(e) = &result {
            debug!(error = %e, "send failed");
            crate::capture::send_failed(from, to, guaranteed, data, &e.to_string());
        }
        result
    }

    /// The friends a game message to `to` goes to, with their links.
    fn game_targets(
        &self,
        to: DPID,
        facts: &SendFacts,
    ) -> TransportResult<Vec<(FriendId, Arc<Link<H>>)>> {
        let s = self.state.lock();
        if to == dpid::DPID_ALLPLAYERS {
            return Ok(s
                .friends
                .iter()
                .filter_map(|(id, r)| Some((*id, r.link.clone()?)))
                .collect());
        }
        let target = if to == dpid::DPID_SERVERPLAYER {
            let host = facts.host.ok_or(TransportError::NotConnected)?;
            let link = s.friends.get(&host).and_then(|r| r.link.clone());
            (host, link.ok_or(TransportError::NotConnected)?)
        } else {
            let found = s.friends.iter().find(|(_, r)| r.players.contains(&to));
            let found = found.and_then(|(id, r)| Some((*id, r.link.clone()?)));
            found.ok_or(TransportError::PlayerNotFound)?
        };
        Ok(vec![target])
    }

    /// A game message was read from `friend`. It goes through the friend's
    /// pipeline, whose outcome is carried out. From a sender with no link (its
    /// connection ended and the reader hasn't finished) it is delivered with
    /// its plain capture line and no pipeline.
    pub fn receive_game(
        &self,
        friend: FriendId,
        from: DPID,
        to: DPID,
        guaranteed: bool,
        data: Vec<u8>,
    ) {
        let link = self
            .state
            .lock()
            .friends
            .get(&friend)
            .and_then(|r| r.link.clone());
        let Some(link) = link else {
            let capture_id = crate::capture::inbound(&friend, from, to, guaranteed, &data);
            self.inbox
                .add_game_message(from, to, data, guaranteed, capture_id);
            return;
        };
        let InboundResult {
            outcome,
            capture_id,
            sync_event: _,
        } = link.pipeline.inbound(from, to, guaranteed, &data);
        match outcome {
            InboundOutcome::Deliver => self
                .inbox
                .add_game_message(from, to, data, guaranteed, capture_id),
        }
    }

    /// Send a control message (any Peer protocol message but a game message)
    /// to one friend. Fails if the friend has no link.
    pub fn send_control(&self, friend: FriendId, msg: &Message) -> TransportResult<()> {
        let link = self
            .state
            .lock()
            .friends
            .get(&friend)
            .and_then(|r| r.link.clone());
        let link = link.ok_or(TransportError::NotConnected)?;
        queue_control(friend, &link, msg, encode_message(msg)?)
    }

    /// Send a control message to every friend with a link. Every friend is
    /// attempted; the result is the last error, if any.
    pub fn send_control_to_all(&self, msg: &Message) -> TransportResult<()> {
        self.control_to_all_filtered(None, msg)
    }

    /// Send a control message to every friend with a link but `except`, as
    /// [`send_control_to_all`](Self::send_control_to_all) does.
    pub fn send_control_to_all_but(&self, except: FriendId, msg: &Message) -> TransportResult<()> {
        self.control_to_all_filtered(Some(except), msg)
    }

    fn control_to_all_filtered(
        &self,
        except: Option<FriendId>,
        msg: &Message,
    ) -> TransportResult<()> {
        let encoded = encode_message(msg)?;
        let targets: Vec<(FriendId, Arc<Link<H>>)> = {
            let s = self.state.lock();
            s.friends
                .iter()
                .filter(|(id, _)| Some(**id) != except)
                .filter_map(|(id, r)| Some((*id, r.link.clone()?)))
                .collect()
        };
        let span = debug_span!(
            "broadcast",
            peer_count = targets.len(),
            msg_type = msg.type_name()
        );
        let _guard = span.enter();
        let mut last_error = None;
        for (friend, link) in targets {
            if let Err(e) = queue_control(friend, &link, msg, encoded.clone()) {
                warn!(endpoint = %short(&friend), error = %e, "enqueue failed (peer dead)");
                last_error = Some(e);
            }
        }
        last_error.map_or(Ok(()), Err)
    }
}

/// Call the link's pipeline for an outbound game message and carry out its
/// outcome.
fn queue_game<H>(
    link: &Link<H>,
    from: DPID,
    to: DPID,
    guaranteed: bool,
    data: &[u8],
    encoded: Vec<u8>,
) -> TransportResult<()> {
    let OutboundResult {
        outcome,
        capture_id,
        sync_event: _,
    } = link.pipeline.outbound(from, to, guaranteed, data);
    match outcome {
        OutboundOutcome::Send => link
            .outbox
            .send((encoded, capture_id))
            .map_err(|_| TransportError::NotConnected),
    }
}

/// Queue an encoded control message on a link, with its capture line first.
/// Control messages never pass through the pipeline.
fn queue_control<H>(
    friend: FriendId,
    link: &Link<H>,
    msg: &Message,
    encoded: Vec<u8>,
) -> TransportResult<()> {
    debug_assert!(
        !matches!(msg, Message::GameMessage { .. }),
        "game messages go through send_game"
    );
    crate::capture::outbound_control(&friend, msg);
    link.outbox
        .send((encoded, None))
        .map_err(|_| TransportError::NotConnected)
}

/// Short hex form of an endpoint id for log lines.
fn short(friend: &FriendId) -> String {
    format!(
        "{:08x}",
        u32::from_be_bytes([friend[0], friend[1], friend[2], friend[3]])
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::decode_message;
    use std::time::Instant;
    use tokio::sync::mpsc::UnboundedReceiver;

    const ALICE: FriendId = [1; 32];
    const BOB: FriendId = [2; 32];
    const LOCAL: DPID = 0x10;
    const ALICE_PLAYER: DPID = 0x11;
    const BOB_PLAYER: DPID = 0x12;

    /// A stand-in link: a plain channel whose receiving end the test holds,
    /// a stand-in handle, and an RTT of "none".
    fn stand_in(friend: FriendId) -> (Link<()>, UnboundedReceiver<Frame>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Link::new(tx, FriendPipeline::new(friend, Instant::now, || None), ()),
            rx,
        )
    }

    /// Everything queued on an outbox so far, decoded. Messages are compared
    /// by their debug form, as `Message` has no `PartialEq`.
    fn drain(rx: &mut UnboundedReceiver<Frame>) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok((bytes, _)) = rx.try_recv() {
            out.push(format!("{:?}", decode_message(&bytes).unwrap()));
        }
        out
    }

    fn game(from: DPID, to: DPID, data: &[u8]) -> String {
        format!(
            "{:?}",
            Message::GameMessage {
                from,
                to,
                flags: dp_types::DPSEND_GUARANTEED,
                data: data.to_vec()
            }
        )
    }

    fn joiner() -> SendFacts {
        SendFacts {
            is_host: false,
            host: Some(ALICE),
            local_player: Some(LOCAL),
        }
    }

    /// A registry with Alice and Bob linked, each with one player.
    fn two_friends() -> (
        FriendRegistry<()>,
        GameInbox,
        UnboundedReceiver<Frame>,
        UnboundedReceiver<Frame>,
    ) {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (a, arx) = stand_in(ALICE);
        let (b, brx) = stand_in(BOB);
        reg.link_opened(ALICE, None, a);
        reg.link_opened(BOB, None, b);
        reg.player_joined(ALICE_PLAYER, ALICE);
        reg.player_joined(BOB_PLAYER, BOB);
        (reg, inbox, arx, brx)
    }

    #[test]
    fn all_players_reaches_each_friend_once_and_not_the_sender() {
        let (reg, inbox, mut arx, mut brx) = two_friends();
        reg.send_game(LOCAL, dpid::DPID_ALLPLAYERS, true, vec![7], &joiner())
            .unwrap();
        assert_eq!(
            drain(&mut arx),
            vec![game(LOCAL, dpid::DPID_ALLPLAYERS, &[7])]
        );
        assert_eq!(
            drain(&mut brx),
            vec![game(LOCAL, dpid::DPID_ALLPLAYERS, &[7])]
        );
        assert_eq!(inbox.count(), 0);
    }

    #[test]
    fn the_server_player_loops_back_to_the_inbox_when_we_host() {
        let (reg, inbox, mut arx, mut brx) = two_friends();
        let host = SendFacts {
            is_host: true,
            host: None,
            local_player: Some(LOCAL),
        };
        reg.send_game(LOCAL, dpid::DPID_SERVERPLAYER, false, vec![3, 4], &host)
            .unwrap();
        let back = inbox.take().unwrap();
        assert_eq!(
            (back.from, back.to, back.data, back.guaranteed),
            (LOCAL, dpid::DPID_SERVERPLAYER, vec![3, 4], false)
        );
        assert!(inbox.take().is_none());
        assert!(drain(&mut arx).is_empty());
        assert!(drain(&mut brx).is_empty());
    }

    #[test]
    fn the_server_player_reaches_the_hosts_friend_when_we_joined() {
        let (reg, inbox, mut arx, mut brx) = two_friends();
        reg.send_game(LOCAL, dpid::DPID_SERVERPLAYER, true, vec![5], &joiner())
            .unwrap();
        assert_eq!(
            drain(&mut arx),
            vec![game(LOCAL, dpid::DPID_SERVERPLAYER, &[5])]
        );
        assert!(drain(&mut brx).is_empty());
        assert_eq!(inbox.count(), 0);
    }

    #[test]
    fn a_dpid_reaches_only_its_friend() {
        let (reg, inbox, mut arx, mut brx) = two_friends();
        reg.send_game(LOCAL, BOB_PLAYER, true, vec![9], &joiner())
            .unwrap();
        assert!(drain(&mut arx).is_empty());
        assert_eq!(drain(&mut brx), vec![game(LOCAL, BOB_PLAYER, &[9])]);
        assert_eq!(inbox.count(), 0);
    }

    #[test]
    fn an_unknown_dpid_fails_with_player_not_found() {
        let (reg, _inbox, mut arx, mut brx) = two_friends();
        let result = reg.send_game(LOCAL, 0x99, true, vec![9], &joiner());
        assert!(
            matches!(result, Err(TransportError::PlayerNotFound)),
            "{result:?}"
        );
        assert!(drain(&mut arx).is_empty());
        assert!(drain(&mut brx).is_empty());
    }

    fn ping(n: u64) -> Message {
        Message::Ping { timestamp: n }
    }

    #[test]
    fn one_friends_frames_come_out_in_call_order_with_game_and_control_interleaved() {
        let (reg, _inbox, mut arx, mut brx) = two_friends();
        reg.send_game(LOCAL, BOB_PLAYER, true, vec![1], &joiner())
            .unwrap();
        reg.send_control(BOB, &ping(2)).unwrap();
        reg.send_game(LOCAL, dpid::DPID_ALLPLAYERS, true, vec![3], &joiner())
            .unwrap();
        reg.send_control_to_all(&ping(4)).unwrap();
        reg.send_control_to_all_but(ALICE, &ping(5)).unwrap();
        reg.send_game(LOCAL, BOB_PLAYER, true, vec![6], &joiner())
            .unwrap();
        assert_eq!(
            drain(&mut brx),
            vec![
                game(LOCAL, BOB_PLAYER, &[1]),
                format!("{:?}", ping(2)),
                game(LOCAL, dpid::DPID_ALLPLAYERS, &[3]),
                format!("{:?}", ping(4)),
                format!("{:?}", ping(5)),
                game(LOCAL, BOB_PLAYER, &[6]),
            ]
        );
        assert_eq!(
            drain(&mut arx),
            vec![
                game(LOCAL, dpid::DPID_ALLPLAYERS, &[3]),
                format!("{:?}", ping(4))
            ]
        );
    }

    #[test]
    fn a_control_message_to_a_friend_with_no_link_fails_with_not_connected() {
        let (reg, _inbox, _arx, _brx) = two_friends();
        let result = reg.send_control([3; 32], &ping(1));
        assert!(
            matches!(result, Err(TransportError::NotConnected)),
            "{result:?}"
        );
    }

    #[test]
    fn game_messages_from_friends_reach_the_inbox_in_arrival_order() {
        let (reg, inbox, _arx, _brx) = two_friends();
        reg.receive_game(ALICE, ALICE_PLAYER, LOCAL, true, vec![1]);
        reg.receive_game(BOB, BOB_PLAYER, dpid::DPID_ALLPLAYERS, false, vec![2]);
        reg.receive_game(ALICE, ALICE_PLAYER, LOCAL, true, vec![3]);
        let got: Vec<_> = std::iter::from_fn(|| inbox.take())
            .map(|m| (m.from, m.to, m.data, m.guaranteed))
            .collect();
        assert_eq!(
            got,
            vec![
                (ALICE_PLAYER, LOCAL, vec![1], true),
                (BOB_PLAYER, dpid::DPID_ALLPLAYERS, vec![2], false),
                (ALICE_PLAYER, LOCAL, vec![3], true),
            ]
        );
    }

    #[test]
    fn a_game_message_from_a_sender_with_no_record_is_still_delivered() {
        let (reg, inbox, _arx, _brx) = two_friends();
        reg.receive_game([3; 32], 0x13, LOCAL, true, vec![4]);
        let m = inbox.take().unwrap();
        assert_eq!((m.from, m.to, m.data), (0x13, LOCAL, vec![4]));
    }

    #[test]
    fn a_hello_routes_the_friends_player_there() {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox);
        let (a, mut arx) = stand_in(ALICE);
        reg.link_opened(ALICE, None, a);
        reg.hello(ALICE, ALICE_PLAYER, "alice's ticket".into());
        reg.send_game(LOCAL, ALICE_PLAYER, true, vec![1], &joiner())
            .unwrap();
        assert_eq!(drain(&mut arx), vec![game(LOCAL, ALICE_PLAYER, &[1])]);
    }

    #[test]
    fn a_player_that_left_is_no_longer_routed() {
        let (reg, _inbox, _arx, mut brx) = two_friends();
        reg.player_left(BOB_PLAYER);
        let result = reg.send_game(LOCAL, BOB_PLAYER, true, vec![1], &joiner());
        assert!(
            matches!(result, Err(TransportError::PlayerNotFound)),
            "{result:?}"
        );
        assert!(drain(&mut brx).is_empty());
    }
}
