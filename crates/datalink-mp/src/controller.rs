//! Session controller: owns the Helper's current Transport and the status
//! the page shows.
//!
//! The IPC server and the command line both go through the controller, so
//! there is one place that knows which Transport is current. The IPC server
//! also reports here what it sees of the game's DLL. Stop replaces the
//! Transport with a new one, which has a new Ticket. Shutting the controller
//! down closes the Transport for good, and is how the Helper ends.

use crate::platform::{self, SelfCheck};
use dp_types::DPID;
use iroh_transport::{
    Ticket, TicketError, Transport, TransportError, TransportOptions, TransportResult,
    STREAM_PROTO_VERSION,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use thiserror::Error;
use tracing::{info, warn};

/// The Ticket sequence number of the Ticket the Helper starts with.
const FIRST_TICKET_SEQ: u64 = 1;

/// Where the Helper is in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// No peers, no dial in progress
    Ready,
    /// A dial we started (a join) is in progress
    Joining,
    /// Our dial succeeded and at least one peer is connected
    Joined,
    /// At least one peer is connected and we did not dial
    Hosting,
}

/// Why the text given to a join is not a Ticket to dial. The page words the
/// `invalid_ticket` banner by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidTicket {
    /// The text does not parse as a Ticket
    NotATicket,
    /// The text is this Helper's own Ticket
    OwnTicket,
}

/// Why a join was refused before any dial.
#[derive(Debug, Error)]
pub enum JoinRefused {
    /// Worded as the command line's `join` has always worded it.
    #[error("Invalid ticket: {0}")]
    NotATicket(#[source] TicketError),

    #[error("That's this Helper's own Ticket: give the Ticket of the Helper to join")]
    OwnTicket,

    #[error("A dial to a friend's Helper is already in progress")]
    DialInProgress,
}

impl JoinRefused {
    /// Why the `invalid_ticket` banner went up for this refusal, if it did.
    pub fn invalid_ticket(&self) -> Option<InvalidTicket> {
        match self {
            JoinRefused::NotATicket(_) => Some(InvalidTicket::NotATicket),
            JoinRefused::OwnTicket => Some(InvalidTicket::OwnTicket),
            JoinRefused::DialInProgress => None,
        }
    }
}

/// Why Stop changed nothing.
#[derive(Debug, Error)]
pub(crate) enum StopError {
    #[error("Failed to create the new Transport")]
    Transport(#[source] TransportError),

    #[error("The Helper has shut down")]
    ShutDown,
}

/// Why a join failed.
#[derive(Debug, Error)]
pub enum JoinError {
    #[error(transparent)]
    Refused(#[from] JoinRefused),

    #[error("Failed to connect to the friend's Helper")]
    Dial(#[source] TransportError),
}

/// Something about a Ticket that may keep its dial from working, though the
/// dial is still made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JoinWarning {
    /// The Ticket names the friend's Helper but none of its addresses
    TicketNoAddresses,
}

/// A join that passed the checks, whose dial is now in progress: status
/// reports `joining` until [`SessionController::dial`] has run it.
///
/// Dropped without being dialled (a panic, or a runtime shutting down before
/// it ran the dial), it ends the dial in progress all the same, so a later
/// join is never refused for a dial that will not happen.
#[must_use = "the dial is in progress until it is run with SessionController::dial"]
pub(crate) struct PendingDial {
    ticket: Ticket,
    warnings: Vec<JoinWarning>,
    joins: Arc<Mutex<Joins>>,
}

impl PendingDial {
    /// What the player should know about the Ticket before the dial ends.
    pub(crate) fn warnings(&self) -> &[JoinWarning] {
        &self.warnings
    }
}

impl Drop for PendingDial {
    fn drop(&mut self) {
        lock(&self.joins).dialling = false;
    }
}

fn lock(joins: &Mutex<Joins>) -> MutexGuard<'_, Joins> {
    joins.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the controller knows of the joins it was asked for.
#[derive(Debug, Default)]
struct Joins {
    /// A dial is in progress
    dialling: bool,
    /// A dial of ours succeeded, and peers have been connected ever since
    dialled: bool,
    /// How the most recent join attempt failed, if it did in a way the page
    /// has a banner for. These are the event banners: one field, so that a
    /// join attempt (and anything else that clears them) clears them all.
    failure: Option<JoinFailure>,
}

/// How a join attempt failed, as the page tells the player. Each is one of
/// the event banners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinFailure {
    /// The text given is not a Ticket to dial
    InvalidTicket(InvalidTicket),
    /// The dial timed out or could not connect
    CantReachHost,
    /// The friend's Helper rejected the dial for its ALPN: another release
    PeerVersionMismatch,
}

impl JoinFailure {
    /// The banner for a join whose dial, or whatever followed it, failed with
    /// `error`. None for the failures no banner speaks of, such as a host
    /// that has no session yet.
    fn of_dial(error: &TransportError) -> Option<Self> {
        match error {
            TransportError::CantReach => Some(Self::CantReachHost),
            TransportError::PeerProtocolMismatch => Some(Self::PeerVersionMismatch),
            _ => None,
        }
    }

