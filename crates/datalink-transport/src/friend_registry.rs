//! The **friend registry**: one per Transport, and everything sent to or
//! received from friends goes through it.
//!
//! It keeps one record per friend: the friend's endpoint id, its ticket, its
//! players' DPIDs (these are the routes), its current **link**, any spare
//! links, and a slot for facts scoped to the Game session that must outlive a
//! link.
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
use crate::inbox::{GameInbox, SystemEvent};
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

/// Who this Helper is, for the Hello a re-attached link carries first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFacts {
    /// Our own player, once the game created it.
    pub player: Option<DPID>,
    /// Our ticket.
    pub ticket: String,
}

/// What the session roster says about a friend whose link ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterFacts {
    /// Whether the friend hosts our Game session.
    pub was_host: bool,
    /// The friend's ticket, if one of its players is in the roster.
    pub ticket: Option<String>,
}

/// The registry's answer when a link ends: what the caller must do next.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEnded {
    /// Nothing more to do.
    Nothing,
    /// The friend is lost but kept: re-dial it with this ticket, and tell the
    /// registry if the re-dial gives up.
    Redial { ticket: String },
}

/// Facts about a friend scoped to the Game session that must outlive a link.
/// Cleared when a Game session begins. Empty for now.
#[derive(Debug, Default)]
struct GameSessionFacts {}

struct Record<H> {
    /// From our dial, the friend's Hello, or the roster at a loss. Nothing
    /// reads it yet: the re-dial uses the roster's ticket, as before.
    #[allow(dead_code)]
    ticket: Option<String>,
    /// The friend's players: the routes.
    players: Vec<DPID>,
    /// The current link: every frame to the friend goes on it.
    link: Option<Arc<Link<H>>>,
    /// Later links to the friend, oldest first, while the current one is open.
    /// Their readers still deliver; one takes over if the current link ends.
    spares: Vec<Arc<Link<H>>>,
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
            spares: Vec::new(),
            session: GameSessionFacts::default(),
        }
    }
}

