//! DirectPlay data structures

use crate::{DPID, DWORD, GUID, LPVOID};
use serde::{Deserialize, Serialize};

/// Session descriptor structure
/// This is the core structure for describing a DirectPlay session
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPSESSIONDESC2 {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub guid_instance: GUID,
    pub guid_application: GUID,
    pub dw_max_players: DWORD,
    pub dw_current_players: DWORD,
    pub lpsz_session_name: *mut u16, // LPWSTR
    pub lpsz_password: *mut u16,     // LPWSTR
    pub dw_reserved1: DWORD,
    pub dw_reserved2: DWORD,
    pub dw_user1: DWORD,
    pub dw_user2: DWORD,
    pub dw_user3: DWORD,
    pub dw_user4: DWORD,
}

impl Default for DPSESSIONDESC2 {
    fn default() -> Self {
        Self {
            dw_size: std::mem::size_of::<Self>() as DWORD,
            dw_flags: 0,
            guid_instance: GUID::zeroed(),
            guid_application: GUID::zeroed(),
            dw_max_players: 0,
            dw_current_players: 0,
            lpsz_session_name: std::ptr::null_mut(),
            lpsz_password: std::ptr::null_mut(),
            dw_reserved1: 0,
            dw_reserved2: 0,
            dw_user1: 0,
            dw_user2: 0,
            dw_user3: 0,
            dw_user4: 0,
        }
    }
}

/// Rust-safe version of session descriptor for internal use
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionDesc {
    pub flags: u32,
    pub guid_instance: GUID,
    pub guid_application: GUID,
    pub max_players: u32,
    pub current_players: u32,
    pub session_name: String,
    pub password: Option<String>,
    pub user1: u32,
    pub user2: u32,
    pub user3: u32,
    pub user4: u32,
}

impl SessionDesc {
    /// Convert from raw DPSESSIONDESC2 (Unicode version)
    ///
    /// # Safety
    /// The pointers in `desc` must be valid or null
    pub unsafe fn from_raw(desc: &DPSESSIONDESC2) -> Self {
        Self {
            flags: desc.dw_flags,
            guid_instance: desc.guid_instance,
            guid_application: desc.guid_application,
            max_players: desc.dw_max_players,
            current_players: desc.dw_current_players,
            session_name: wide_to_string(desc.lpsz_session_name),
            password: if desc.lpsz_password.is_null() {
                None
            } else {
                Some(wide_to_string(desc.lpsz_password))
            },
            user1: desc.dw_user1,
            user2: desc.dw_user2,
            user3: desc.dw_user3,
            user4: desc.dw_user4,
        }
    }

    /// Convert from raw DPSESSIONDESC2 (ANSI version)
    ///
    /// # Safety
    /// The pointers in `desc` must be valid or null, and point to ANSI strings
    pub unsafe fn from_raw_ansi(desc: &DPSESSIONDESC2) -> Self {
        Self {
            flags: desc.dw_flags,
            guid_instance: desc.guid_instance,
            guid_application: desc.guid_application,
            max_players: desc.dw_max_players,
            current_players: desc.dw_current_players,
            session_name: ansi_to_string(desc.lpsz_session_name as *const i8),
            password: if desc.lpsz_password.is_null() {
                None
            } else {
                Some(ansi_to_string(desc.lpsz_password as *const i8))
            },
            user1: desc.dw_user1,
            user2: desc.dw_user2,
            user3: desc.dw_user3,
            user4: desc.dw_user4,
        }
    }
}

/// Player/group name structure
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPNAME {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub lpsz_short_name: *mut u16, // LPWSTR
    pub lpsz_long_name: *mut u16,  // LPWSTR
}

impl Default for DPNAME {
    fn default() -> Self {
        Self {
            dw_size: std::mem::size_of::<Self>() as DWORD,
            dw_flags: 0,
            lpsz_short_name: std::ptr::null_mut(),
            lpsz_long_name: std::ptr::null_mut(),
        }
    }
}

/// ANSI version of player/group name structure
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPNAME_A {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub lpsz_short_name: *mut i8, // LPSTR
    pub lpsz_long_name: *mut i8,  // LPSTR
}

impl Default for DPNAME_A {
    fn default() -> Self {
        Self {
            dw_size: std::mem::size_of::<Self>() as DWORD,
            dw_flags: 0,
            lpsz_short_name: std::ptr::null_mut(),
            lpsz_long_name: std::ptr::null_mut(),
        }
    }
}

