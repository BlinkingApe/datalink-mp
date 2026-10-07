//! IDirectPlay4 implementation

use crate::globals;
use crate::ipc_client::IpcClient;
use dp_types::*;
use datalink_ipc::SessionListEntry;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, debug_span, error, info, warn};

/// Monotonic counter for the inbound-message receive trace (see
/// `drain_helper_to_local_queue`). Numbers each `Receive()`-visible message so a
/// reorder shows up as `chunkIdx`/`hostSeq` stepping backwards across increasing #.
static RX_SEQ: AtomicU64 = AtomicU64::new(0);
/// Cached `DPLAYX_RXTRACE` presence; the receive trace is off unless it is set.
static RX_TRACE: OnceLock<bool> = OnceLock::new();

fn rx_trace_enabled() -> bool {
    *RX_TRACE.get_or_init(|| std::env::var_os("DPLAYX_RXTRACE").is_some())
}

// Windows kernel32 SetEvent function for signaling event handles
#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn SetEvent(hEvent: HANDLE) -> i32;
}

/// A wrapper for Windows HANDLE that is Send + Sync.
/// Windows handles are inherently thread-safe and can be used from any thread.
#[derive(Clone, Copy)]
struct SendHandle(HANDLE);

// SAFETY: Windows HANDLEs are thread-safe and can be used from any thread
unsafe impl Send for SendHandle {}
unsafe impl Sync for SendHandle {}

impl SendHandle {
    fn new(h: HANDLE) -> Self {
        SendHandle(h)
    }

    #[allow(dead_code)]
    fn get(&self) -> HANDLE {
        self.0
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

/// IDirectPlay4 vtable
static DIRECTPLAY4_VTBL: IDirectPlay4Vtbl = IDirectPlay4Vtbl {
    // IUnknown
    QueryInterface: DirectPlay_QueryInterface,
    AddRef: DirectPlay_AddRef,
    Release: DirectPlay_Release,
    // IDirectPlay2
    AddPlayerToGroup: DirectPlay_AddPlayerToGroup,
    Close: DirectPlay_Close,
    CreateGroup: DirectPlay_CreateGroup,
    CreatePlayer: DirectPlay_CreatePlayer,
    DeletePlayerFromGroup: DirectPlay_DeletePlayerFromGroup,
    DestroyGroup: DirectPlay_DestroyGroup,
    DestroyPlayer: DirectPlay_DestroyPlayer,
    EnumGroupPlayers: DirectPlay_EnumGroupPlayers,
    EnumGroups: DirectPlay_EnumGroups,
    EnumPlayers: DirectPlay_EnumPlayers,
    EnumSessions: DirectPlay_EnumSessions,
    GetCaps: DirectPlay_GetCaps,
    GetGroupData: DirectPlay_GetGroupData,
    GetGroupName: DirectPlay_GetGroupName,
    GetMessageCount: DirectPlay_GetMessageCount,
    GetPlayerAddress: DirectPlay_GetPlayerAddress,
    GetPlayerCaps: DirectPlay_GetPlayerCaps,
    GetPlayerData: DirectPlay_GetPlayerData,
    GetPlayerName: DirectPlay_GetPlayerName,
    GetSessionDesc: DirectPlay_GetSessionDesc,
    Initialize: DirectPlay_Initialize,
    Open: DirectPlay_Open,
    Receive: DirectPlay_Receive,
    Send: DirectPlay_Send,
    SetGroupData: DirectPlay_SetGroupData,
    SetGroupName: DirectPlay_SetGroupName,
    SetPlayerData: DirectPlay_SetPlayerData,
    SetPlayerName: DirectPlay_SetPlayerName,
    SetSessionDesc: DirectPlay_SetSessionDesc,
    // IDirectPlay3
    AddGroupToGroup: DirectPlay_AddGroupToGroup,
    CreateGroupInGroup: DirectPlay_CreateGroupInGroup,
    DeleteGroupFromGroup: DirectPlay_DeleteGroupFromGroup,
    EnumConnections: DirectPlay_EnumConnections,
    EnumGroupsInGroup: DirectPlay_EnumGroupsInGroup,
    GetGroupConnectionSettings: DirectPlay_GetGroupConnectionSettings,
    InitializeConnection: DirectPlay_InitializeConnection,
    SecureOpen: DirectPlay_SecureOpen,
    SendChatMessage: DirectPlay_SendChatMessage,
    SetGroupConnectionSettings: DirectPlay_SetGroupConnectionSettings,
    StartSession: DirectPlay_StartSession,
    GetGroupFlags: DirectPlay_GetGroupFlags,
    GetGroupParent: DirectPlay_GetGroupParent,
    GetPlayerAccount: DirectPlay_GetPlayerAccount,
    GetPlayerFlags: DirectPlay_GetPlayerFlags,
    // IDirectPlay4
    GetGroupOwner: DirectPlay_GetGroupOwner,
    SetGroupOwner: DirectPlay_SetGroupOwner,
    SendEx: DirectPlay_SendEx,
    GetMessageQueue: DirectPlay_GetMessageQueue,
    CancelMessage: DirectPlay_CancelMessage,
    CancelPriority: DirectPlay_CancelPriority,
};

/// State of the DirectPlay object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DPState {
    /// Just created, not initialized
    Uninitialized,
    /// Initialized with service provider
    Initialized,
    /// Session opened (hosting or joined)
    InSession,
    /// Closed
    Closed,
}

/// DirectPlay object
/// Encoded session name with stable heap storage.
///
/// SMAC's EnumSessions callback (SessionUIUpdate) **stores the `lpszSessionName`
/// pointer** rather than copying the string, then reads it later when it renders
/// the join list and when it calls Open on the selected session. So the name
/// buffer must outlive the EnumSessions call. Real DirectPlay keeps its internal
/// session cache alive until the next enumeration; we mirror that by parking these
/// buffers in the DirectPlayObject (`enum_name_cache`), replaced each EnumSessions.
enum EncodedSessionName {
    Wide(Vec<u16>),
    Ansi(Vec<i8>),
}

impl EncodedSessionName {
    fn as_ptr(&self) -> *mut u16 {
        match self {
            EncodedSessionName::Wide(v) => v.as_ptr() as *mut u16,
            // Caller interprets the bytes as ANSI based on interface type.
            EncodedSessionName::Ansi(v) => v.as_ptr() as *mut u16,
        }
    }
}

#[repr(C)]
pub struct DirectPlayObject {
    /// Vtable pointer (must be first for COM compatibility)
    vtbl: *const IDirectPlay4Vtbl,
    /// Reference count
    ref_count: AtomicU32,
    /// Object state
    state: Mutex<DPState>,
    /// Selected service provider GUID
    sp_guid: Mutex<Option<GUID>>,
    /// Pending messages (for Receive)
    messages: Mutex<VecDeque<PendingMessage>>,
    /// Is this the Unicode version?
    is_unicode: bool,
    /// Discovered sessions (from EnumSessions) for later lookup when joining
    discovered_sessions: Mutex<Vec<SessionListEntry>>,
    /// Event handles for players (player_id -> event handle)
    /// When messages arrive for a player, we signal their event
    player_events: Arc<Mutex<HashMap<DPID, SendHandle>>>,
    /// Flag to stop the polling thread
    poll_stop: Arc<AtomicBool>,
    /// Background thread handle for message polling
    poll_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Next group ID to allocate (start at 0x40000000 to avoid collision with player IDs)
    next_group_id: AtomicU32,
    /// Groups: group_id -> set of player IDs
    groups: Mutex<HashMap<DPID, HashSet<DPID>>>,
    /// Group data: group_id -> (local_data, remote_data)
    group_data: Mutex<HashMap<DPID, (Vec<u8>, Vec<u8>)>>,
    /// Stable storage for the session-name buffers handed to the most recent
    /// EnumSessions callback. SMAC keeps the pointer past the callback, so these
    /// must outlive the EnumSessions call; replaced on each enumeration (like real
    /// DirectPlay's session cache). Without this the game reads a freed buffer and
    /// shows a blank session name.
    enum_name_cache: Mutex<Vec<EncodedSessionName>>,
}

/// A pending message
#[derive(Debug, Clone)]
struct PendingMessage {
    from: DPID,
    to: DPID,
    data: Vec<u8>,
}

impl DirectPlayObject {
    pub fn new() -> Result<Self, std::io::Error> {
        info!("DirectPlayObject::new() ENTER");
        let count = globals::inc_object_count();
        info!("DirectPlayObject::new() object count now {}", count);

        // NOTE: IPC connection is created lazily when needed (EnumSessions, Open, etc.)
        info!("DirectPlayObject::new() deferring IPC connection");

        let obj = Self {
            vtbl: &DIRECTPLAY4_VTBL,
            ref_count: AtomicU32::new(1),
            state: Mutex::new(DPState::Uninitialized),
            sp_guid: Mutex::new(None),
            messages: Mutex::new(VecDeque::new()),
            is_unicode: true,
            discovered_sessions: Mutex::new(Vec::new()),
            player_events: Arc::new(Mutex::new(HashMap::new())),
            poll_stop: Arc::new(AtomicBool::new(false)),
            poll_thread: Mutex::new(None),
            // Group support: IDs start at 0x40000000 to avoid collision with player IDs
            next_group_id: AtomicU32::new(0x40000000),
            groups: Mutex::new(HashMap::new()),
            group_data: Mutex::new(HashMap::new()),
            enum_name_cache: Mutex::new(Vec::new()),
        };
        info!("DirectPlayObject::new() EXIT vtbl={:?}", obj.vtbl);
        Ok(obj)
    }

    /// Initialize with a service provider GUID (called from DirectPlayCreate)
    pub fn initialize_with_guid(&mut self, guid: GUID) {
        info!("DirectPlayObject::initialize_with_guid({:?})", guid);
        *self.sp_guid.lock() = Some(guid);
        *self.state.lock() = DPState::Initialized;
    }

    fn get_ipc_client(&self) -> Option<parking_lot::MutexGuard<'static, Option<IpcClient>>> {
        let guard = globals::get_ipc_client();
        if guard.is_some() {
            Some(guard)
        } else {
            None
        }
    }

