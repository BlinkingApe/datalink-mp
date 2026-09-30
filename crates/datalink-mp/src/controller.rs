//! Session controller: owns the Helper's current Transport and the status
//! the page shows.
//!
//! The IPC server and the command line both go through the controller, so
//! there is one place that knows which Transport is current. The IPC server
//! also reports here what it sees of the game's DLL. Shutting the controller
//! down closes the Transport for good, and is how the Helper ends.

use crate::platform::{self, SelfCheck};
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
    /// The most recent join was refused because its text is not a Ticket to
    /// dial: the `invalid_ticket` banner
    invalid_ticket: Option<InvalidTicket>,
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

/// Owns the current Transport, and therefore the Helper's Ticket.
pub struct SessionController {
    transport: Arc<Transport>,
    game_folder: PathBuf,
    ipc_port: u16,
    ipc_port_in_use: bool,
    game_connected: AtomicBool,
    ipc_version_mismatch: AtomicBool,
    /// Shared with the dial in progress, which ends itself: see [`PendingDial`].
    joins: Arc<Mutex<Joins>>,
    /// Whether the Helper has shut down. Held while it does, so that a second
    /// shutdown waits for the first instead of racing it.
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
            transport: Arc::new(transport),
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
        self.transport.clone()
    }

    /// A snapshot of the Helper's status. Nothing here waits on the Transport
    /// or the network.
    ///
    /// The Game folder self-check is done again for each snapshot, so a file
    /// the player restores is seen without a restart. It reads the folder's
    /// list of files: from async code, call this inside `spawn_blocking`.
    pub fn status(&self) -> Status {
        let transport = self.transport();
        // The peers are read with the joins locked: a dial that succeeded has
        // put its peer in the Transport's list before it records its success,
        // so what is read here always agrees with what the joins say.
        let mut joins = self.joins();
        // A plain lock read, unlike the Transport's blocking calls.
        let mut peers = transport
            .connected_peers()
            .iter()
            .map(|id| id.fmt_short().to_string())
            .collect::<Vec<_>>();
        let state = session_state(&mut joins, !peers.is_empty());
        let invalid_ticket = joins.invalid_ticket;
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
        if invalid_ticket.is_some() {
            banners.push(Banner::InvalidTicket);
        }
        Status {
            release_version: env!("CARGO_PKG_VERSION"),
            ipc_version: ipc_protocol::PROTOCOL_VERSION,
            peer_protocol_version: STREAM_PROTO_VERSION,
            os: OS,
            state,
            ticket: transport.our_ticket().to_string(),
            ticket_seq: FIRST_TICKET_SEQ,
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
    pub(crate) fn dll_disconnected(&self) {
        self.game_connected.store(false, Ordering::Relaxed);
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
    /// pasted Ticket do no harm. Each join attempt clears the
    /// `invalid_ticket` banner, and one refused as not a Ticket, or as our
    /// own, sets it again; one refused because a dial is already in progress
    /// changes nothing.
    ///
    /// The dial is not made here: run the result with [`dial`](Self::dial).
    pub(crate) fn begin_join(&self, text: &str) -> Result<PendingDial, JoinRefused> {
        let mut joins = self.joins();
        if joins.dialling {
            return Err(JoinRefused::DialInProgress);
        }
        let checked = self.check_ticket(text);
        joins.invalid_ticket = checked.as_ref().err().and_then(JoinRefused::invalid_ticket);
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
        // Through the field, not `transport()`: no reference to a Transport
        // is taken, so none can be the last one dropped on an async thread.
        if ticket.addr().id == self.transport.endpoint_id() {
            return Err(JoinRefused::OwnTicket);
        }
        Ok(ticket)
    }

    /// Dial the friend's Helper of a join that [`begin_join`](Self::begin_join)
    /// accepted (blocking, for up to the Transport's dial timeout).
    ///
    /// It waits on the Transport's runtime: from async code, call it inside
    /// `spawn_blocking`.
    pub(crate) fn dial(&self, pending: PendingDial) -> TransportResult<()> {
        let ticket = pending.ticket.serialize();
        info!("Connecting to host ticket: {}", ticket);
        let dialled = self.transport().connect_to_peer(&ticket);
        match &dialled {
            Ok(()) => {
                info!("Connected to host!");
                // Under the same lock as the end of the dial: status never
                // sees the dial over and its success not yet recorded.
                let mut joins = self.joins();
                joins.dialled = true;
                joins.dialling = false;
            }
            Err(e) => warn!("Could not connect to host: {}", e),
        }
        // Dropping it ends the dial, whichever way it went.
        drop(pending);
        dialled
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
        self.transport.shutdown();
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