/// Rust-safe version of player name
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerName {
    pub short_name: String,
    pub long_name: String,
}

impl PlayerName {
    /// Convert from raw DPNAME
    ///
    /// # Safety
    /// The pointers in `name` must be valid or null
    pub unsafe fn from_raw(name: &DPNAME) -> Self {
        Self {
            short_name: wide_to_string(name.lpsz_short_name),
            long_name: wide_to_string(name.lpsz_long_name),
        }
    }

    /// Convert from raw DPNAME_A (ANSI)
    ///
    /// # Safety
    /// The pointers in `name` must be valid or null
    pub unsafe fn from_raw_ansi(name: &DPNAME_A) -> Self {
        Self {
            short_name: ansi_to_string(name.lpsz_short_name),
            long_name: ansi_to_string(name.lpsz_long_name),
        }
    }
}

/// Capability structure
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct DPCAPS {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub dw_max_buffer_size: DWORD,
    pub dw_max_queue_size: DWORD,
    pub dw_max_players: DWORD,
    pub dw_hundred_baud: DWORD, // Bandwidth in 100 baud units
    pub dw_latency: DWORD,      // Latency in ms
    pub dw_max_local_players: DWORD,
    pub dw_header_length: DWORD,
    pub dw_timeout: DWORD,
}

impl DPCAPS {
    /// Create a DPCAPS with reasonable defaults for Iroh transport
    pub fn for_iroh() -> Self {
        Self {
            dw_size: std::mem::size_of::<Self>() as DWORD,
            dw_flags: 0, // Will be filled based on session state
            dw_max_buffer_size: 65536,
            dw_max_queue_size: 256,
            dw_max_players: 16,
            dw_hundred_baud: 10000, // ~1 Mbps
            dw_latency: 50,         // 50ms estimated
            dw_max_local_players: 1,
            dw_header_length: 64,
            dw_timeout: 5000, // 5 second timeout
        }
    }
}

/// Credentials structure for secure sessions
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPCREDENTIALS {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub lpsz_username: *mut u16,
    pub lpsz_password: *mut u16,
    pub lpsz_domain: *mut u16,
}

/// Security description for secure sessions
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPSECURITYDESC {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub lpsz_sspi_provider: *mut u16,
    pub lpsz_capi_provider: *mut u16,
    pub dw_capi_provider_type: DWORD,
    pub dw_encryption_algorithm: DWORD,
}

/// Chat message structure
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPCHAT {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub lpsz_message: *mut u16,
}

/// Connection data for service providers
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPLCONNECTION {
    pub dw_size: DWORD,
    pub dw_flags: DWORD,
    pub lp_session_desc: *mut DPSESSIONDESC2,
    pub lp_player_name: *mut DPNAME,
    pub guid_sp: GUID,
    pub lp_address: LPVOID,
    pub dw_address_size: DWORD,
}

/// Enumerated session data passed to callback
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPENUMSESSIONDATA {
    pub lp_this_sd: *mut DPSESSIONDESC2,
    pub dw_timeout: DWORD,
    pub dw_flags: DWORD,
    pub lp_context: LPVOID,
}

/// Message header for received messages
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPMSG_HEADER {
    pub dw_type: DWORD,
}

/// System message types
pub mod sysmsg {
    use super::*;

    pub const CYCLOPSMSGID_CYCLOPSEVENT: DWORD = 0;

    // System message IDs (internal use)
    pub const CYCLOPSMSGID_CYCLOPSTRACE: DWORD = 1;

    // Player messages
    pub const CYCLOPSMSGID_CYCLOPSCHAT: DWORD = 2;

    // DirectPlay system message IDs
    pub const CYCLOPSMSGID_CYCLOPSGAMESETTINGS: DWORD = 3;