    fn banner(self) -> Banner {
        match self {
            Self::InvalidTicket(_) => Banner::InvalidTicket,
            Self::CantReachHost => Banner::CantReachHost,
            Self::PeerVersionMismatch => Banner::PeerVersionMismatch,
        }
    }
}

/// Something the page must tell the player, as a code the page has the words for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Banner {
    /// The Helper is not running from the Game folder
    NotGameFolder,
    /// Another program holds the IPC port, so the game cannot reach the Helper
    IpcPortInUse,
    /// The game's DLL does not speak this Helper's IPC version
    IpcVersionMismatch,
    /// The most recent join was given text that is not a Ticket, or our own
    /// Ticket. [`Status::invalid_ticket`] says which.
    InvalidTicket,
    /// The most recent join's dial could not reach the friend's Helper
    CantReachHost,
    /// The most recent join's dial was refused by a friend's Helper of
    /// another release
    PeerVersionMismatch,
}

/// A snapshot of the Helper's status, as the page shows it.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub release_version: &'static str,
    pub ipc_version: u32,
    pub peer_protocol_version: u16,
    /// `windows`, `linux` or `macos`
    pub os: &'static str,
    pub state: State,
    pub ticket: String,
    pub ticket_seq: u64,
    pub ipc_port: u16,
    /// What the Game folder self-check found
    pub self_check: SelfCheck,
    /// Whether the game's DLL is connected to the Helper
    pub game_connected: bool,
    /// Short IDs of the connected Helpers: iroh's short form of each endpoint
    /// ID, sorted
    pub peers: Vec<String>,
    /// Codes of the active banners
    pub banners: Vec<Banner>,
    /// Why the `invalid_ticket` banner is up, while it is
    pub invalid_ticket: Option<InvalidTicket>,
}

/// The current Transport, and the sequence number of its Ticket.
struct Current {
    transport: Arc<Transport>,
    ticket_seq: u64,
}

/// Owns the current Transport, and therefore the Helper's Ticket.
pub struct SessionController {
    /// Replaced by Stop. Both fields change under one lock, so the Ticket
    /// status shows always goes with its sequence number. Where the joins are
    /// locked too, they are locked first.
    current: Mutex<Current>,
    /// What each Transport is built with, the one Stop builds included.
    transport_options: TransportOptions,
    game_folder: PathBuf,
    ipc_port: u16,
    ipc_port_in_use: bool,
    game_connected: AtomicBool,
    ipc_version_mismatch: AtomicBool,
    /// Shared with the dial in progress, which ends itself: see [`PendingDial`].
    joins: Arc<Mutex<Joins>>,
    /// Whether the Helper has shut down. Held while it does, and while Stop
    /// replaces the Transport, so that neither races a shutdown or a Stop.
    shut_down: Mutex<bool>,
    shut_down_signal: Condvar,
}

impl SessionController {
    /// Create the controller with a live Transport (this starts the Iroh endpoint).
    ///
    /// `ipc_port_in_use` says that another program holds `ipc_port`, so no IPC
    /// server is listening on it.
    pub(crate) fn new(
        options: TransportOptions,
        game_folder: PathBuf,
        ipc_port: u16,
        ipc_port_in_use: bool,
    ) -> TransportResult<Self> {
        info!("Initializing Iroh transport...");
        let transport = Transport::with_options(options)?;
        info!(
            "Transport initialized. Endpoint ID: {:?}",
            transport.endpoint_id()
        );

        Ok(Self {
            current: Mutex::new(Current {
                transport: Arc::new(transport),
                ticket_seq: FIRST_TICKET_SEQ,
            }),
            transport_options: options,
            game_folder,
            ipc_port,
            ipc_port_in_use,
            game_connected: AtomicBool::new(false),
            ipc_version_mismatch: AtomicBool::new(false),
            joins: Arc::default(),
            shut_down: Mutex::new(false),
            shut_down_signal: Condvar::new(),
        })
    }