    /// Store a discovered session (from EnumSessions) for later lookup when joining
    fn store_discovered_session(&self, session: SessionListEntry) {
        let mut sessions = self.discovered_sessions.lock();
        // Replace if same guid_instance, otherwise add
        if let Some(pos) = sessions.iter().position(|s| s.guid_instance == session.guid_instance) {
            sessions[pos] = session;
        } else {
            sessions.push(session);
        }
    }

    /// Get a discovered session by its instance GUID
    fn get_discovered_session(&self, guid_instance: &GUID) -> Option<SessionListEntry> {
        self.discovered_sessions.lock()
            .iter()
            .find(|s| s.guid_instance == *guid_instance)
            .cloned()
    }

    /// Start the message polling thread if not already running
    fn start_message_poll_thread(&self) {
        let mut guard = self.poll_thread.lock();
        if guard.is_some() {
            debug!("Polling thread already running");
            return;
        }

        info!("Starting message polling thread");

        let stop_flag = self.poll_stop.clone();
        let events = self.player_events.clone();

        // Reset stop flag
        stop_flag.store(false, Ordering::SeqCst);

        let handle = std::thread::spawn(move || {
            debug!("Polling thread started");

            while !stop_flag.load(Ordering::SeqCst) {
                // Check message count via IPC
                let should_signal = {
                    let guard = globals::get_ipc_client();
                    if let Some(ref client) = *guard {
                        client.message_count() > 0
                    } else {
                        false
                    }
                };

                if should_signal {
                    // Signal all player events
                    let events_guard = events.lock();
                    for (_player_id, event) in events_guard.iter() {
                        if !event.is_null() {
                            debug!(player_id = _player_id, "Signaling player event");
                            #[cfg(target_os = "windows")]
                            unsafe {
                                SetEvent(event.get());
                            }
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(10));
            }

            debug!("Polling thread stopped");
        });

        *guard = Some(handle);
    }

    /// Stop the message polling thread
    fn stop_poll_thread(&self) {
        info!("Stopping message polling thread");
        self.poll_stop.store(true, Ordering::SeqCst);

        // Take the thread handle and actually wait for it to finish
        if let Some(handle) = self.poll_thread.lock().take() {
            // Join the thread to ensure clean shutdown
            // This prevents crashes on DLL unload
            if let Err(e) = handle.join() {
                warn!("Polling thread panicked: {:?}", e);
            } else {
                info!("Polling thread stopped cleanly");
            }
        }
    }

    /// Register a player's event handle
    fn register_player_event(&self, player_id: DPID, event: HANDLE) {
        if event.is_null() {
            return;
        }
        info!(player_id, event = ?event, "Registering player event handle");
        self.player_events.lock().insert(player_id, SendHandle::new(event));
        self.start_message_poll_thread();
    }

    /// Unregister a player's event handle
    fn unregister_player_event(&self, player_id: DPID) {
        info!(player_id, "Unregistering player event handle");
        self.player_events.lock().remove(&player_id);

        // Stop polling if no more events
        if self.player_events.lock().is_empty() {
            self.stop_poll_thread();
        }
    }

    /// Drain all pending messages from the IPC helper into the local queue.
    /// This ensures GetMessageCount returns an accurate count.
    ///
    /// This is the single choke point where every inbound message enters the
    /// game-visible queue, in the exact order the game will `Receive()` them. When
    /// `DPLAYX_RXTRACE` is set we log each message's sync header fields here, giving
    /// ground-truth arrival order for the multi-chunk syncs (0x2101/0x4101). For a
    /// given `syncType`, `chunkIndex` should be monotonic and `gameStateValue` (the
    /// host sequence stamped at rebroadcast) should be non-decreasing; a backwards
    /// step is a transport-induced reorder — the thing we suspect can hang the
    /// joiner's reassembly. Zero overhead when the env var is unset.
    fn drain_helper_to_local_queue(&self) {
        if let Some(client) = self.get_ipc_client() {
            if let Some(c) = client.as_ref() {
                let mut count = 0;
                while let Some(msg) = c.receive() {
                    if rx_trace_enabled() {
                        let seq = RX_SEQ.fetch_add(1, Ordering::Relaxed);
                        let d = &msg.data;
                        let u16le = |o: usize| -> u16 {
                            if d.len() >= o + 2 { u16::from_le_bytes([d[o], d[o + 1]]) } else { 0 }
                        };
                        let u32le = |o: usize| -> u32 {
                            if d.len() >= o + 4 {
                                u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
                            } else { 0 }
                        };
                        info!(
                            "RXTRACE #{} from={:#x} to={:#x} size={} type={:#06x} hostSeq={} syncType={:#x} chunkIdx={} dataOff={}",
                            seq, msg.from, msg.to, d.len(),
                            u16le(0), u32le(8), u16le(16), u16le(18), u32le(32)
                        );
                    }
                    self.messages.lock().push_back(PendingMessage {
                        from: msg.from,
                        to: msg.to,
                        data: msg.data,
                    });
                    count += 1;
                }
                if count > 0 {
                    debug!("Drained {} messages from helper to local queue", count);
                }
            }
        }
    }

    /// Queue a local system message and signal player events.
    /// Used when local operations (CreateGroup, AddPlayerToGroup, etc.) need to
    /// generate system messages that the game expects to receive via Receive().
    fn queue_local_system_message(&self, msg_data: Vec<u8>) {
        debug!("Queueing local system message, size={}", msg_data.len());
        self.messages.lock().push_back(PendingMessage {
            from: 0, // DPID_SYSMSG
            to: 0,   // DPID_ALLPLAYERS
            data: msg_data,
        });
        // Signal all player events so game knows to call Receive()
        self.signal_all_player_events();
    }

    /// Signal all registered player event handles.
    fn signal_all_player_events(&self) {
        let events_guard = self.player_events.lock();
        for (_player_id, event) in events_guard.iter() {
            if !event.is_null() {
                debug!(player_id = _player_id, "Signaling player event for local system message");
                #[cfg(target_os = "windows")]
                unsafe {
                    SetEvent(event.get());
                }
            }
        }
    }
}

impl Drop for DirectPlayObject {
    fn drop(&mut self) {
        debug!("DirectPlayObject dropped");
        // Stop the polling thread before dropping
        self.stop_poll_thread();
        self.player_events.lock().clear();
        globals::dec_object_count();
    }
}

// Helper to get DirectPlayObject from IDirectPlay4 pointer
unsafe fn get_dp(this: *mut IDirectPlay4) -> Option<&'static DirectPlayObject> {
    if this.is_null() {
        return None;
    }
    Some(&*(this as *const DirectPlayObject))
}

unsafe fn get_dp_mut(this: *mut IDirectPlay4) -> Option<&'static mut DirectPlayObject> {
    if this.is_null() {
        return None;
    }
    Some(&mut *(this as *mut DirectPlayObject))
}

// ============================================================================
// IUnknown Implementation
// ============================================================================

unsafe extern "system" fn DirectPlay_QueryInterface(
    this: *mut IDirectPlay4,
    riid: *const GUID,
    ppvObject: *mut LPVOID,
) -> HRESULT {
    info!("DirectPlay::QueryInterface ENTER this={:?} riid={:?}", this, riid);

    if this.is_null() || riid.is_null() || ppvObject.is_null() {
        info!("DirectPlay::QueryInterface EXIT E_INVALIDARG (null param)");
        return E_INVALIDARG;
    }

    let iid = &*riid;
    info!("DirectPlay::QueryInterface iid={:?}", iid);

    // Check for ANSI interfaces (ending with 'A')
    let is_ansi = *iid == IID_IDIRECTPLAY2A
        || *iid == IID_IDIRECTPLAY3A
        || *iid == IID_IDIRECTPLAY4A;

    // Support all DirectPlay interfaces
    if *iid == IID_IUNKNOWN
        || *iid == IID_IDIRECTPLAY2
        || *iid == IID_IDIRECTPLAY2A
        || *iid == IID_IDIRECTPLAY3
        || *iid == IID_IDIRECTPLAY3A
        || *iid == IID_IDIRECTPLAY4
        || *iid == IID_IDIRECTPLAY4A
    {
        // Update unicode flag based on interface type
        if let Some(dp) = get_dp_mut(this) {
            dp.is_unicode = !is_ansi;
            info!("DirectPlay::QueryInterface is_unicode={}", dp.is_unicode);
        }

        DirectPlay_AddRef(this);
        *ppvObject = this as LPVOID;
        info!("DirectPlay::QueryInterface EXIT S_OK");
        return S_OK;
    }

    *ppvObject = std::ptr::null_mut();
    info!("DirectPlay::QueryInterface EXIT E_NOINTERFACE");
    E_NOINTERFACE
}

unsafe extern "system" fn DirectPlay_AddRef(this: *mut IDirectPlay4) -> DWORD {
    info!("DirectPlay::AddRef ENTER this={:?}", this);
    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => {
            info!("DirectPlay::AddRef EXIT 0 (null this)");
            return 0;
        }
    };

    let count = dp.ref_count.fetch_add(1, Ordering::SeqCst) + 1;
    info!("DirectPlay::AddRef EXIT count={}", count);
    count
}

unsafe extern "system" fn DirectPlay_Release(this: *mut IDirectPlay4) -> DWORD {
    info!("DirectPlay::Release ENTER this={:?}", this);

    if this.is_null() {
        info!("DirectPlay::Release EXIT 0 (null this)");
        return 0;
    }

    // Get the object pointer once - we'll use it for the atomic op
    let dp = &*(this as *const DirectPlayObject);

    // Fetch the old count and decrement atomically
    let old_count = dp.ref_count.fetch_sub(1, Ordering::SeqCst);
    let new_count = old_count - 1;
    info!("DirectPlay::Release count={}", new_count);

    if new_count == 0 {
        // Drop must be the LAST operation once the count reaches 0;
        // after this, the 'dp' reference is invalid.
        info!("DirectPlay::Release dropping object");
        drop(Box::from_raw(this as *mut DirectPlayObject));
    }

    info!("DirectPlay::Release EXIT count={}", new_count);
    new_count
}

