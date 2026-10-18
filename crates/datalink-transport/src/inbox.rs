//! The game inbox: everything the game will be handed, in the order it is
//! handed over.
//!
//! One per Transport. A single ordered queue holds friends' game messages,
//! the host's local loopback, and the DirectPlay system messages the game is
//! owed. The inbox builds each system message from the event it is given; the
//! caller passes in the roster facts (player count, a leaver's name and flags),
//! so the inbox never reads the session manager.
//!
//! The handle is shared between the IPC thread and the runtime's reader and
//! cleanup tasks. It has one short synchronous lock, never held across an
//! await or while another module's lock is taken.

use crate::protocol::PlayerInfo;
use dp_types::{dpid, PlayerName, DPID};
use parking_lot::Mutex;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tracing::{debug, info};

/// A message queued for the game's DirectPlay `Receive`
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub from: DPID,
    pub to: DPID,
    pub data: Vec<u8>,
    pub guaranteed: bool,
    /// Its id in the traffic capture, for a game message while one is running
    pub capture_id: Option<u64>,
}

/// Something the game is owed a system message for
#[derive(Debug, Clone)]
pub enum SystemEvent {
    /// A player joined. `current_players` is the roster count at that moment.
    PlayerJoined { info: PlayerInfo, current_players: u32 },
    /// A player left. Name and flags are looked up before the player is removed.
    PlayerLeft { player_id: DPID, name: PlayerName, flags: u32 },
    /// The host is gone
    SessionLost,
    /// An already-built DirectPlay system message
    Message(Vec<u8>),
}

#[derive(Default)]
struct Inner {
    queue: VecDeque<QueuedMessage>,
    /// Players a CREATEPLAYERORGROUP has been added for, this Game session
    joined: HashSet<DPID>,
}

/// Handle to the game inbox; clones share one inbox.
#[derive(Clone, Default)]
pub struct GameInbox {
    inner: Arc<Mutex<Inner>>,
}