    /// The current Transport.
    ///
    /// Ask each time one is needed instead of keeping the result: the current
    /// Transport is replaced by Stop.
    pub fn transport(&self) -> Arc<Transport> {
        self.current().transport.clone()
    }

    fn current(&self) -> MutexGuard<'_, Current> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether `transport` is still the current one: Stop has not replaced it.
    fn is_current(&self, transport: &Arc<Transport>) -> bool {
        Arc::ptr_eq(transport, &self.current().transport)
    }

    /// A snapshot of the Helper's status. Nothing here waits on the Transport
    /// or the network.
    ///
    /// The Game folder self-check is done again for each snapshot, so a file
    /// the player restores is seen without a restart. It reads the folder's
    /// list of files: from async code, call this inside `spawn_blocking`.
    pub fn status(&self) -> Status {
        // The peers are read with the joins locked: a dial that succeeded has
        // put its peer in the Transport's list before it records its success,
        // so what is read here always agrees with what the joins say. So is
        // the Transport, which Stop replaces with the joins locked.
        let mut joins = self.joins();
        let (transport, ticket_seq) = {
            let current = self.current();
            (current.transport.clone(), current.ticket_seq)
        };
        // A plain lock read, unlike the Transport's blocking calls.
        let mut peers = transport
            .connected_peers()
            .iter()
            .map(|id| id.fmt_short().to_string())
            .collect::<Vec<_>>();
        let state = session_state(&mut joins, !peers.is_empty());
        let failure = joins.failure;
        drop(joins);
        // The Transport lists them in no particular order; sorted, the page
        // shows them the same way on every poll.
        peers.sort();
        let mut banners = Vec::new();
        let self_check = platform::check_game_folder(&self.game_folder);
        if !self_check.passed {
            banners.push(Banner::NotGameFolder);
        }
        if self.ipc_port_in_use {
            banners.push(Banner::IpcPortInUse);
        }
        if self.ipc_version_mismatch.load(Ordering::Relaxed) {
            banners.push(Banner::IpcVersionMismatch);
        }
        banners.extend(failure.map(JoinFailure::banner));
        let invalid_ticket = match failure {
            Some(JoinFailure::InvalidTicket(reason)) => Some(reason),
            _ => None,
        };
        Status {
            release_version: env!("CARGO_PKG_VERSION"),
            ipc_version: ipc_protocol::PROTOCOL_VERSION,
            peer_protocol_version: STREAM_PROTO_VERSION,
            os: OS,
            state,
            ticket: transport.our_ticket().to_string(),
            ticket_seq,
            ipc_port: self.ipc_port,
            self_check,
            game_connected: self.game_connected.load(Ordering::Relaxed),
            peers,
            banners,
            invalid_ticket,
        }
    }