// ============================================================================
// IDirectPlay2 Implementation
// ============================================================================

unsafe extern "system" fn DirectPlay_AddPlayerToGroup(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    idPlayer: DPID,
) -> HRESULT {
    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let mut groups = dp.groups.lock();
    if let Some(members) = groups.get_mut(&idGroup) {
        members.insert(idPlayer);
        info!(
            "AddPlayerToGroup: added player {:#x} to group {:#x}",
            idPlayer, idGroup
        );
        // Must drop lock before queueing message (avoids potential deadlock)
        drop(groups);

        // Queue DPMSG_ADDPLAYERTOGROUP system message
        let sysmsg = serialize_dpmsg_addplayertogroup(idGroup, idPlayer);
        dp.queue_local_system_message(sysmsg);

        DP_OK
    } else {
        warn!(
            "AddPlayerToGroup: group {:#x} not found",
            idGroup
        );
        DPERR_INVALIDGROUP
    }
}

unsafe extern "system" fn DirectPlay_Close(this: *mut IDirectPlay4) -> HRESULT {
    debug!("DirectPlay::Close");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let mut state = dp.state.lock();
    if *state == DPState::Closed {
        return DPERR_UNINITIALIZED;
    }

    // Stop the message polling thread and clear all player events
    dp.stop_poll_thread();
    dp.player_events.lock().clear();

    // Close the session via IPC
    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            c.close_session();
        }
    }

    *state = DPState::Closed;
    DP_OK
}

unsafe extern "system" fn DirectPlay_CreateGroup(
    this: *mut IDirectPlay4,
    lpidGroup: *mut DPID,
    lpGroupName: *const DPNAME,
    lpData: LPVOID,
    dwDataSize: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpidGroup.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Allocate a new group ID
    let group_id = dp.next_group_id.fetch_add(1, Ordering::SeqCst);

    // Create empty group
    dp.groups.lock().insert(group_id, HashSet::new());

    *lpidGroup = group_id;
    info!("CreateGroup: created group {:#x}", group_id);

    // Queue DPMSG_CREATEPLAYERORGROUP system message so game receives notification
    let group_name = if !lpGroupName.is_null() {
        let dpname = &*lpGroupName;
        if !dpname.lpsz_short_name.is_null() {
            let cstr = std::ffi::CStr::from_ptr(dpname.lpsz_short_name as *const i8);
            cstr.to_string_lossy().into_owned()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let name = PlayerName {
        short_name: group_name,
        long_name: String::new(),
    };

    let data_slice = if !lpData.is_null() && dwDataSize > 0 {
        std::slice::from_raw_parts(lpData as *const u8, dwDataSize as usize)
    } else {
        &[]
    };

    let sysmsg = serialize_dpmsg_createplayerorgroup(
        group_id,
        DPPLAYERTYPE_GROUP,
        0, // current_players (group starts empty)
        &name,
        data_slice,
        dwFlags,
    );
    dp.queue_local_system_message(sysmsg);

    DP_OK
}

unsafe extern "system" fn DirectPlay_CreatePlayer(
    this: *mut IDirectPlay4,
    lpidPlayer: *mut DPID,
    lpPlayerName: *const DPNAME,
    hEvent: HANDLE,
    lpData: LPVOID,
    dwDataSize: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!(
        hEvent = ?hEvent,
        dwFlags = dwFlags,
        "DirectPlay::CreatePlayer"
    );

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpidPlayer.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    let state = *dp.state.lock();
    if state != DPState::InSession {
        return DPERR_NOSESSIONS;
    }

    // Get player name with correct encoding
    let name = if lpPlayerName.is_null() {
        PlayerName::default()
    } else if dp.is_unicode {
        PlayerName::from_raw(&*lpPlayerName)
    } else {
        // ANSI interface - interpret as DPNAME_A
        PlayerName::from_raw_ansi(&*(lpPlayerName as *const structs::DPNAME_A))
    };

    // Read player data if provided
    let data = if lpData.is_null() || dwDataSize == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(lpData as *const u8, dwDataSize as usize).to_vec()
    };

    // Create player via IPC
    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            if let Some(player_id) = c.create_player(name, dwFlags, data) {
                *lpidPlayer = player_id;
                info!("Created player with ID {}", player_id);

                // Register event handle for async message notification
                if !hEvent.is_null() {
                    dp.register_player_event(player_id, hEvent);
                }

                return DP_OK;
            }
        }
    }

    DPERR_CANTCREATEPLAYER
}

unsafe extern "system" fn DirectPlay_DeletePlayerFromGroup(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    idPlayer: DPID,
) -> HRESULT {
    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let mut groups = dp.groups.lock();
    if let Some(members) = groups.get_mut(&idGroup) {
        members.remove(&idPlayer);
        info!(
            "DeletePlayerFromGroup: removed player {:#x} from group {:#x}",
            idPlayer, idGroup
        );
        // Must drop lock before queueing message
        drop(groups);

        // Queue DPMSG_DELETEPLAYERFROMGROUP system message
        let sysmsg = serialize_dpmsg_deleteplayerfromgroup(idGroup, idPlayer);
        dp.queue_local_system_message(sysmsg);

        DP_OK
    } else {
        warn!("DeletePlayerFromGroup: group {:#x} not found", idGroup);
        DPERR_INVALIDGROUP
    }
}

unsafe extern "system" fn DirectPlay_DestroyGroup(
    this: *mut IDirectPlay4,
    idGroup: DPID,
) -> HRESULT {
    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let mut groups = dp.groups.lock();
    if groups.remove(&idGroup).is_some() {
        info!("DestroyGroup: destroyed group {:#x}", idGroup);
        // Must drop lock before queueing message
        drop(groups);

        // Queue DPMSG_DESTROYPLAYERORGROUP system message
        let name = PlayerName {
            short_name: String::new(),
            long_name: String::new(),
        };
        let sysmsg = serialize_dpmsg_destroyplayerorgroup(
            idGroup,
            DPPLAYERTYPE_GROUP,
            &name,
            0, // flags
        );
        dp.queue_local_system_message(sysmsg);

        DP_OK
    } else {
        warn!("DestroyGroup: group {:#x} not found", idGroup);
        DPERR_INVALIDGROUP
    }
}

unsafe extern "system" fn DirectPlay_DestroyPlayer(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
) -> HRESULT {
    debug!("DirectPlay::DestroyPlayer({})", idPlayer);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    // Per Wine: remove player from all groups before destroying
    {
        let mut groups = dp.groups.lock();
        for (_group_id, members) in groups.iter_mut() {
            members.remove(&idPlayer);
        }
    }

    // Unregister event handle for this player
    dp.unregister_player_event(idPlayer);

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            if c.destroy_player(idPlayer) {
                return DP_OK;
            }
        }
    }

    DPERR_INVALIDPLAYER
}

unsafe extern "system" fn DirectPlay_EnumGroupPlayers(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    _lpguidInstance: *const GUID,
    lpEnumPlayersCallback2: LPDPENUMPLAYERSCALLBACK2,
    lpContext: LPVOID,
    _dwFlags: DWORD,
) -> HRESULT {
    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let callback = match lpEnumPlayersCallback2 {
        Some(cb) => cb,
        None => return DPERR_INVALIDPARAMS,
    };

    // Copy member list to avoid holding lock during callbacks
    let members: Vec<DPID> = {
        let groups = dp.groups.lock();
        match groups.get(&idGroup) {
            Some(m) => m.iter().copied().collect(),
            None => {
                warn!("EnumGroupPlayers: group {:#x} not found", idGroup);
                return DPERR_INVALIDGROUP;
            }
        }
    };

    info!(
        "EnumGroupPlayers: enumerating {} players in group {:#x}",
        members.len(),
        idGroup
    );

    // Get player info from IPC to provide actual names
    let players = if let Some(guard) = dp.get_ipc_client() {
        if let Some(client) = guard.as_ref() {
            client.get_players()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Call callback for each player
    for player_id in members {
        // Find player info for this member
        let player_info = players.iter().find(|p| p.player_id == player_id);

        // Build DPNAME with actual player name if available
        let short_name_wide;
        let short_name_ansi;
        let long_name_wide;
        let long_name_ansi;

        let (short_ptr, long_ptr, player_flags) = if let Some(player) = player_info {
            if dp.is_unicode {
                short_name_wide = structs::string_to_wide(&player.name.short_name);
                long_name_wide = structs::string_to_wide(&player.name.long_name);
                (
                    short_name_wide.as_ptr() as *mut u16,
                    long_name_wide.as_ptr() as *mut u16,
                    player.flags,
                )
            } else {
                short_name_ansi = structs::string_to_ansi(&player.name.short_name);
                long_name_ansi = structs::string_to_ansi(&player.name.long_name);
                (
                    short_name_ansi.as_ptr() as *mut u16,
                    long_name_ansi.as_ptr() as *mut u16,
                    player.flags,
                )
            }
        } else {
            // Player not found in IPC, use empty name
            (std::ptr::null_mut(), std::ptr::null_mut(), 0)
        };

        let name = DPNAME {
            dw_size: std::mem::size_of::<DPNAME>() as DWORD,
            dw_flags: 0,
            lpsz_short_name: short_ptr,
            lpsz_long_name: long_ptr,
        };

        let result = callback(
            player_id,
            DPPLAYERTYPE_PLAYER, // Correct player type constant
            &name,
            player_flags,
            lpContext,
        );
        if result == 0 {
            // Callback returned FALSE, stop enumeration
            break;
        }
    }

    DP_OK
}

unsafe extern "system" fn DirectPlay_EnumGroups(
    this: *mut IDirectPlay4,
    lpguidInstance: *const GUID,
    lpEnumPlayersCallback2: LPDPENUMPLAYERSCALLBACK2,
    lpContext: LPVOID,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::EnumGroups");
    // No groups - just return OK
    DP_OK
}

unsafe extern "system" fn DirectPlay_EnumPlayers(
    this: *mut IDirectPlay4,
    lpguidInstance: *const GUID,
    lpEnumPlayersCallback2: LPDPENUMPLAYERSCALLBACK2,
    lpContext: LPVOID,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::EnumPlayers");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let callback = match lpEnumPlayersCallback2 {
        Some(cb) => cb,
        None => return DPERR_INVALIDPARAMS,
    };

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            let players = c.get_players();
            for player in players {
                // Filter based on dwFlags per Wine implementation
                // DPENUMPLAYERS_LOCAL (0x08) - only local players
                // DPENUMPLAYERS_REMOTE (0x10) - only remote players
                if (dwFlags & DPENUMPLAYERS_LOCAL) != 0 && !player.is_local {
                    continue; // Skip remote players when LOCAL requested
                }
                if (dwFlags & DPENUMPLAYERS_REMOTE) != 0 && player.is_local {
                    continue; // Skip local players when REMOTE requested
                }

                // Build DPNAME with correct encoding based on interface type
                // Declare all storage variables at this scope to keep them alive through callback
                let short_name_wide;
                let short_name_ansi;
                let long_name_wide;
                let long_name_ansi;

                let (short_ptr, long_ptr) = if dp.is_unicode {
                    short_name_wide = structs::string_to_wide(&player.name.short_name);
                    long_name_wide = structs::string_to_wide(&player.name.long_name);
                    (short_name_wide.as_ptr() as *mut u16, long_name_wide.as_ptr() as *mut u16)
                } else {
                    short_name_ansi = structs::string_to_ansi(&player.name.short_name);
                    long_name_ansi = structs::string_to_ansi(&player.name.long_name);
                    (short_name_ansi.as_ptr() as *mut u16, long_name_ansi.as_ptr() as *mut u16)
                };

                let name = DPNAME {
                    dw_size: std::mem::size_of::<DPNAME>() as DWORD,
                    dw_flags: 0,
                    lpsz_short_name: short_ptr,
                    lpsz_long_name: long_ptr,
                };

                // Per Wine: dwPlayerType is DPPLAYERTYPE_PLAYER (0x1) for players
                // NOT DPPLAYER_LOCAL (0x08) which is a filter flag
                let player_type = DPPLAYERTYPE_PLAYER;

                let cont = callback(player.player_id, player_type, &name, player.flags, lpContext);

                if cont == 0 {
                    break;
                }
            }
        }
    }

    DP_OK
}