impl GameInbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// A game message from a friend
    pub fn add_game_message(
        &self,
        from: DPID,
        to: DPID,
        data: Vec<u8>,
        guaranteed: bool,
        capture_id: Option<u64>,
    ) {
        self.inner.lock().queue.push_back(QueuedMessage { from, to, data, guaranteed, capture_id });
    }

    /// The host's own message to the server player, coming back to it
    pub fn add_loopback(&self, from: DPID, to: DPID, data: Vec<u8>, guaranteed: bool, capture_id: Option<u64>) {
        self.add_game_message(from, to, data, guaranteed, capture_id);
    }

    /// Add the system message for `event`. Returns false if it was a second
    /// "player joined" for the same player in this Game session, and so not added.
    pub fn add_system(&self, event: SystemEvent) -> bool {
        let data = match event {
            SystemEvent::PlayerJoined { info, current_players } => {
                if !self.inner.lock().joined.insert(info.player_id) {
                    debug!(player_id = info.player_id, "Skipping duplicate CREATEPLAYERORGROUP");
                    return false;
                }
                dp_types::serialize_dpmsg_createplayerorgroup(
                    info.player_id,
                    dp_types::DPPLAYERTYPE_PLAYER,
                    current_players,
                    &info.name,
                    &info.data,
                    info.flags,
                )
            }
            SystemEvent::PlayerLeft { player_id, name, flags } => dp_types::serialize_dpmsg_destroyplayerorgroup(
                player_id,
                dp_types::DPPLAYERTYPE_PLAYER,
                &name,
                flags,
            ),
            SystemEvent::SessionLost => dp_types::serialize_dpmsg_sessionlost(),
            SystemEvent::Message(data) => data,
        };
        self.inner.lock().queue.push_back(QueuedMessage {
            from: dpid::DPID_SYSMSG,
            to: dpid::DPID_ALLPLAYERS,
            data,
            guaranteed: true,
            capture_id: None,
        });
        true
    }

    /// A Game session began: everything waiting is stale, and players may join again.
    pub fn session_began(&self) {
        let mut inner = self.inner.lock();
        let discarded = inner.queue.len();
        inner.queue.clear();
        inner.joined.clear();
        if discarded > 0 {
            info!(discarded, "Discarded messages left over from an earlier session");
        }
    }

    /// The DLL's Receive: the next item, writing the capture's drained line
    pub fn take(&self) -> Option<QueuedMessage> {
        let msg = self.inner.lock().queue.pop_front()?;
        debug!(from = msg.from, to = msg.to, size = msg.data.len(), "returning message from inbox");
        if let Some(id) = msg.capture_id {
            crate::capture::drained(id);
        } else if msg.from == dpid::DPID_SYSMSG {
            crate::capture::system_drained(&msg.data);
        }
        Some(msg)
    }

    /// The DLL's GetMessageCount
    pub fn count(&self) -> usize {
        self.inner.lock().queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: DPID) -> PlayerInfo {
        PlayerInfo {
            player_id: id,
            name: PlayerName::default(),
            flags: 0,
            node_id: [0; 32],
            ticket: String::new(),
            data: vec![],
        }
    }

    fn joined(id: DPID) -> SystemEvent {
        SystemEvent::PlayerJoined { info: player(id), current_players: 2 }
    }

    #[test]
    fn items_come_out_in_the_order_added() {
        let inbox = GameInbox::new();
        inbox.add_game_message(5, 6, vec![1], true, None);
        inbox.add_system(joined(7));
        inbox.add_loopback(8, dpid::DPID_SERVERPLAYER, vec![2], false, None);
        inbox.add_system(SystemEvent::SessionLost);

        let a = inbox.take().unwrap();
        assert_eq!((a.from, a.data), (5, vec![1]));
        assert_eq!(inbox.take().unwrap().from, dpid::DPID_SYSMSG);
        let c = inbox.take().unwrap();
        assert_eq!((c.from, c.data, c.guaranteed), (8, vec![2], false));
        assert_eq!(inbox.take().unwrap().data, dp_types::serialize_dpmsg_sessionlost());
        assert!(inbox.take().is_none());
    }

    #[test]
    fn a_duplicate_player_joined_is_not_added() {
        let inbox = GameInbox::new();
        assert!(inbox.add_system(joined(7)));
        assert!(!inbox.add_system(joined(7)));
        assert!(inbox.add_system(joined(8)));
        assert_eq!(inbox.count(), 2);
    }

    #[test]
    fn session_began_empties_and_allows_the_same_player_to_join_again() {
        let inbox = GameInbox::new();
        inbox.add_system(joined(7));
        inbox.add_game_message(1, 2, vec![9], true, None);
        inbox.session_began();
        assert_eq!(inbox.count(), 0);
        assert!(inbox.take().is_none());
        assert!(inbox.add_system(joined(7)));
        assert_eq!(inbox.count(), 1);
    }

    #[test]
    fn count_matches_take() {
        let inbox = GameInbox::new();
        for i in 0..4 {
            inbox.add_game_message(1, 2, vec![i], true, None);
        }
        let mut taken = 0;
        while inbox.count() > 0 {
            assert_eq!(inbox.count(), 4 - taken);
            inbox.take().unwrap();
            taken += 1;
        }
        assert_eq!(taken, 4);
    }

    #[test]
    fn system_messages_have_todays_bytes() {
        let inbox = GameInbox::new();
        let name = PlayerName::default();
        inbox.add_system(SystemEvent::PlayerJoined { info: player(7), current_players: 3 });
        inbox.add_system(SystemEvent::PlayerLeft { player_id: 7, name: name.clone(), flags: 4 });
        inbox.add_system(SystemEvent::Message(vec![1, 2, 3]));

        let j = inbox.take().unwrap();
        assert_eq!((j.from, j.to, j.guaranteed), (dpid::DPID_SYSMSG, dpid::DPID_ALLPLAYERS, true));
        assert_eq!(
            j.data,
            dp_types::serialize_dpmsg_createplayerorgroup(7, dp_types::DPPLAYERTYPE_PLAYER, 3, &name, &[], 0)
        );
        assert_eq!(
            inbox.take().unwrap().data,
            dp_types::serialize_dpmsg_destroyplayerorgroup(7, dp_types::DPPLAYERTYPE_PLAYER, &name, 4)
        );
        assert_eq!(inbox.take().unwrap().data, vec![1, 2, 3]);
    }
}