    fn joins(&self) -> MutexGuard<'_, Joins> {
        lock(&self.joins)
    }

    /// The IPC server reports that a DLL handshake succeeded: the game is
    /// connected, and the DLL in the Game folder is the right one.
    pub(crate) fn dll_handshake_succeeded(&self) {
        self.game_connected.store(true, Ordering::Relaxed);
        self.ipc_version_mismatch.store(false, Ordering::Relaxed);
    }

    /// The IPC server reports a DLL that does not speak this Helper's IPC version.
    pub(crate) fn dll_mismatched(&self) {
        self.ipc_version_mismatch.store(true, Ordering::Relaxed);
    }

    /// The IPC server reports that the DLL's connection ended, whichever way.
    ///
    /// A game that crashed or was closed while in a session never sent
    /// CloseSession. Its session goes with it: kept, it would refuse the next
    /// Host Game, and the game would carry on into its setup screen with no
    /// session behind it.
    pub(crate) fn dll_disconnected(&self) {
        self.game_connected.store(false, Ordering::Relaxed);
        let transport = self.transport();
        if transport.session_manager().in_session() {
            info!("The game left without closing its session; closing it");
            transport.close_session();
        }
    }

    /// Join the Helper named by a friend's Ticket: check the Ticket, then dial
    /// it (blocking, for up to the Transport's dial timeout).
    ///
    /// This is the command line's join. The page's join is the same two
    /// halves, [`begin_join`](Self::begin_join) and [`dial`](Self::dial), with
    /// the answer to the page sent in between.
    pub fn join(&self, text: &str) -> Result<(), JoinError> {
        let pending = self.begin_join(text)?;
        self.dial(pending).map_err(JoinError::Dial)
    }

    /// Check the text given to a join, and if it is a friend's Ticket, mark a
    /// dial as in progress. Never waits on the Transport or the network, so
    /// it may be called from async code.
    ///
    /// The text is trimmed first, so stray spaces and line breaks around a
    /// pasted Ticket do no harm. Each join attempt clears the event banners
    /// of the one before it, and one refused as not a Ticket, or as our own,
    /// sets the `invalid_ticket` banner; one refused because a dial is
    /// already in progress changes nothing.
    ///
    /// The dial is not made here: run the result with [`dial`](Self::dial).
    pub(crate) fn begin_join(&self, text: &str) -> Result<PendingDial, JoinRefused> {
        let mut joins = self.joins();
        if joins.dialling {
            return Err(JoinRefused::DialInProgress);
        }
        let checked = self.check_ticket(text);
        joins.failure = checked
            .as_ref()
            .err()
            .and_then(JoinRefused::invalid_ticket)
            .map(JoinFailure::InvalidTicket);
        let ticket = checked?;
        let mut warnings = Vec::new();
        if ticket.addr().addrs.is_empty() {
            // It may still work, if the friend's Helper can be looked up by
            // its ID: dial it all the same.
            warn!("The Ticket to join carries no addresses");
            warnings.push(JoinWarning::TicketNoAddresses);
        }
        joins.dialling = true;
        Ok(PendingDial {
            ticket,
            warnings,
            joins: self.joins.clone(),
        })
    }

    /// The friend's Ticket in the text given to a join, trimmed.
    fn check_ticket(&self, text: &str) -> Result<Ticket, JoinRefused> {
        let ticket = Ticket::parse(text.trim()).map_err(JoinRefused::NotATicket)?;
        // Not through `transport()`: no reference to a Transport is taken,
        // so none can be the last one dropped on an async thread.
        if ticket.addr().id == self.current().transport.endpoint_id() {
            return Err(JoinRefused::OwnTicket);
        }
        Ok(ticket)
    }

    /// Dial the friend's Helper of a join that [`begin_join`](Self::begin_join)
    /// accepted (blocking, for up to the Transport's dial timeout).
    ///
    /// A dial that cannot reach the friend's Helper, or that it refuses for
    /// another release, puts up that event banner, unless Stop replaced the
    /// Transport it was made on meanwhile: the player pressed Stop, and the
    /// dial's end is no news to them.
    ///
    /// It waits on the Transport's runtime: from async code, call it inside
    /// `spawn_blocking`.
    pub(crate) fn dial(&self, pending: PendingDial) -> TransportResult<()> {
        let ticket = pending.ticket.serialize();
        info!("Connecting to host ticket: {}", ticket);
        let transport = self.transport();
        let dialled = transport.connect_to_peer(&ticket);
        match &dialled {
            Ok(()) => info!("Connected to host!"),
            Err(e) => warn!("Could not connect to host: {}", e),
        }
        {
            // Under the same lock as the end of the dial: status never sees
            // the dial over and how it went not yet recorded.
            let mut joins = self.joins();
            if self.is_current(&transport) {
                match &dialled {
                    Ok(()) => joins.dialled = true,
                    Err(e) => joins.failure = JoinFailure::of_dial(e),
                }
            }
            joins.dialling = false;
        }
        // Dropping it ends the dial, whichever way it went.
        drop(pending);
        dialled
    }

    /// Join the session hosted by the Helper named by `host_ticket`, for the
    /// game's DLL (blocking, for up to the Transport's dial timeout and the
    /// session exchange after it).
    ///
    /// To the page this is a join attempt like its own: it clears the event
    /// banners of the one before it, and a dial that cannot reach the host,
    /// or that the host refuses for another release, puts up the same banner
    /// as the page's dial would, unless Stop replaced the Transport meanwhile.
    /// Failing for any other reason, such as a host with no session yet, puts
    /// up none.
    pub(crate) fn join_session(&self, host_ticket: &str) -> TransportResult<DPID> {
        self.joins().failure = None;
        let transport = self.transport();
        let joined = transport.join_session_by_ticket(host_ticket);
        if let Err(e) = &joined {
            let mut joins = self.joins();
            if self.is_current(&transport) {
                joins.failure = JoinFailure::of_dial(e);
            }
        }
        joined
    }

    /// Stop: end the current connections and carry on with a new Transport,
    /// and so a new Ticket (blocking). Returns the new Ticket's sequence
    /// number.
    ///
    /// The new Transport is created first, so that if that fails nothing has
    /// changed, and so that the IPC server always finds a Transport: the
    /// game's link to the Helper survives Stop. The old Transport is then
    /// shut down gracefully, so the connected Helpers notice at once. Stop
    /// also clears the event banners, which were about joins made on the old
    /// one.
    ///
    /// Creating a Transport and shutting one down both wait on a Transport's
    /// runtime, and the old Transport's last reference may be dropped here:
    /// from async code, call it inside `spawn_blocking`. Takes up to
    /// `Transport::SHUTDOWN_TIMEOUT` once the new Transport is up.
    pub(crate) fn stop(&self) -> Result<u64, StopError> {
        let shut_down = self.shut_down.lock().unwrap_or_else(PoisonError::into_inner);
        if *shut_down {
            return Err(StopError::ShutDown);
        }
        info!("Stopping: creating a new transport");
        // No lock but the one above is held while this waits on the network:
        // status and the IPC server carry on with the old Transport.
        let transport = Transport::with_options(self.transport_options).map_err(|e| {
            warn!("Stop failed, nothing changed: {}", e);
            StopError::Transport(e)
        })?;
        info!("New transport. Endpoint ID: {:?}", transport.endpoint_id());
        let (old, ticket_seq) = {
            let mut joins = self.joins();
            let mut current = self.current();
            let old = std::mem::replace(&mut current.transport, Arc::new(transport));
            current.ticket_seq += 1;
            joins.failure = None;
            (old, current.ticket_seq)
        };
        old.shutdown();
        // Most likely the last reference, so the old Transport ends here, on
        // this thread. Otherwise a request still using it drops it after,
        // on the thread it runs on, which is never an async one either.
        drop(old);
        drop(shut_down);
        info!("Stopped. Ticket sequence number: {}", ticket_seq);
        Ok(ticket_seq)
    }

    /// Shut the Helper down: close every peer connection and the endpoint
    /// gracefully, so the connected Helpers notice at once (blocking).
    ///
    /// This is what Quit does, and it ends the Helper: whoever waits in
    /// [`wait_for_shutdown`](Self::wait_for_shutdown) is woken once the
    /// Transport is closed. Shutting down again does nothing.
    ///
    /// Takes up to `Transport::SHUTDOWN_TIMEOUT`, waiting on the Transport's
    /// runtime: from async code, call it inside `spawn_blocking`.
    pub fn shutdown(&self) {
        let mut shut_down = self.shut_down.lock().unwrap_or_else(PoisonError::into_inner);
        if *shut_down {
            return;
        }
        info!("Shutting down");
        // A clone, so that the Transport is not locked while it shuts down:
        // status reads it meanwhile.
        self.transport().shutdown();
        *shut_down = true;
        self.shut_down_signal.notify_all();
    }

    /// Block until the Helper has shut down.
    pub(crate) fn wait_for_shutdown(&self) {
        let mut shut_down = self.shut_down.lock().unwrap_or_else(PoisonError::into_inner);
        while !*shut_down {
            shut_down = self
                .shut_down_signal
                .wait(shut_down)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Where the Helper is in a session, given what it knows of its joins and
/// whether any Helper is connected right now.
///
/// Once no Helper is connected, a dial that succeeded earlier no longer
/// counts: the Helper is back to `ready`, and a friend who dials it next makes
/// it `hosting`. That is noticed here, when status is read, which the page
/// does every second; a friend who leaves and another who dials in between two
/// reads leave the Helper `joined`.
fn session_state(joins: &mut Joins, peer_connected: bool) -> State {
    if !peer_connected && !joins.dialling {
        joins.dialled = false;
    }
    match (joins.dialling, peer_connected, joins.dialled) {
        (true, _, _) => State::Joining,
        (false, false, _) => State::Ready,
        (false, true, true) => State::Joined,
        (false, true, false) => State::Hosting,
    }
}

const OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "linux"
};