    // System message constants
    pub const DPSYS_CYCLOPSMSGID_CYCLOPSEVENTBASE: DWORD = 0x0001;
    pub const DPSYS_CREATEPLAYERORGROUP: DWORD = 0x0003;
    pub const DPSYS_DESTROYPLAYERORGROUP: DWORD = 0x0005;
    pub const DPSYS_ADDPLAYERTOGROUP: DWORD = 0x0007;
    pub const DPSYS_DELETEPLAYERFROMGROUP: DWORD = 0x0021;
    pub const DPSYS_SESSIONLOST: DWORD = 0x0031;
    pub const DPSYS_HOST: DWORD = 0x0101;
    pub const DPSYS_SETPLAYERORGROUPDATA: DWORD = 0x0102;
    pub const DPSYS_SETPLAYERORGROUPNAME: DWORD = 0x0103;
    pub const DPSYS_SETSESSIONDESC: DWORD = 0x0104;
    pub const DPSYS_ADDGROUPTOGROUP: DWORD = 0x0105;
    pub const DPSYS_DELETEGROUPFROMGROUP: DWORD = 0x0106;
    pub const DPSYS_SECUREMESSAGE: DWORD = 0x0107;
    pub const DPSYS_STARTSESSION: DWORD = 0x0108;
    pub const DPSYS_CHAT: DWORD = 0x0109;
    pub const DPSYS_SETGROUPOWNER: DWORD = 0x010A;
    pub const DPSYS_SENDCOMPLETE: DWORD = 0x010D;
}

/// Create player or group message
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPMSG_CREATEPLAYERORGROUP {
    pub dw_type: DWORD,
    pub dw_player_type: DWORD,
    pub dpid: DPID,
    pub dw_current_players: DWORD,
    pub lp_data: LPVOID,
    pub dw_data_size: DWORD,
    pub dpn_name: DPNAME,
    pub dpid_parent: DPID,
    pub dw_flags: DWORD,
}

/// Destroy player or group message
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPMSG_DESTROYPLAYERORGROUP {
    pub dw_type: DWORD,
    pub dw_player_type: DWORD,
    pub dpid: DPID,
    pub lp_local_data: LPVOID,
    pub dw_local_data_size: DWORD,
    pub lp_remote_data: LPVOID,
    pub dw_remote_data_size: DWORD,
    pub dpn_name: DPNAME,
    pub dpid_parent: DPID,
    pub dw_flags: DWORD,
}

/// Session lost message
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPMSG_SESSIONLOST {
    pub dw_type: DWORD,
}

/// Host message (you became the host)
#[repr(C)]
#[derive(Clone, Debug)]
pub struct DPMSG_HOST {
    pub dw_type: DWORD,
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert a null-terminated wide string to a Rust String
///
/// # Safety
/// The pointer must be null or point to a valid null-terminated wide string
pub unsafe fn wide_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }

    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf16_lossy(slice)
}

/// Convert a null-terminated ANSI string to a Rust String
///
/// # Safety
/// The pointer must be null or point to a valid null-terminated string
pub unsafe fn ansi_to_string(ptr: *const i8) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let cstr = std::ffi::CStr::from_ptr(ptr);
    cstr.to_string_lossy().into_owned()
}

/// Convert a Rust string to a null-terminated wide string (allocated)
pub fn string_to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert a Rust string to a null-terminated ANSI string (allocated)
pub fn string_to_ansi(s: &str) -> Vec<i8> {
    let mut bytes: Vec<i8> = s.bytes().map(|b| b as i8).collect();
    bytes.push(0);
    bytes
}

// ============================================================================
// System Message Serialization
// ============================================================================

/// Player type constant for DPMSG_* messages
/// Per Wine dplay.h: DPPLAYERTYPE_PLAYER = 1, DPPLAYERTYPE_GROUP = 0
pub const DPPLAYERTYPE_PLAYER: DWORD = 0x00000001;
pub const DPPLAYERTYPE_GROUP: DWORD = 0x00000000;