unsafe extern "system" fn DirectPlay_EnumSessions(
    this: *mut IDirectPlay4,
    lpsd: *const DPSESSIONDESC2,
    dwTimeout: DWORD,
    lpEnumSessionsCallback2: LPDPENUMSESSIONSCALLBACK2,
    lpContext: LPVOID,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::EnumSessions(timeout={}, flags={:#x})", dwTimeout, dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let callback = match lpEnumSessionsCallback2 {
        Some(cb) => cb,
        None => return DPERR_INVALIDPARAMS,
    };

    let app_guid = if lpsd.is_null() {
        GUID::zeroed()
    } else {
        (*lpsd).guid_application
    };

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            let sessions = c.enum_sessions(&app_guid, dwTimeout);
            info!("EnumSessions: got {} sessions, is_unicode={}", sessions.len(), dp.is_unicode);

            // Pre-encode all session names and PARK them in the DirectPlayObject so
            // they outlive this call. SMAC stores the name pointer from the callback
            // and reads it later (join-list render, Open); a buffer freed when
            // EnumSessions returns would leave the game reading freed memory and
            // showing a blank session name. Replaced each enumeration, like real DP.
            let encoded_names: Vec<EncodedSessionName> = sessions.iter().map(|s| {
                if dp.is_unicode {
                    EncodedSessionName::Wide(structs::string_to_wide(&s.session_name))
                } else {
                    EncodedSessionName::Ansi(structs::string_to_ansi(&s.session_name))
                }
            }).collect();

            let mut name_cache = dp.enum_name_cache.lock();
            *name_cache = encoded_names;

            for (session_info, encoded_name) in sessions.iter().zip(name_cache.iter()) {
                // Store session for later lookup when joining
                dp.store_discovered_session(session_info.clone());

                let session_name_ptr = encoded_name.as_ptr();
                info!("EnumSessions: session '{}' name_ptr={:?}", session_info.session_name, session_name_ptr);

                let sd = DPSESSIONDESC2 {
                    dw_size: std::mem::size_of::<DPSESSIONDESC2>() as DWORD,
                    dw_flags: session_info.flags,
                    guid_instance: session_info.guid_instance,
                    guid_application: session_info.guid_application,
                    dw_max_players: session_info.max_players,
                    dw_current_players: session_info.current_players,
                    lpsz_session_name: session_name_ptr,
                    lpsz_password: std::ptr::null_mut(),
                    dw_reserved1: 0,
                    dw_reserved2: 0,
                    dw_user1: 0,
                    dw_user2: 0,
                    dw_user3: 0,
                    dw_user4: 0,
                };

                let mut timeout = dwTimeout;
                // Per Wine: per-session callbacks get flags=0, NOT the original dwFlags
                // Only the final callback gets DPESC_TIMEDOUT
                let cont = callback(&sd, &mut timeout, 0, lpContext);

                if cont == 0 {
                    break;
                }
            }
        }
    }

    // Final callback with NULL to signal end - per Wine, flags MUST be DPESC_TIMEDOUT
    let mut timeout = 0;
    callback(std::ptr::null(), &mut timeout, 0x00000001, lpContext); // DPESC_TIMEDOUT

    DP_OK
}