impl<H: Clone> Record<H> {
    /// Every link's connection handle: the current link's, then the spares'.
    fn handles(&self) -> Vec<H> {
        self.link
            .iter()
            .chain(&self.spares)
            .map(|l| l.handle.clone())
            .collect()
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
    /// The first link to a friend is current. A later one, from a dial of
    /// ours or the friend's, is a spare while the current one is open: frames
    /// keep going to the current link, and the spare's reader still delivers.
    ///
    /// A link that opens onto a record with no link re-attaches it: a friend
    /// being re-dialled, or one whose players were routed before the dial.
    /// Its players route there again, and when we have a player, our Hello
    /// (from `us`) is the link's first frame.
    pub fn link_opened(
        &self,
        friend: FriendId,
        ticket: Option<String>,
        link: Link<H>,
        us: &LocalFacts,
    ) -> LinkId {
        // Encoded before the lock is taken; queued only if the link
        // re-attaches a record.
        let hello = us.player.map(|player_id| Message::Hello {
            player_id,
            ticket: us.ticket.clone(),
        });
        let encoded_hello = hello.as_ref().and_then(|h| encode_message(h).ok());
        let mut said_hello = false;
        let id = {
            let mut s = self.state.lock();
            s.next_link += 1;
            let id = LinkId(s.next_link);
            let reattaching = s.friends.get(&friend).is_some_and(|r| r.link.is_none());
            let record = s.friends.entry(friend).or_default();
            if ticket.is_some() {
                record.ticket = ticket;
            }
            let link = Arc::new(Link { id, ..link });
            if record.link.is_some() {
                record.spares.push(link);
            } else {
                if reattaching {
                    // Queued while no one else can see the link, so it is the
                    // first frame on it. The friend's Helper routes our player
                    // here before anything else arrives.
                    if let Some(frame) = encoded_hello {
                        said_hello = link.outbox.send((frame, None)).is_ok();
                    }
                }
                record.link = Some(link);
            }
            id
        };
        if let (true, Some(hello)) = (said_hello, &hello) {
            crate::capture::outbound_control(&friend, hello);
        }
        id
    }

    /// A link to `friend` ended. `roster` is what the session roster says
    /// about the friend.
    ///
    /// A spare ending is removed, and nothing else happens. The current link
    /// ending while a spare is open hands over to the oldest spare: the old
    /// link's pipeline hears "connection lost", but the friend is not lost.
    ///
    /// The current link ending with no spare open loses the friend: the
    /// link's pipeline hears "connection lost", the capture's `lost` line is
    /// written, and
    /// - the host is forgotten, and the game told its session is lost
    /// - a friend with players and a roster ticket is kept with no link, so
    ///   directed sends to its players fail while it is re-dialled, and the
    ///   answer is "re-dial with this ticket"
    /// - anyone else (a friend only connected for session discovery) is
    ///   forgotten
    ///
    /// A link of a friend already forgotten (every friend is, at shutdown) is
    /// still a loss: the `lost` line is written and, for the host, the game
    /// told.
    pub fn link_ended(&self, friend: FriendId, ended: LinkId, roster: RosterFacts) -> LinkEnded {
        let (lost, answer) = {
            let mut s = self.state.lock();
            match s.friends.get_mut(&friend) {
                None => (None, LinkEnded::Nothing),
                Some(record) if record.link.as_ref().map(|l| l.id) != Some(ended) => {
                    record.spares.retain(|l| l.id != ended);
                    debug!(endpoint = %short(&friend), "a link that is not the friend's current one ended");
                    return LinkEnded::Nothing;
                }
                Some(record) if !record.spares.is_empty() => {
                    let next = record.spares.remove(0);
                    let old = record.link.replace(next);
                    drop(s);
                    debug!(endpoint = %short(&friend), "current link ended, a spare took over");
                    if let Some(old) = old {
                        old.pipeline.connection_lost();
                    }
                    return LinkEnded::Nothing;
                }
                Some(record) => {
                    let lost = record.link.take();
                    let answer = match &roster.ticket {
                        Some(ticket) if !roster.was_host && !record.players.is_empty() => {
                            record.ticket = Some(ticket.clone());
                            LinkEnded::Redial {
                                ticket: ticket.clone(),
                            }
                        }
                        _ => {
                            s.friends.remove(&friend);
                            LinkEnded::Nothing
                        }
                    };
                    (lost, answer)
                }
            }
        };
        debug!(endpoint = %short(&friend), was_host = roster.was_host, ?answer, "friend lost");
        if let Some(link) = lost {
            link.pipeline.connection_lost();
        }
        crate::capture::connection_lost(&friend, roster.was_host);
        if roster.was_host {
            self.inbox.add_system(SystemEvent::SessionLost);
        }
        answer
    }

    /// Every re-dial of `friend` failed: forget it, routes and all, unless a
    /// link to it opened meanwhile. Game-session-sync ticket 05 may later
    /// make this tell the game the friend's players left.
    pub fn redial_gave_up(&self, friend: FriendId) {
        let mut s = self.state.lock();
        if s.friends.get(&friend).is_some_and(|r| r.link.is_none()) {
            s.friends.remove(&friend);
            debug!(endpoint = %short(&friend), "re-dial gave up, friend forgotten");
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

    /// `player` left: it is no longer routed anywhere. Its friend is kept,
    /// even with no players left: a friend is forgotten only when it is lost,
    /// its re-dial gives up, or it is disconnected.
    pub fn player_left(&self, player: DPID) {
        let mut s = self.state.lock();
        for record in s.friends.values_mut() {
            record.players.retain(|p| *p != player);
        }
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

    /// The connection handles of `friend`'s links, current and spare, for the
    /// caller to close. The friend is kept: its loss is handled when the links
    /// end, as any other.
    pub fn disconnect(&self, friend: FriendId) -> Vec<H> {
        let s = self.state.lock();
        s.friends.get(&friend).map_or_else(Vec::new, Record::handles)
    }

    /// Forget every friend. Returns all their links' connection handles, for
    /// the caller to close.
    pub fn disconnect_all(&self) -> Vec<H> {
        let removed: Vec<_> = self.state.lock().friends.drain().collect();
        removed.iter().flat_map(|(_, r)| r.handles()).collect()
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
        reg.link_opened(ALICE, None, a, &us());
        reg.link_opened(BOB, None, b, &us());
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
        reg.link_opened(ALICE, None, a, &us());
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

    /// Who the tests' Helper is: its player and its ticket.
    fn us() -> LocalFacts {
        LocalFacts {
            player: Some(LOCAL),
            ticket: "our ticket".into(),
        }
    }

    /// A stand-in link's id and the receiving end of its outbox.
    type Linked = (LinkId, UnboundedReceiver<Frame>);

    /// Open a stand-in link to `friend`, as accepted, and route `player` there.
    fn linked(reg: &FriendRegistry<()>, friend: FriendId, player: DPID) -> Linked {
        let (link, rx) = stand_in(friend);
        let id = reg.link_opened(friend, None, link, &us());
        reg.player_joined(player, friend);
        (id, rx)
    }

    fn the_host(ticket: &str) -> RosterFacts {
        RosterFacts {
            was_host: true,
            ticket: Some(ticket.into()),
        }
    }

    fn sends_to(reg: &FriendRegistry<()>, player: DPID) -> TransportResult<()> {
        reg.send_game(LOCAL, player, true, vec![1], &joiner())
    }

    #[test]
    fn losing_the_hosts_link_puts_session_lost_in_the_inbox_and_forgets_the_host() {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (alice, _arx) = linked(&reg, ALICE, ALICE_PLAYER);

        let answer = reg.link_ended(ALICE, alice, the_host("alice's ticket"));

        assert_eq!(answer, LinkEnded::Nothing);
        let lost = inbox.take().expect("the game should be told");
        assert_eq!(lost.data, dp_types::serialize_dpmsg_sessionlost());
        assert!(inbox.take().is_none());
        // Forgotten: a new link to the host is not given the host's player.
        let (link, mut arx) = stand_in(ALICE);
        reg.link_opened(ALICE, None, link, &us());
        let result = sends_to(&reg, ALICE_PLAYER);
        assert!(matches!(result, Err(TransportError::PlayerNotFound)), "{result:?}");
        assert!(drain(&mut arx).is_empty());
    }

    fn a_joiner(ticket: &str) -> RosterFacts {
        RosterFacts {
            was_host: false,
            ticket: Some(ticket.into()),
        }
    }

    #[test]
    fn losing_a_friend_with_players_answers_redial_and_directed_sends_to_them_fail() {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (_alice, _arx) = linked(&reg, ALICE, ALICE_PLAYER);
        let (bob, _brx) = linked(&reg, BOB, BOB_PLAYER);

        let answer = reg.link_ended(BOB, bob, a_joiner("bob's ticket"));

        assert_eq!(
            answer,
            LinkEnded::Redial {
                ticket: "bob's ticket".into()
            }
        );
        let result = sends_to(&reg, BOB_PLAYER);
        assert!(matches!(result, Err(TransportError::PlayerNotFound)), "{result:?}");
        assert_eq!(inbox.count(), 0);
    }

    fn hello_from_us() -> String {
        format!(
            "{:?}",
            Message::Hello {
                player_id: LOCAL,
                ticket: "our ticket".into()
            }
        )
    }

    /// A registry with Bob lost and being re-dialled.
    fn bob_being_redialled() -> (FriendRegistry<()>, GameInbox) {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (bob, _brx) = linked(&reg, BOB, BOB_PLAYER);
        let _ = reg.link_ended(BOB, bob, a_joiner("bob's ticket"));
        (reg, inbox)
    }

    #[test]
    fn a_new_link_for_a_friend_being_redialled_carries_hello_first_then_directed_sends() {
        let (reg, _inbox) = bob_being_redialled();

        let (link, mut brx) = stand_in(BOB);
        reg.link_opened(BOB, Some("bob's ticket".into()), link, &us());
        sends_to(&reg, BOB_PLAYER).unwrap();

        assert_eq!(
            drain(&mut brx),
            vec![hello_from_us(), game(LOCAL, BOB_PLAYER, &[1])]
        );
    }

    #[test]
    fn losing_a_discovery_only_friend_answers_nothing_and_forgets_it() {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (link, _brx) = stand_in(BOB);
        let bob = reg.link_opened(BOB, Some("bob's ticket".into()), link, &us());

        let not_in_roster = RosterFacts {
            was_host: false,
            ticket: None,
        };
        assert_eq!(reg.link_ended(BOB, bob, not_in_roster), LinkEnded::Nothing);

        assert_eq!(inbox.count(), 0);
        // Forgotten: a new link to it is a first link, with no Hello.
        let (link, mut brx) = stand_in(BOB);
        reg.link_opened(BOB, None, link, &us());
        assert!(drain(&mut brx).is_empty());
    }

    #[test]
    fn after_the_redial_gave_up_directed_sends_to_the_friends_players_fail() {
        let (reg, _inbox) = bob_being_redialled();

        reg.redial_gave_up(BOB);

        // Forgotten: a link opening later is a first link, with no Hello and
        // no routes.
        let (link, mut brx) = stand_in(BOB);
        reg.link_opened(BOB, None, link, &us());
        let result = sends_to(&reg, BOB_PLAYER);
        assert!(matches!(result, Err(TransportError::PlayerNotFound)), "{result:?}");
        assert!(drain(&mut brx).is_empty());
    }

    #[test]
    fn a_disconnected_friends_loss_is_handled_when_its_link_ends() {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (bob, _brx) = linked(&reg, BOB, BOB_PLAYER);

        assert_eq!(reg.disconnect(BOB), vec![()]);

        assert_eq!(
            reg.link_ended(BOB, bob, a_joiner("bob's ticket")),
            LinkEnded::Redial {
                ticket: "bob's ticket".into()
            }
        );
    }

    #[test]
    fn a_friend_being_redialled_whose_last_player_left_is_still_reattached() {
        let (reg, _inbox) = bob_being_redialled();

        reg.player_left(BOB_PLAYER);

        // Still known: the re-dial's link re-attaches it, Hello first.
        let (link, mut brx) = stand_in(BOB);
        reg.link_opened(BOB, Some("bob's ticket".into()), link, &us());
        assert_eq!(drain(&mut brx), vec![hello_from_us()]);
    }

    #[test]
    fn an_incoming_link_for_a_friend_being_redialled_is_reattached_the_same_way() {
        let (reg, _inbox) = bob_being_redialled();

        let (link, mut brx) = stand_in(BOB);
        reg.link_opened(BOB, None, link, &us());
        sends_to(&reg, BOB_PLAYER).unwrap();

        assert_eq!(
            drain(&mut brx),
            vec![hello_from_us(), game(LOCAL, BOB_PLAYER, &[1])]
        );
    }

    /// A registry with Bob linked twice: the first link, accepted, and a
    /// spare from our own dial completing after it.
    fn bob_linked_twice() -> (FriendRegistry<()>, GameInbox, Linked, Linked) {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let first = linked(&reg, BOB, BOB_PLAYER);
        let (link, rx) = stand_in(BOB);
        let spare = reg.link_opened(BOB, Some("bob's ticket".into()), link, &us());
        (reg, inbox, first, (spare, rx))
    }

    #[test]
    fn a_second_link_does_not_replace_the_current_one_and_frames_keep_going_to_the_first() {
        let (reg, _inbox, (_, mut first), (_, mut spare)) = bob_linked_twice();

        sends_to(&reg, BOB_PLAYER).unwrap();
        reg.send_control(BOB, &ping(2)).unwrap();

        assert_eq!(
            drain(&mut first),
            vec![game(LOCAL, BOB_PLAYER, &[1]), format!("{:?}", ping(2))]
        );
        assert!(drain(&mut spare).is_empty());
    }

    #[test]
    fn a_spare_ending_changes_nothing() {
        let (reg, inbox, (first_id, mut first), (spare_id, _spare)) = bob_linked_twice();

        assert_eq!(
            reg.link_ended(BOB, spare_id, a_joiner("bob's ticket")),
            LinkEnded::Nothing
        );

        sends_to(&reg, BOB_PLAYER).unwrap();
        assert_eq!(drain(&mut first), vec![game(LOCAL, BOB_PLAYER, &[1])]);
        assert_eq!(inbox.count(), 0);
        // The spare is gone: the first link ending now loses Bob.
        assert_eq!(
            reg.link_ended(BOB, first_id, a_joiner("bob's ticket")),
            LinkEnded::Redial {
                ticket: "bob's ticket".into()
            }
        );
    }

    #[test]
    fn the_current_link_ending_with_a_spare_open_moves_frames_to_the_spare() {
        let (reg, inbox, (first_id, _first), (_, mut spare)) = bob_linked_twice();

        assert_eq!(
            reg.link_ended(BOB, first_id, a_joiner("bob's ticket")),
            LinkEnded::Nothing
        );

        sends_to(&reg, BOB_PLAYER).unwrap();
        reg.send_control(BOB, &ping(2)).unwrap();
        // No Hello: Bob's Helper still routes our player to us.
        assert_eq!(
            drain(&mut spare),
            vec![game(LOCAL, BOB_PLAYER, &[1]), format!("{:?}", ping(2))]
        );
        assert_eq!(inbox.count(), 0);
    }

    #[test]
    fn the_hosts_current_link_ending_with_a_spare_open_is_not_a_lost_session() {
        let inbox = GameInbox::new();
        let reg = FriendRegistry::new(inbox.clone());
        let (first, _arx) = linked(&reg, ALICE, ALICE_PLAYER);
        let (link, mut spare) = stand_in(ALICE);
        reg.link_opened(ALICE, None, link, &us());

        assert_eq!(
            reg.link_ended(ALICE, first, the_host("alice's ticket")),
            LinkEnded::Nothing
        );

        assert_eq!(inbox.count(), 0);
        reg.send_game(LOCAL, dpid::DPID_SERVERPLAYER, true, vec![5], &joiner())
            .unwrap();
        assert_eq!(
            drain(&mut spare),
            vec![game(LOCAL, dpid::DPID_SERVERPLAYER, &[5])]
        );
    }

    #[test]
    fn disconnecting_a_friend_hands_back_every_link_to_close() {
        let (reg, _inbox, _first, _spare) = bob_linked_twice();
        let (_alice, _arx) = linked(&reg, ALICE, ALICE_PLAYER);

        assert_eq!(reg.disconnect(BOB).len(), 2);
        assert_eq!(reg.disconnect_all().len(), 3);
    }
}
