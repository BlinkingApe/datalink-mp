//! DirectPlay 4 type definitions for Rust
//!
//! This crate provides Rust definitions for Microsoft DirectPlay 4 types,
//! structures, GUIDs, flags, and error codes.

pub mod guid;
pub mod structs;
pub mod flags;
pub mod hresult;
pub mod vtable;

pub use guid::*;
pub use structs::*;
pub use flags::*;
pub use hresult::*;
pub use vtable::*;

/// DirectPlay Player ID - a 32-bit identifier for players and groups
pub type DPID = u32;

/// The 1-byte JACKAL player-data blob that SMAC's `CreatePlayer` supplies as
/// `lpData` (`dwDataSize == 1`).
///
/// `RegisterPlayerInternal` (game @0x64d020) writes this byte verbatim into the
/// player table slot at `+0x168`, where:
/// - **bit0 (0x1)** = "this player is the host" (also latches the host DPID at
///   `mgr+0x764` when set),
/// - **bit1 (0x2)** = "eligible for reliable broadcast delivery".
///
/// The reliable-broadcast fan-out (`DirectPlaySendWrapper` @0x64a210) SKIPS any
/// player whose slot byte lacks bit1, silently dropping the message before it
/// reaches our DLL. So every real player must be registered with bit1 set. The
/// value is fully determined by role: host `0x03`, everyone else `0x02`. We bake
/// it into the synthesized `CREATEPLAYERORGROUP` so eligibility never depends on
/// the game's (empty-in-practice) `CreatePlayer` data or a later
/// `SETPLAYERORGROUPDATA`.
pub const fn player_data_byte(is_host: bool) -> u8 {
    if is_host {
        0x03
    } else {
        0x02
    }
}

/// Special DPID values
pub mod dpid {
    use super::DPID;

    /// System player (used for system messages)
    pub const DPID_SYSMSG: DPID = 0;
    /// Broadcast to all players
    pub const DPID_ALLPLAYERS: DPID = 0;
    /// The server/host player - per Windows SDK dplay.h this is 1, not 0xFFFFFFFF
    pub const DPID_SERVERPLAYER: DPID = 1;
    /// Unknown player
    pub const DPID_UNKNOWN: DPID = 0xFFFFFFFF;
}

/// Windows types used by DirectPlay
pub mod win_types {
    pub type HRESULT = i32;
    pub type DWORD = u32;
    pub type WORD = u16;
    pub type BYTE = u8;
    pub type BOOL = i32;
    pub type LPVOID = *mut core::ffi::c_void;
    pub type LPCVOID = *const core::ffi::c_void;
    pub type HANDLE = *mut core::ffi::c_void;
    pub type HWND = *mut core::ffi::c_void;

    pub const TRUE: BOOL = 1;
    pub const FALSE: BOOL = 0;
}

pub use win_types::*;