/// Serialize a DPMSG_CREATEPLAYERORGROUP message to bytes.
///
/// The format is:
/// - Fixed structure header (48 bytes)
/// - Short name string (null-terminated ANSI)
/// - Player data (if any)
///
/// The DPNAME pointers are set as offsets from the start of the buffer.
pub fn serialize_dpmsg_createplayerorgroup(
    player_id: DPID,
    player_type: DWORD,
    current_players: DWORD,
    name: &PlayerName,
    data: &[u8],
    flags: DWORD,
) -> Vec<u8> {
    // Calculate offsets
    // Structure size: 48 bytes (12 DWORDs)
    // - dwType (4)
    // - dwPlayerType (4)
    // - dpId (4)
    // - dwCurrentPlayers (4)
    // - lpData (4)
    // - dwDataSize (4)
    // - dpnName.dwSize (4)
    // - dpnName.dwFlags (4)
    // - dpnName.lpszShortNameA (4)
    // - dpnName.lpszLongNameA (4)
    // - dpIdParent (4)
    // - dwFlags (4)
    const STRUCT_SIZE: usize = 48;

    let short_name_bytes = string_to_ansi(&name.short_name);
    let short_name_offset = STRUCT_SIZE as u32;
    let data_offset = short_name_offset + short_name_bytes.len() as u32;

    let mut buf = Vec::with_capacity(STRUCT_SIZE + short_name_bytes.len() + data.len());

    // dwType
    buf.extend_from_slice(&sysmsg::DPSYS_CREATEPLAYERORGROUP.to_le_bytes());
    // dwPlayerType
    buf.extend_from_slice(&player_type.to_le_bytes());
    // dpId
    buf.extend_from_slice(&player_id.to_le_bytes());
    // dwCurrentPlayers
    buf.extend_from_slice(&current_players.to_le_bytes());
    // lpData (offset or 0 if no data)
    if data.is_empty() {
        buf.extend_from_slice(&0u32.to_le_bytes());
    } else {
        buf.extend_from_slice(&data_offset.to_le_bytes());
    }
    // dwDataSize
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    // dpnName.dwSize
    buf.extend_from_slice(&16u32.to_le_bytes()); // sizeof(DPNAME) = 16
    // dpnName.dwFlags
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dpnName.lpszShortNameA (offset to string)
    buf.extend_from_slice(&short_name_offset.to_le_bytes());
    // dpnName.lpszLongNameA (0 = no long name)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dpIdParent (0 for players)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dwFlags
    buf.extend_from_slice(&flags.to_le_bytes());

    // Append short name string (null-terminated ANSI)
    for &b in &short_name_bytes {
        buf.push(b as u8);
    }

    // Append player data
    buf.extend_from_slice(data);

    buf
}

/// Serialize a DPMSG_DESTROYPLAYERORGROUP message to bytes.
///
/// The format is:
/// - Fixed structure header
/// - Short name string (null-terminated ANSI)
pub fn serialize_dpmsg_destroyplayerorgroup(
    player_id: DPID,
    player_type: DWORD,
    name: &PlayerName,
    flags: DWORD,
) -> Vec<u8> {
    // Structure size: 56 bytes (14 DWORDs)
    // - dwType (4)
    // - dwPlayerType (4)
    // - dpId (4)
    // - lpLocalData (4)
    // - dwLocalDataSize (4)
    // - lpRemoteData (4)
    // - dwRemoteDataSize (4)
    // - dpnName.dwSize (4)
    // - dpnName.dwFlags (4)
    // - dpnName.lpszShortNameA (4)
    // - dpnName.lpszLongNameA (4)
    // - dpIdParent (4)
    // - dwFlags (4)
    const STRUCT_SIZE: usize = 52;

    let short_name_bytes = string_to_ansi(&name.short_name);
    let short_name_offset = STRUCT_SIZE as u32;

    let mut buf = Vec::with_capacity(STRUCT_SIZE + short_name_bytes.len());

    // dwType
    buf.extend_from_slice(&sysmsg::DPSYS_DESTROYPLAYERORGROUP.to_le_bytes());
    // dwPlayerType
    buf.extend_from_slice(&player_type.to_le_bytes());
    // dpId
    buf.extend_from_slice(&player_id.to_le_bytes());
    // lpLocalData (0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dwLocalDataSize (0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // lpRemoteData (0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dwRemoteDataSize (0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dpnName.dwSize
    buf.extend_from_slice(&16u32.to_le_bytes());
    // dpnName.dwFlags
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dpnName.lpszShortNameA (offset to string)
    buf.extend_from_slice(&short_name_offset.to_le_bytes());
    // dpnName.lpszLongNameA (0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dpIdParent (0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dwFlags
    buf.extend_from_slice(&flags.to_le_bytes());

    // Append short name string
    for &b in &short_name_bytes {
        buf.push(b as u8);
    }

    buf
}

/// Serialize a DPMSG_SESSIONLOST message to bytes.
pub fn serialize_dpmsg_sessionlost() -> Vec<u8> {
    sysmsg::DPSYS_SESSIONLOST.to_le_bytes().to_vec()
}

/// Serialize a DPMSG_HOST message to bytes.
pub fn serialize_dpmsg_host() -> Vec<u8> {
    sysmsg::DPSYS_HOST.to_le_bytes().to_vec()
}