unsafe extern "system" fn DirectPlay_GetCaps(
    this: *mut IDirectPlay4,
    lpDPCaps: *mut DPCAPS,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetCaps(flags={:#x})", dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpDPCaps.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Validate dwSize (Wine requires this)
    let expected_size = std::mem::size_of::<DPCAPS>() as DWORD;
    if (*lpDPCaps).dw_size != expected_size {
        debug!("GetCaps: invalid dwSize {} (expected {})", (*lpDPCaps).dw_size, expected_size);
        return DPERR_INVALIDPARAMS;
    }

    // Query host status via IPC
    let is_host = dp
        .get_ipc_client()
        .and_then(|guard| guard.as_ref().map(|c| c.is_host()))
        .unwrap_or(false);

    // Build capability flags
    // Per Windows dplayx.dll: when GUARANTEEDSUPPORTED is set, also set GUARANTEEDOPTIMIZED
    // Note: KEEPALIVEOPTIMIZED was not found being set by the original DLL
    let mut flags: DWORD = DPCAPS_GUARANTEEDSUPPORTED | DPCAPS_GUARANTEEDOPTIMIZED;
    if is_host {
        flags |= DPCAPS_ISHOST;
    }
    debug!("GetCaps: is_host={}, flags={:#x}", is_host, flags);

    // Fill in the caps structure
    (*lpDPCaps).dw_flags = flags;
    (*lpDPCaps).dw_max_buffer_size = 65536;
    (*lpDPCaps).dw_max_queue_size = 256;
    (*lpDPCaps).dw_max_players = 16;
    (*lpDPCaps).dw_hundred_baud = 10000; // ~1 Mbps
    (*lpDPCaps).dw_latency = 50;         // 50ms estimated
    (*lpDPCaps).dw_max_local_players = 1;
    (*lpDPCaps).dw_header_length = 64;
    (*lpDPCaps).dw_timeout = 5000; // 5 second timeout

    DP_OK
}

unsafe extern "system" fn DirectPlay_GetGroupData(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetGroupData({}, flags={:#x})", idGroup, dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    // Check group exists
    if !dp.groups.lock().contains_key(&idGroup) {
        debug!("GetGroupData: group {} not found", idGroup);
        return DPERR_INVALIDGROUP;
    }

    if lpdwDataSize.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    let group_data = dp.group_data.lock();
    let is_local = (dwFlags & DPGET_LOCAL) != 0;
    let data = group_data
        .get(&idGroup)
        .map(|(local, remote)| if is_local { local } else { remote })
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let needed = data.len() as DWORD;

    // If lpData is null, just return the required size
    if lpData.is_null() {
        *lpdwDataSize = needed;
        debug!("GetGroupData: returning size {}", needed);
        return DP_OK;
    }

    // Check if buffer is large enough
    if *lpdwDataSize < needed {
        *lpdwDataSize = needed;
        debug!("GetGroupData: buffer too small ({} < {})", *lpdwDataSize, needed);
        return DPERR_BUFFERTOOSMALL;
    }

    // Copy data to buffer
    if needed > 0 {
        std::ptr::copy_nonoverlapping(data.as_ptr(), lpData as *mut u8, data.len());
    }
    *lpdwDataSize = needed;
    debug!("GetGroupData: copied {} bytes", needed);

    DP_OK
}

unsafe extern "system" fn DirectPlay_GetGroupName(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetGroupName({})", idGroup);
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_GetMessageCount(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpdwCount: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetMessageCount({})", idPlayer);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpdwCount.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Drain all pending messages from the helper into the local queue first,
    // so the returned count includes messages still in transit.
    dp.drain_helper_to_local_queue();

    let count = dp.messages.lock().len() as DWORD;
    *lpdwCount = count;

    debug!("GetMessageCount returning {}", count);
    DP_OK
}

unsafe extern "system" fn DirectPlay_GetPlayerAddress(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetPlayerAddress({})", idPlayer);
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_GetPlayerCaps(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpPlayerCaps: *mut DPCAPS,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetPlayerCaps({})", idPlayer);

    if lpPlayerCaps.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Validate dwSize per Wine implementation
    if (*lpPlayerCaps).dw_size != std::mem::size_of::<DPCAPS>() as DWORD {
        debug!("GetPlayerCaps: invalid dwSize {} (expected {})",
               (*lpPlayerCaps).dw_size, std::mem::size_of::<DPCAPS>());
        return DPERR_INVALIDPARAMS;
    }

    *lpPlayerCaps = DPCAPS::for_iroh();
    DP_OK
}

unsafe extern "system" fn DirectPlay_GetPlayerData(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetPlayerData({})", idPlayer);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpdwDataSize.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    let local = dwFlags & DPGET_LOCAL != 0;

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            if let Some(data) = c.get_player_data(idPlayer, local) {
                let needed = data.len() as DWORD;
                if lpData.is_null() || *lpdwDataSize < needed {
                    *lpdwDataSize = needed;
                    return DPERR_BUFFERTOOSMALL;
                }

                std::ptr::copy_nonoverlapping(data.as_ptr(), lpData as *mut u8, data.len());
                *lpdwDataSize = needed;
                return DP_OK;
            }
        }
    }

    *lpdwDataSize = 0;
    DP_OK
}

unsafe extern "system" fn DirectPlay_GetPlayerName(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetPlayerName({})", idPlayer);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpdwDataSize.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            let players = c.get_players();
            if let Some(player) = players.iter().find(|p| p.player_id == idPlayer) {
                // Get string bytes based on encoding
                let (short_bytes, long_bytes): (Vec<u8>, Vec<u8>) = if dp.is_unicode {
                    let short = structs::string_to_wide(&player.name.short_name);
                    let long = structs::string_to_wide(&player.name.long_name);
                    // Convert to bytes (each u16 is 2 bytes)
                    let short_b: Vec<u8> = short.iter().flat_map(|&w| w.to_le_bytes()).collect();
                    let long_b: Vec<u8> = long.iter().flat_map(|&w| w.to_le_bytes()).collect();
                    (short_b, long_b)
                } else {
                    let short = structs::string_to_ansi(&player.name.short_name);
                    let long = structs::string_to_ansi(&player.name.long_name);
                    // Convert i8 to u8 for byte copying
                    let short_b: Vec<u8> = short.iter().map(|&b| b as u8).collect();
                    let long_b: Vec<u8> = long.iter().map(|&b| b as u8).collect();
                    (short_b, long_b)
                };

                // Total size: DPNAME struct + short name bytes + long name bytes
                let name_size = std::mem::size_of::<DPNAME>()
                    + short_bytes.len()
                    + long_bytes.len();

                if lpData.is_null() || (*lpdwDataSize as usize) < name_size {
                    *lpdwDataSize = name_size as DWORD;
                    return DPERR_BUFFERTOOSMALL;
                }

                // Layout: [DPNAME][short_name_bytes][long_name_bytes]
                let base_ptr = lpData as *mut u8;
                let struct_size = std::mem::size_of::<DPNAME>();

                // Calculate string positions in the buffer
                let short_offset = struct_size;
                let long_offset = struct_size + short_bytes.len();

                // Copy short name after the struct
                let short_dst = base_ptr.add(short_offset);
                std::ptr::copy_nonoverlapping(short_bytes.as_ptr(), short_dst, short_bytes.len());

                // Copy long name after short name
                let long_dst = base_ptr.add(long_offset);
                std::ptr::copy_nonoverlapping(long_bytes.as_ptr(), long_dst, long_bytes.len());

                // Fill in the DPNAME struct with pointers to the copied strings
                let name_ptr = lpData as *mut DPNAME;
                (*name_ptr).dw_size = struct_size as DWORD;
                (*name_ptr).dw_flags = 0;
                (*name_ptr).lpsz_short_name = short_dst as *mut u16;
                (*name_ptr).lpsz_long_name = long_dst as *mut u16;

                *lpdwDataSize = name_size as DWORD;
                return DP_OK;
            }
        }
    }

    DPERR_INVALIDPLAYER
}

unsafe extern "system" fn DirectPlay_GetSessionDesc(
    this: *mut IDirectPlay4,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    info!("DirectPlay::GetSessionDesc ENTER lpData={:?} lpdwDataSize={:?}", lpData, lpdwDataSize);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => {
            info!("DirectPlay::GetSessionDesc EXIT DPERR_INVALIDOBJECT");
            return DPERR_INVALIDOBJECT;
        }
    };

    if lpdwDataSize.is_null() {
        info!("DirectPlay::GetSessionDesc EXIT DPERR_INVALIDPARAMS");
        return DPERR_INVALIDPARAMS;
    }

    let state = *dp.state.lock();
    info!("DirectPlay::GetSessionDesc state={:?}", state);
    if state != DPState::InSession {
        info!("DirectPlay::GetSessionDesc EXIT DPERR_NOSESSIONS");
        return DPERR_NOSESSIONS;
    }

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            info!("DirectPlay::GetSessionDesc calling get_session_desc");
            if let Some(desc) = c.get_session_desc() {
                info!("DirectPlay::GetSessionDesc got desc: {:?}", desc.session_name);
                let needed = std::mem::size_of::<DPSESSIONDESC2>() as DWORD;

                if lpData.is_null() || *lpdwDataSize < needed {
                    *lpdwDataSize = needed;
                    return DPERR_BUFFERTOOSMALL;
                }

                let sd = DPSESSIONDESC2 {
                    dw_size: needed,
                    dw_flags: desc.flags,
                    guid_instance: desc.guid_instance,
                    guid_application: desc.guid_application,
                    dw_max_players: desc.max_players,
                    dw_current_players: desc.current_players,
                    lpsz_session_name: std::ptr::null_mut(), // Caller must handle
                    lpsz_password: std::ptr::null_mut(),
                    dw_reserved1: 0,
                    dw_reserved2: 0,
                    dw_user1: desc.user1,
                    dw_user2: desc.user2,
                    dw_user3: desc.user3,
                    dw_user4: desc.user4,
                };

                *(lpData as *mut DPSESSIONDESC2) = sd;
                *lpdwDataSize = needed;
                info!("DirectPlay::GetSessionDesc EXIT DP_OK");
                return DP_OK;
            } else {
                info!("DirectPlay::GetSessionDesc get_session_desc returned None");
            }
        }
    }

    info!("DirectPlay::GetSessionDesc EXIT DPERR_NOSESSIONS (no desc)");
    DPERR_NOSESSIONS
}

unsafe extern "system" fn DirectPlay_Initialize(
    this: *mut IDirectPlay4,
    lpGUID: *const GUID,
) -> HRESULT {
    debug!("DirectPlay::Initialize");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let mut state = dp.state.lock();
    if *state != DPState::Uninitialized {
        return DPERR_ALREADYINITIALIZED;
    }

    // Store service provider GUID if provided
    if !lpGUID.is_null() {
        *dp.sp_guid.lock() = Some(*lpGUID);
    }

    *state = DPState::Initialized;
    info!("DirectPlay initialized");

    DP_OK
}