/// Serialize a DPMSG_SETPLAYERORGROUPDATA message to bytes.
///
/// The format is:
/// - dwType (4 bytes): DPSYS_SETPLAYERORGROUPDATA
/// - dwPlayerType (4 bytes): DPPLAYERTYPE_PLAYER or DPPLAYERTYPE_GROUP
/// - dpId (4 bytes): Player or group ID
/// - lpData (4 bytes): Offset to data (fixed up by receiver)
/// - dwDataSize (4 bytes): Size of data
/// - [data bytes]
///
/// The lpData pointer is stored as an offset from the start of the buffer.
pub fn serialize_dpmsg_setplayerorgroupdata(
    player_id: DPID,
    player_type: DWORD,
    data: &[u8],
) -> Vec<u8> {
    // Structure size: 20 bytes (5 DWORDs)
    const STRUCT_SIZE: usize = 20;

    let data_offset = if data.is_empty() {
        0u32
    } else {
        STRUCT_SIZE as u32
    };

    let mut buf = Vec::with_capacity(STRUCT_SIZE + data.len());

    // dwType
    buf.extend_from_slice(&sysmsg::DPSYS_SETPLAYERORGROUPDATA.to_le_bytes());
    // dwPlayerType
    buf.extend_from_slice(&player_type.to_le_bytes());
    // dpId
    buf.extend_from_slice(&player_id.to_le_bytes());
    // lpData (offset or 0 if no data)
    buf.extend_from_slice(&data_offset.to_le_bytes());
    // dwDataSize
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());

    // Append data
    buf.extend_from_slice(data);

    buf
}

/// Serialize a DPMSG_SETPLAYERORGROUPNAME message to bytes.
///
/// The format is:
/// - dwType (4 bytes): DPSYS_SETPLAYERORGROUPNAME
/// - dwPlayerType (4 bytes): DPPLAYERTYPE_PLAYER or DPPLAYERTYPE_GROUP
/// - dpId (4 bytes): Player or group ID
/// - dpnName.dwSize (4 bytes): Size of DPNAME struct (16)
/// - dpnName.dwFlags (4 bytes): 0
/// - dpnName.lpszShortNameA (4 bytes): Offset to short name
/// - dpnName.lpszLongNameA (4 bytes): Offset to long name
/// - [short name string, null terminated]
/// - [long name string, null terminated]
pub fn serialize_dpmsg_setplayerorgroupname(
    player_id: DPID,
    player_type: DWORD,
    short_name: &str,
    long_name: &str,
) -> Vec<u8> {
    // Structure size: 28 bytes (7 DWORDs) before strings
    const STRUCT_SIZE: usize = 28;

    let short_name_bytes: Vec<u8> = short_name.bytes().chain(std::iter::once(0)).collect();
    let long_name_bytes: Vec<u8> = long_name.bytes().chain(std::iter::once(0)).collect();

    let short_name_offset = STRUCT_SIZE as u32;
    let long_name_offset = short_name_offset + short_name_bytes.len() as u32;

    let mut buf = Vec::with_capacity(STRUCT_SIZE + short_name_bytes.len() + long_name_bytes.len());

    // dwType
    buf.extend_from_slice(&sysmsg::DPSYS_SETPLAYERORGROUPNAME.to_le_bytes());
    // dwPlayerType
    buf.extend_from_slice(&player_type.to_le_bytes());
    // dpId
    buf.extend_from_slice(&player_id.to_le_bytes());
    // dpnName.dwSize
    buf.extend_from_slice(&16u32.to_le_bytes());
    // dpnName.dwFlags
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dpnName.lpszShortNameA (offset)
    buf.extend_from_slice(&short_name_offset.to_le_bytes());
    // dpnName.lpszLongNameA (offset)
    buf.extend_from_slice(&long_name_offset.to_le_bytes());

    // Append strings
    buf.extend_from_slice(&short_name_bytes);
    buf.extend_from_slice(&long_name_bytes);

    buf
}

/// Serialize a DPMSG_ADDPLAYERTOGROUP message to bytes.
///
/// The format is:
/// - dwType (4 bytes): DPSYS_ADDPLAYERTOGROUP (0x0007)
/// - dpIdGroup (4 bytes): Group ID
/// - dpIdPlayer (4 bytes): Player ID
pub fn serialize_dpmsg_addplayertogroup(group_id: DPID, player_id: DPID) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12);
    buf.extend_from_slice(&sysmsg::DPSYS_ADDPLAYERTOGROUP.to_le_bytes());
    buf.extend_from_slice(&group_id.to_le_bytes());
    buf.extend_from_slice(&player_id.to_le_bytes());
    buf
}

/// Serialize a DPMSG_DELETEPLAYERFROMGROUP message to bytes.
///
/// The format is:
/// - dwType (4 bytes): DPSYS_DELETEPLAYERFROMGROUP (0x0020)
/// - dpIdGroup (4 bytes): Group ID
/// - dpIdPlayer (4 bytes): Player ID
pub fn serialize_dpmsg_deleteplayerfromgroup(group_id: DPID, player_id: DPID) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12);
    buf.extend_from_slice(&sysmsg::DPSYS_DELETEPLAYERFROMGROUP.to_le_bytes());
    buf.extend_from_slice(&group_id.to_le_bytes());
    buf.extend_from_slice(&player_id.to_le_bytes());
    buf
}

/// Serialize a DPMSG_SETSESSIONDESC message to bytes.
///
/// The format is:
/// - dwType (4 bytes): DPSYS_SETSESSIONDESC
/// - [DPSESSIONDESC2 structure follows]
pub fn serialize_dpmsg_setsessiondesc(desc: &SessionDesc) -> Vec<u8> {
    let mut buf = Vec::new();

    // dwType
    buf.extend_from_slice(&sysmsg::DPSYS_SETSESSIONDESC.to_le_bytes());

    // For now, just include the basic fields the game needs
    // A full implementation would serialize the entire DPSESSIONDESC2
    // but games typically only check specific fields

    // dwSize of DPSESSIONDESC2
    buf.extend_from_slice(&80u32.to_le_bytes()); // sizeof(DPSESSIONDESC2)
    // dwFlags
    buf.extend_from_slice(&desc.flags.to_le_bytes());
    // guidInstance (16 bytes)
    buf.extend_from_slice(&desc.guid_instance.data1.to_le_bytes());
    buf.extend_from_slice(&desc.guid_instance.data2.to_le_bytes());
    buf.extend_from_slice(&desc.guid_instance.data3.to_le_bytes());
    buf.extend_from_slice(&desc.guid_instance.data4);
    // guidApplication (16 bytes)
    buf.extend_from_slice(&desc.guid_application.data1.to_le_bytes());
    buf.extend_from_slice(&desc.guid_application.data2.to_le_bytes());
    buf.extend_from_slice(&desc.guid_application.data3.to_le_bytes());
    buf.extend_from_slice(&desc.guid_application.data4);
    // dwMaxPlayers
    buf.extend_from_slice(&desc.max_players.to_le_bytes());
    // dwCurrentPlayers
    buf.extend_from_slice(&desc.current_players.to_le_bytes());
    // lpszSessionName (pointer - set to 0, name follows if needed)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // lpszPassword (pointer - set to 0)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dwReserved1, dwReserved2
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    // dwUser1-4
    buf.extend_from_slice(&desc.user1.to_le_bytes());
    buf.extend_from_slice(&desc.user2.to_le_bytes());
    buf.extend_from_slice(&desc.user3.to_le_bytes());
    buf.extend_from_slice(&desc.user4.to_le_bytes());

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_desc_size() {
        // Verify size matches expected Windows struct size
        let desc = DPSESSIONDESC2::default();
        assert_eq!(desc.dw_size as usize, std::mem::size_of::<DPSESSIONDESC2>());
    }

    #[test]
    fn test_wide_string_roundtrip() {
        let original = "Hello, DirectPlay!";
        let wide = string_to_wide(original);
        let back = unsafe { wide_to_string(wide.as_ptr()) };
        assert_eq!(original, back);
    }

    #[test]
    fn test_serialize_createplayerorgroup() {
        let name = PlayerName {
            short_name: "Player1".to_string(),
            long_name: String::new(),
        };
        let data = serialize_dpmsg_createplayerorgroup(
            0x10000, // player_id
            DPPLAYERTYPE_PLAYER,
            2, // current_players
            &name,
            &[], // no data
            0,   // flags
        );

        // Verify structure fields
        assert_eq!(&data[0..4], &sysmsg::DPSYS_CREATEPLAYERORGROUP.to_le_bytes());
        assert_eq!(&data[4..8], &DPPLAYERTYPE_PLAYER.to_le_bytes());
        assert_eq!(&data[8..12], &0x10000u32.to_le_bytes()); // player_id

        // Verify name is at expected offset (48)
        let name_offset = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
        assert_eq!(name_offset, 48);

        // Verify name string
        let name_start = name_offset as usize;
        let name_end = data.len();
        let name_bytes = &data[name_start..name_end];
        assert_eq!(name_bytes, b"Player1\0");
    }
}