unsafe extern "system" fn DirectPlay_Open(
    this: *mut IDirectPlay4,
    lpsd: *mut DPSESSIONDESC2,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::Open(flags={:#x})", dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpsd.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Validate dwSize per Wine implementation
    if (*lpsd).dw_size != std::mem::size_of::<DPSESSIONDESC2>() as DWORD {
        debug!("Open: invalid dwSize {} (expected {})",
               (*lpsd).dw_size, std::mem::size_of::<DPSESSIONDESC2>());
        return DPERR_INVALIDPARAMS;
    }

    let mut state = dp.state.lock();

    // Must be initialized
    if *state == DPState::Uninitialized {
        return DPERR_UNINITIALIZED;
    }

    // If already in session and trying to JOIN, just return OK (game may call JOIN after CREATE)
    if *state == DPState::InSession {
        let join = dwFlags & DPOPEN_JOIN != 0;
        if join {
            info!("DirectPlay::Open already in session, JOIN is no-op, returning DP_OK");
            return DP_OK;
        }
        info!("DirectPlay::Open already in session, returning DPERR_ALREADYINITIALIZED");
        return DPERR_ALREADYINITIALIZED;
    }

    let sd = &mut *lpsd;
    let session_desc = if dp.is_unicode {
        SessionDesc::from_raw(sd)
    } else {
        SessionDesc::from_raw_ansi(sd)
    };
    info!("DirectPlay::Open session_name={:?} is_unicode={}", session_desc.session_name, dp.is_unicode);

    let create = dwFlags & DPOPEN_CREATE != 0;
    let join = dwFlags & DPOPEN_JOIN != 0;

    if create && join {
        return DPERR_INVALIDFLAGS;
    }

    if create {
        // Create new session
        info!("Creating session: {}", session_desc.session_name);

        let mut desc = session_desc.clone();
        if desc.guid_instance.is_nil() {
            desc.guid_instance = GUID::new_random();
        }

        if let Some(client) = dp.get_ipc_client() {
            if let Some(c) = client.as_ref() {
                if let Some(guid) = c.create_session(desc) {
                    // Update the passed-in session desc with instance GUID
                    sd.guid_instance = guid;
                    *state = DPState::InSession;

                    // NOTE: The host flag (+0x1b30) is now set correctly via
                    // DPSYS_CREATEPLAYERORGROUP message queued in Transport::create_player.
                    // The game receives this message via Receive() and processes it
                    // through NetworkPlayerStateHandler, which computes +0x1b30.

                    return DP_OK;
                }
            }
        }

        return DPERR_CANTCREATESESSION;
    } else if join {
        // Join existing session
        info!("Joining session: {:?}", session_desc.guid_instance);

        // Look up the session from EnumSessions to get the host_ticket
        let session = dp.get_discovered_session(&session_desc.guid_instance);

        if let Some(session) = session {
            info!("Found discovered session: {} with ticket {}", session.session_name, session.host_ticket);
            if let Some(client) = dp.get_ipc_client() {
                if let Some(c) = client.as_ref() {
                    match c.join_session_by_ticket(&session.host_ticket) {
                        Ok(player_id) => {
                            info!("Joined session, got player_id {}", player_id);
                            *state = DPState::InSession;
                            return DP_OK;
                        }
                        Err(e) => {
                            error!("Failed to join session: {:?}", e);
                            return DPERR_NOSESSIONS;
                        }
                    }
                }
            }
            return DPERR_NOCONNECTION;
        } else {
            error!("Session {:?} not found in discovered sessions", session_desc.guid_instance);
            return DPERR_NOSESSIONS;
        }
    }

    DPERR_INVALIDFLAGS
}

unsafe extern "system" fn DirectPlay_Receive(
    this: *mut IDirectPlay4,
    lpidFrom: *mut DPID,
    lpidTo: *mut DPID,
    dwFlags: DWORD,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    let peek = dwFlags & DPRECEIVE_PEEK != 0;
    let span = debug_span!("DirectPlay_Receive", flags = dwFlags, peek);
    let _guard = span.enter();

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpdwDataSize.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Drain ALL pending messages from the helper into the local queue, not just
    // one: partial draining lets messages pile up in the helper while
    // GetMessageCount reports 0.
    dp.drain_helper_to_local_queue();

    // Get from queue
    let mut messages = dp.messages.lock();
    let queue_len = messages.len();

    if messages.is_empty() {
        debug!("no messages available");
        return DPERR_NOMESSAGES;
    }

    debug!(queue_len, "checking local queue");

    // Per Wine: filter by DPRECEIVE_TOPLAYER and DPRECEIVE_FROMPLAYER
    // DPRECEIVE_ALL (0x01) means "receive for any player" - disables TOPLAYER filtering
    // Find the first message matching the filter criteria
    let receive_all = (dwFlags & DPRECEIVE_ALL) != 0;
    let filter_to = !receive_all && (dwFlags & DPRECEIVE_TOPLAYER) != 0 && !lpidTo.is_null();
    let filter_from = (dwFlags & DPRECEIVE_FROMPLAYER) != 0 && !lpidFrom.is_null();

    let found_idx = messages.iter().position(|m| {
        if filter_to && m.to != *lpidTo {
            return false;
        }
        if filter_from && m.from != *lpidFrom {
            return false;
        }
        true
    });

    let msg = match found_idx {
        Some(idx) => {
            if peek {
                messages.get(idx).cloned()
            } else {
                messages.remove(idx)
            }
        }
        None => None,
    };

    match msg {
        Some(m) => {
            if !lpidFrom.is_null() {
                *lpidFrom = m.from;
            }
            if !lpidTo.is_null() {
                *lpidTo = m.to;
            }

            let needed = m.data.len() as DWORD;
            if lpData.is_null() || *lpdwDataSize < needed {
                // Put message back if not peeking
                if !peek {
                    messages.push_front(m);
                }
                *lpdwDataSize = needed;
                debug!(needed, "buffer too small");
                return DPERR_BUFFERTOOSMALL;
            }

            debug!(from = m.from, to = m.to, size = m.data.len(), "returning message to game");

            // Diagnostic logging: dump first 32 bytes of message data
            let preview_len = std::cmp::min(32, m.data.len());
            info!(
                "RECV: from={:#x} to={:#x} size={} data[0..{}]={:02x?}",
                m.from, m.to, m.data.len(), preview_len, &m.data[..preview_len]
            );

            // Ready state protocol logging
            if m.data.len() >= 2 {
                let msg_type = u16::from_le_bytes([m.data[0], m.data[1]]);
                match msg_type {
                    0x0f0a => info!("<<< READY_SET: from={:#x} to={:#x}", m.from, m.to),
                    0x0f0b => info!("<<< SAVE_DATA: from={:#x} to={:#x} size={}", m.from, m.to, m.data.len()),
                    0x0f0c => info!("<<< SAVE_CONFIRM: from={:#x} to={:#x}", m.from, m.to),
                    0x0f07 => info!("<<< GAME_START_SIGNAL: from={:#x} to={:#x}", m.from, m.to),
                    0x4301 => info!("<<< NEXT_TURN: from={:#x} to={:#x}", m.from, m.to),
                    0x4303 => info!("<<< NEXT_UPKEEP: from={:#x} to={:#x}", m.from, m.to),
                    _ => {}
                }
            }

            // Faction-selection decode (lobby debugging). The GAME message type sits at
            // WIRE offset 8 (offset 0 is a routing category); a faction pick is 0x2f04
            // (request bit 0x2000) and the host's authoritative echo is 0x4f04. Handler
            // base = wire+8, so faction id = wire+8+0x13 = 27 and the embedded owner DPID
            // = wire+8+0x188 = 400. Pair this with the FSA-FSD probe fire pattern: for a
            // KNOWN-FREE faction, any reject (no FACTION ECHO(0x4f04) back) is spurious.
            if m.data.len() >= 404 {
                let game_type = u16::from_le_bytes([m.data[8], m.data[9]]);
                if game_type == 0x2f04 || game_type == 0x4f04 {
                    let faction = m.data[27];
                    let owner_dpid =
                        u32::from_le_bytes([m.data[400], m.data[401], m.data[402], m.data[403]]);
                    info!(
                        "FACTION {} RECV: from={:#x} to={:#x} faction={} embedded_dpid={:#x}",
                        if game_type == 0x2f04 { "PICK(0x2f04)" } else { "ECHO(0x4f04)" },
                        m.from, m.to, faction, owner_dpid
                    );
                }
            }

            // NOTE: bytes 4-7 are a per-message SEQUENCE COUNTER for these SMAC message
            // types, NOT a player_id (confirmed by forensics: the value tracks send order,
            // not the DPID). The old "MISMATCH" warning here was a false positive comparing
            // a sequence counter to the DPID; kept at debug only, does not affect delivery.
            if m.data.len() >= 8 && m.from != 0 {
                let seq_field = u32::from_le_bytes([m.data[4], m.data[5], m.data[6], m.data[7]]);
                debug!("RECV seq_field={:#x} from={:#x}", seq_field, m.from);
            }

            std::ptr::copy_nonoverlapping(m.data.as_ptr(), lpData as *mut u8, m.data.len());

            // Fix up pointer fields in system messages (from = DPID_SYSMSG = 0)
            // Wine stores offsets during serialization and fixes them up during Receive
            if m.from == 0 && m.data.len() >= 4 {
                fixup_system_message_pointers(lpData as *mut u8, m.data.len());
            }

            *lpdwDataSize = needed;
            DP_OK
        }
        None => {
            debug!("no messages available");
            DPERR_NOMESSAGES
        }
    }
}

unsafe extern "system" fn DirectPlay_Send(
    this: *mut IDirectPlay4,
    idFrom: DPID,
    idTo: DPID,
    dwFlags: DWORD,
    lpData: LPVOID,
    dwDataSize: DWORD,
) -> HRESULT {
    let guaranteed = dwFlags & DPSEND_GUARANTEED != 0;
    let span = debug_span!("DirectPlay_Send", from = idFrom, to = idTo, size = dwDataSize, guaranteed);
    let _guard = span.enter();

    debug!("sending");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpData.is_null() && dwDataSize > 0 {
        return DPERR_INVALIDPARAMS;
    }

    let data = if dwDataSize > 0 {
        std::slice::from_raw_parts(lpData as *const u8, dwDataSize as usize).to_vec()
    } else {
        Vec::new()
    };

    // Diagnostic logging: dump first 32 bytes of message data
    let preview_len = std::cmp::min(32, data.len());
    info!(
        "SEND: from={:#x} to={:#x} flags={:#x} size={} data[0..{}]={:02x?}",
        idFrom, idTo, dwFlags, dwDataSize, preview_len, &data[..preview_len]
    );

    // Symmetric emit-order trace (see drain_helper_to_local_queue). Compared with
    // the joiner's RXTRACE this proves whether any observed reorder is host-emit
    // order (should be strictly monotonic — single-threaded sequence stamping) or
    // introduced by our transport. Same header field layout (SyncMessageHeader).
    if rx_trace_enabled() {
        let seq = RX_SEQ.fetch_add(1, Ordering::Relaxed);
        let d = &data;
        let u16le = |o: usize| -> u16 {
            if d.len() >= o + 2 { u16::from_le_bytes([d[o], d[o + 1]]) } else { 0 }
        };
        let u32le = |o: usize| -> u32 {
            if d.len() >= o + 4 {
                u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
            } else { 0 }
        };
        info!(
            "TXTRACE #{} from={:#x} to={:#x} size={} type={:#06x} hostSeq={} syncType={:#x} chunkIdx={} dataOff={}",
            seq, idFrom, idTo, d.len(),
            u16le(0), u32le(8), u16le(16), u16le(18), u32le(32)
        );
    }

    // Faction-selection decode on send (see the RECV-side note for the layout).
    // On the joiner this shows the PICK(0x2f04) it emits; on the host it shows whether
    // an ECHO(0x4f04) is ever sent (its absence = the pick was rejected before echo).
    if data.len() >= 404 {
        let game_type = u16::from_le_bytes([data[8], data[9]]);
        if game_type == 0x2f04 || game_type == 0x4f04 {
            let faction = data[27];
            let owner_dpid =
                u32::from_le_bytes([data[400], data[401], data[402], data[403]]);
            info!(
                "FACTION {} SEND: from={:#x} to={:#x} faction={} embedded_dpid={:#x}",
                if game_type == 0x2f04 { "PICK(0x2f04)" } else { "ECHO(0x4f04)" },
                idFrom, idTo, faction, owner_dpid
            );
        }
    }

    // Ready state protocol logging
    if data.len() >= 2 {
        let msg_type = u16::from_le_bytes([data[0], data[1]]);
        match msg_type {
            0x0f0a => info!(">>> READY_SET: from={:#x} to={:#x}", idFrom, idTo),
            0x0f0b => info!(">>> SAVE_DATA: from={:#x} to={:#x} size={}", idFrom, idTo, data.len()),
            0x0f0c => info!(">>> SAVE_CONFIRM: from={:#x} to={:#x}", idFrom, idTo),
            0x0f07 => info!(">>> GAME_START_SIGNAL: from={:#x} to={:#x}", idFrom, idTo),
            0x4301 => info!(">>> NEXT_TURN: from={:#x} to={:#x}", idFrom, idTo),
            0x4303 => info!(">>> NEXT_UPKEEP: from={:#x} to={:#x}", idFrom, idTo),
            _ => {}
        }
    }

    // Self-loopback: if sender == recipient, deliver locally (IPX semantics)
    if idFrom == idTo && idFrom != 0 {
        info!("LOOPBACK: from={:#x} to={:#x} size={}", idFrom, idTo, data.len());
        dp.messages.lock().push_back(PendingMessage {
            from: idFrom,
            to: idTo,
            data,
        });
        return DP_OK;
    }

    // Check if idTo is a group - if so, expand to individual sends
    {
        let groups = dp.groups.lock();
        if let Some(members) = groups.get(&idTo) {
            let member_count = members.len();
            let members_copy: Vec<DPID> = members.iter().copied().collect();
            drop(groups); // Release lock before sending

            info!(
                "GROUP_SEND: from={:#x} to group {:#x} ({} members)",
                idFrom, idTo, member_count
            );

            // Send to each member - self-loopback handles sender if in group
            for member_id in members_copy {
                if member_id == idFrom {
                    // Self-send: queue locally
                    info!("GROUP_SEND: self-loopback to {:#x}", member_id);
                    dp.messages.lock().push_back(PendingMessage {
                        from: idFrom,
                        to: member_id,
                        data: data.clone(),
                    });
                } else {
                    // Send to remote member
                    if let Some(client) = dp.get_ipc_client() {
                        if let Some(c) = client.as_ref() {
                            if let Err(e) = c.send(idFrom, member_id, data.clone(), guaranteed) {
                                warn!(error = ?e, "GROUP_SEND to {:#x} failed", member_id);
                            }
                        }
                    }
                }
            }
            return DP_OK;
        }
    }

    // Not a group - regular player send
    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            match c.send(idFrom, idTo, data, guaranteed) {
                Ok(()) => {
                    debug!("send completed");
                    return DP_OK;
                }
                Err(e) => {
                    warn!(error = ?e, "send failed");
                    return DPERR_GENERIC;
                }
            }
        }
    }

    debug!("no IPC connection");
    DPERR_NOCONNECTION
}

unsafe extern "system" fn DirectPlay_SetGroupData(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpData: LPVOID,
    dwDataSize: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::SetGroupData({}, size={}, flags={:#x})", idGroup, dwDataSize, dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    // Check group exists
    if !dp.groups.lock().contains_key(&idGroup) {
        debug!("SetGroupData: group {} not found", idGroup);
        return DPERR_INVALIDGROUP;
    }

    // Copy the data
    let data = if lpData.is_null() || dwDataSize == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(lpData as *const u8, dwDataSize as usize).to_vec()
    };

    let is_local = (dwFlags & DPSET_LOCAL) != 0;
    let mut group_data = dp.group_data.lock();
    let entry = group_data.entry(idGroup).or_insert((Vec::new(), Vec::new()));

    if is_local {
        entry.0 = data;
        debug!("SetGroupData: set {} bytes of local data", entry.0.len());
    } else {
        entry.1 = data;
        debug!("SetGroupData: set {} bytes of remote data", entry.1.len());
        // Note: For remote data, we should ideally broadcast via IPC
        // but for now we only store locally (sufficient for single-host scenarios)
    }

    DP_OK
}

unsafe extern "system" fn DirectPlay_SetGroupName(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpGroupName: *const DPNAME,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::SetGroupName({})", idGroup);
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_SetPlayerData(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpData: LPVOID,
    dwDataSize: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::SetPlayerData({})", idPlayer);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let data = if dwDataSize > 0 && !lpData.is_null() {
        std::slice::from_raw_parts(lpData as *const u8, dwDataSize as usize).to_vec()
    } else {
        Vec::new()
    };

    let local = dwFlags & DPSET_LOCAL != 0;

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            c.set_player_data(idPlayer, data, local);
            return DP_OK;
        }
    }

    DPERR_INVALIDPLAYER
}

unsafe extern "system" fn DirectPlay_SetPlayerName(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpPlayerName: *const DPNAME,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::SetPlayerName({})", idPlayer);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    // Parse the name with correct encoding
    let name = if lpPlayerName.is_null() {
        PlayerName::default()
    } else if dp.is_unicode {
        PlayerName::from_raw(&*lpPlayerName)
    } else {
        PlayerName::from_raw_ansi(&*(lpPlayerName as *const structs::DPNAME_A))
    };

    // Set the player name via IPC
    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            if c.set_player_name(idPlayer, name) {
                return DP_OK;
            }
        }
    }

    DPERR_INVALIDPLAYER
}

unsafe extern "system" fn DirectPlay_SetSessionDesc(
    this: *mut IDirectPlay4,
    lpSessDesc: *mut DPSESSIONDESC2,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::SetSessionDesc");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpSessDesc.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    // Parse the new session desc with correct encoding
    let new_desc = if dp.is_unicode {
        SessionDesc::from_raw(&*lpSessDesc)
    } else {
        SessionDesc::from_raw_ansi(&*lpSessDesc)
    };

    // Set session desc via IPC (host only)
    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            // If the new session_name is empty, preserve the existing one.
            // Games often call SetSessionDesc to update flags without changing the name,
            // passing NULL for lpsz_session_name, which we convert to empty string.
            let final_desc = if new_desc.session_name.is_empty() {
                if let Some(current) = c.get_session_desc() {
                    if !current.session_name.is_empty() {
                        debug!("Preserving existing session_name: {}", current.session_name);
                        SessionDesc {
                            session_name: current.session_name,
                            ..new_desc
                        }
                    } else {
                        new_desc
                    }
                } else {
                    new_desc
                }
            } else {
                new_desc
            };

            if c.set_session_desc(final_desc) {
                return DP_OK;
            } else {
                return DPERR_ACCESSDENIED; // Not host
            }
        }
    }

    DPERR_NOSESSIONS
}

// ============================================================================
// IDirectPlay3 Implementation
// ============================================================================

unsafe extern "system" fn DirectPlay_AddGroupToGroup(
    this: *mut IDirectPlay4,
    idParentGroup: DPID,
    idGroup: DPID,
) -> HRESULT {
    debug!("DirectPlay::AddGroupToGroup");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_CreateGroupInGroup(
    this: *mut IDirectPlay4,
    idParentGroup: DPID,
    lpidGroup: *mut DPID,
    lpGroupName: *const DPNAME,
    lpData: LPVOID,
    dwDataSize: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::CreateGroupInGroup");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_DeleteGroupFromGroup(
    this: *mut IDirectPlay4,
    idParentGroup: DPID,
    idGroup: DPID,
) -> HRESULT {
    debug!("DirectPlay::DeleteGroupFromGroup");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_EnumConnections(
    this: *mut IDirectPlay4,
    lpguidApplication: *const GUID,
    lpEnumCallback: LPDPENUMCONNECTIONSCALLBACK,
    lpContext: LPVOID,
    dwFlags: DWORD,
) -> HRESULT {
    info!("DirectPlay::EnumConnections ENTER this={:?} lpEnumCallback={:?} dwFlags={:#x}", this, lpEnumCallback, dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let callback = match lpEnumCallback {
        Some(cb) => cb,
        None => {
            info!("DirectPlay::EnumConnections EXIT DPERR_INVALIDPARAMS (null callback)");
            return DPERR_INVALIDPARAMS;
        }
    };

    info!("DirectPlay::EnumConnections: calling callback for Iroh P2P, is_unicode={}", dp.is_unicode);

    // Report Iroh connection with correct encoding
    let name_wide;
    let name_ansi;
    let name_ptr = if dp.is_unicode {
        name_wide = structs::string_to_wide("Iroh P2P");
        name_wide.as_ptr() as *mut u16
    } else {
        name_ansi = structs::string_to_ansi("Iroh P2P");
        name_ansi.as_ptr() as *mut u16
    };

    let dpname = DPNAME {
        dw_size: std::mem::size_of::<DPNAME>() as DWORD,
        dw_flags: 0,
        lpsz_short_name: name_ptr,
        lpsz_long_name: name_ptr,
    };

    let result = callback(
        &DPSPGUID_IROH,
        std::ptr::null_mut(),
        0,
        &dpname,
        DPCONNECTION_DIRECTPLAY,
        lpContext,
    );
    info!("DirectPlay::EnumConnections: callback returned {}", result);

    info!("DirectPlay::EnumConnections EXIT DP_OK");
    DP_OK
}

unsafe extern "system" fn DirectPlay_EnumGroupsInGroup(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpguidInstance: *const GUID,
    lpEnumCallback: LPDPENUMPLAYERSCALLBACK2,
    lpContext: LPVOID,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::EnumGroupsInGroup");
    DP_OK
}

unsafe extern "system" fn DirectPlay_GetGroupConnectionSettings(
    this: *mut IDirectPlay4,
    dwFlags: DWORD,
    idGroup: DPID,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetGroupConnectionSettings");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_InitializeConnection(
    this: *mut IDirectPlay4,
    lpConnection: LPVOID,
    dwFlags: DWORD,
) -> HRESULT {
    info!("DirectPlay::InitializeConnection ENTER this={:?} lpConnection={:?} dwFlags={:#x}", this, lpConnection, dwFlags);

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => {
            info!("DirectPlay::InitializeConnection EXIT DPERR_INVALIDOBJECT");
            return DPERR_INVALIDOBJECT;
        }
    };

    info!("DirectPlay::InitializeConnection: got DirectPlayObject");

    // Just mark as initialized - we use Iroh regardless
    // Transport will be created lazily when actually needed
    let mut state = dp.state.lock();
    info!("DirectPlay::InitializeConnection: current state={:?}", *state);
    if *state == DPState::Uninitialized {
        *state = DPState::Initialized;
        info!("DirectPlay::InitializeConnection: state changed to Initialized");
    }

    info!("DirectPlay::InitializeConnection EXIT DP_OK");
    DP_OK
}

unsafe extern "system" fn DirectPlay_SecureOpen(
    this: *mut IDirectPlay4,
    lpsd: *const DPSESSIONDESC2,
    dwFlags: DWORD,
    lpSecurity: *const DPSECURITYDESC,
    lpCredentials: *const DPCREDENTIALS,
) -> HRESULT {
    debug!("DirectPlay::SecureOpen");
    // Just call regular Open - Iroh provides encryption
    DirectPlay_Open(this, lpsd as *mut DPSESSIONDESC2, dwFlags)
}

unsafe extern "system" fn DirectPlay_SendChatMessage(
    this: *mut IDirectPlay4,
    idFrom: DPID,
    idTo: DPID,
    dwFlags: DWORD,
    lpChatMessage: *const DPCHAT,
) -> HRESULT {
    debug!("DirectPlay::SendChatMessage");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_SetGroupConnectionSettings(
    this: *mut IDirectPlay4,
    dwFlags: DWORD,
    idGroup: DPID,
    lpConnection: *mut DPLCONNECTION,
) -> HRESULT {
    debug!("DirectPlay::SetGroupConnectionSettings");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_StartSession(
    this: *mut IDirectPlay4,
    dwFlags: DWORD,
    idGroup: DPID,
) -> HRESULT {
    debug!("DirectPlay::StartSession");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_GetGroupFlags(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpdwFlags: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetGroupFlags");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_GetGroupParent(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpidParent: *mut DPID,
) -> HRESULT {
    debug!("DirectPlay::GetGroupParent");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_GetPlayerAccount(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    dwFlags: DWORD,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetPlayerAccount");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_GetPlayerFlags(
    this: *mut IDirectPlay4,
    idPlayer: DPID,
    lpdwFlags: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetPlayerFlags");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    if lpdwFlags.is_null() {
        return DPERR_INVALIDPARAMS;
    }

    if let Some(client) = dp.get_ipc_client() {
        if let Some(c) = client.as_ref() {
            let players = c.get_players();
            if let Some(player) = players.iter().find(|p| p.player_id == idPlayer) {
                *lpdwFlags = player.flags;
                return DP_OK;
            }
        }
    }

    DPERR_INVALIDPLAYER
}

// ============================================================================
// IDirectPlay4 Implementation
// ============================================================================

unsafe extern "system" fn DirectPlay_GetGroupOwner(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    lpidOwner: *mut DPID,
) -> HRESULT {
    debug!("DirectPlay::GetGroupOwner");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_SetGroupOwner(
    this: *mut IDirectPlay4,
    idGroup: DPID,
    idOwner: DPID,
) -> HRESULT {
    debug!("DirectPlay::SetGroupOwner");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_SendEx(
    this: *mut IDirectPlay4,
    idFrom: DPID,
    idTo: DPID,
    dwFlags: DWORD,
    lpData: LPVOID,
    dwDataSize: DWORD,
    dwPriority: DWORD,
    dwTimeout: DWORD,
    lpContext: LPVOID,
    lpdwMsgID: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::SendEx");
    // Just use regular Send
    DirectPlay_Send(this, idFrom, idTo, dwFlags, lpData, dwDataSize)
}

unsafe extern "system" fn DirectPlay_GetMessageQueue(
    this: *mut IDirectPlay4,
    idFrom: DPID,
    idTo: DPID,
    dwFlags: DWORD,
    lpdwNumMsgs: *mut DWORD,
    lpdwNumBytes: *mut DWORD,
) -> HRESULT {
    debug!("DirectPlay::GetMessageQueue");

    let dp = match get_dp(this) {
        Some(dp) => dp,
        None => return DPERR_INVALIDOBJECT,
    };

    let messages = dp.messages.lock();

    if !lpdwNumMsgs.is_null() {
        *lpdwNumMsgs = messages.len() as DWORD;
    }

    if !lpdwNumBytes.is_null() {
        let total_bytes: usize = messages.iter().map(|m| m.data.len()).sum();
        *lpdwNumBytes = total_bytes as DWORD;
    }

    DP_OK
}

unsafe extern "system" fn DirectPlay_CancelMessage(
    this: *mut IDirectPlay4,
    dwMsgID: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::CancelMessage");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn DirectPlay_CancelPriority(
    this: *mut IDirectPlay4,
    dwMinPriority: DWORD,
    dwMaxPriority: DWORD,
    dwFlags: DWORD,
) -> HRESULT {
    debug!("DirectPlay::CancelPriority");
    DPERR_UNSUPPORTED
}

// ============================================================================
// System Message Pointer Fixup
// ============================================================================

/// Fix up pointer fields in system messages received from the network.
///
/// Wine's DirectPlay stores offsets (relative to buffer start) in pointer fields
/// during serialization. When the game receives these messages, the offsets must
/// be converted to actual memory addresses.
///
/// SECURITY: Network-supplied offsets are validated to be within buffer bounds
/// before being converted to pointers. Invalid offsets are replaced with null.
///
/// # Safety
/// - `buf` must point to a valid buffer of at least `len` bytes
/// - `buf` must be writable
/// - Called after the message data has been copied to the output buffer
unsafe fn fixup_system_message_pointers(buf: *mut u8, len: usize) {
    use dp_types::structs::sysmsg;

    // Need at least 4 bytes for dwType
    if len < 4 {
        return;
    }

    let buf_addr = buf as usize;

    // Helper to read a u32 at an offset, with bounds check
    let read_u32 = |offset: usize| -> Option<u32> {
        if offset + 4 <= len {
            let ptr = buf.add(offset) as *const u32;
            Some(ptr.read_unaligned())
        } else {
            None
        }
    };

    // Helper to write a u32 at an offset, with bounds check
    let write_u32 = |offset: usize, value: u32| {
        if offset + 4 <= len {
            let ptr = buf.add(offset) as *mut u32;
            ptr.write_unaligned(value);
        }
    };

    // Helper to fixup a pointer field at a given offset.
    // Reads the stored offset, validates it's within buffer bounds,
    // and replaces it with the actual address (buf + offset).
    // Returns true if fixup succeeded, false if offset was invalid.
    let fixup_ptr_field = |field_offset: usize| -> bool {
        let Some(stored_offset) = read_u32(field_offset) else {
            return false;
        };

        // Offset 0 means null pointer - leave as is
        if stored_offset == 0 {
            return true;
        }

        let stored_offset = stored_offset as usize;

        // Validate: offset must be within buffer bounds
        if stored_offset >= len {
            warn!(
                "FIXUP: invalid offset {} at field offset {} (buffer len={}), setting to null",
                stored_offset, field_offset, len
            );
            write_u32(field_offset, 0);
            return false;
        }

        // Convert offset to actual address
        let actual_addr = buf_addr + stored_offset;
        write_u32(field_offset, actual_addr as u32);
        true
    };

    let dwType = read_u32(0).unwrap_or(0);

    match dwType {
        sysmsg::DPSYS_CREATEPLAYERORGROUP => {
            // DPMSG_CREATEPLAYERORGROUP layout (48 bytes):
            //   0: dwType
            //   4: dwPlayerType
            //   8: dpId
            //  12: dwCurrentPlayers
            //  16: lpData           <- POINTER
            //  20: dwDataSize
            //  24: dpnName.dwSize
            //  28: dpnName.dwFlags
            //  32: lpszShortNameA   <- POINTER
            //  36: lpszLongNameA    <- POINTER
            //  40: dpIdParent
            //  44: dwFlags

            if len < 48 {
                warn!("FIXUP: CREATEPLAYERORGROUP buffer too small ({} < 48)", len);
                return;
            }

            info!("FIXUP: DPSYS_CREATEPLAYERORGROUP - fixing up pointer fields");
            fixup_ptr_field(16); // lpData
            fixup_ptr_field(32); // lpszShortNameA
            fixup_ptr_field(36); // lpszLongNameA
        }

        sysmsg::DPSYS_DESTROYPLAYERORGROUP => {
            // DPMSG_DESTROYPLAYERORGROUP layout (52 bytes):
            //   0: dwType
            //   4: dwPlayerType
            //   8: dpId
            //  12: lpLocalData      <- POINTER
            //  16: dwLocalDataSize
            //  20: lpRemoteData     <- POINTER
            //  24: dwRemoteDataSize
            //  28: dpnName.dwSize
            //  32: dpnName.dwFlags
            //  36: lpszShortNameA   <- POINTER
            //  40: lpszLongNameA    <- POINTER
            //  44: dpIdParent
            //  48: dwFlags

            if len < 52 {
                warn!("FIXUP: DESTROYPLAYERORGROUP buffer too small ({} < 52)", len);
                return;
            }

            info!("FIXUP: DPSYS_DESTROYPLAYERORGROUP - fixing up pointer fields");
            fixup_ptr_field(12); // lpLocalData
            fixup_ptr_field(20); // lpRemoteData
            fixup_ptr_field(36); // lpszShortNameA
            fixup_ptr_field(40); // lpszLongNameA
        }

        sysmsg::DPSYS_SETPLAYERORGROUPDATA => {
            // DPMSG_SETPLAYERORGROUPDATA layout:
            //   0: dwType
            //   4: dwPlayerType
            //   8: dpId
            //  12: lpData           <- POINTER
            //  16: dwDataSize

            if len < 20 {
                warn!("FIXUP: SETPLAYERORGROUPDATA buffer too small ({} < 20)", len);
                return;
            }

            info!("FIXUP: DPSYS_SETPLAYERORGROUPDATA - fixing up pointer fields");
            fixup_ptr_field(12); // lpData
        }

        sysmsg::DPSYS_SETPLAYERORGROUPNAME => {
            // DPMSG_SETPLAYERORGROUPNAME layout (see serialize_dpmsg_setplayerorgroupname):
            //   0: dwType
            //   4: dwPlayerType
            //   8: dpId
            //  12: dpnName.dwSize
            //  16: dpnName.dwFlags
            //  20: dpnName.lpszShortNameA   <- POINTER
            //  24: dpnName.lpszLongNameA    <- POINTER
            // The game reads the (long) name pointer straight out of this buffer,
            // so these offsets must be rewritten to absolute addresses like the
            // other system messages.
            if len < 28 {
                warn!("FIXUP: SETPLAYERORGROUPNAME buffer too small ({} < 28)", len);
                return;
            }

            info!("FIXUP: DPSYS_SETPLAYERORGROUPNAME - fixing up pointer fields");
            fixup_ptr_field(20); // lpszShortNameA
            fixup_ptr_field(24); // lpszLongNameA
        }

        _ => {
            // Unknown or non-pointer system message type - no fixup needed
            debug!("FIXUP: unknown system message type {:#x}, no fixup", dwType);
        }
    }
}
