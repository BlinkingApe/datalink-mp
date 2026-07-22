//! Additional exports and helpers
//!
//! This module provides any additional functionality needed for the DLL.

// Most exports are in lib.rs directly
// This file is for any helpers or additional exports

use dp_types::*;

/// Helper to check if a GUID matches any of the DirectPlay interface IDs
pub fn is_directplay_interface(guid: &GUID) -> bool {
    *guid == IID_IDIRECTPLAY
        || *guid == IID_IDIRECTPLAY2
        || *guid == IID_IDIRECTPLAY2A
        || *guid == IID_IDIRECTPLAY3
        || *guid == IID_IDIRECTPLAY3A
        || *guid == IID_IDIRECTPLAY4
        || *guid == IID_IDIRECTPLAY4A
}

/// Helper to check if a GUID matches any of the DirectPlayLobby interface IDs
pub fn is_lobby_interface(guid: &GUID) -> bool {
    *guid == IID_IDIRECTPLAYLOBBY || *guid == IID_IDIRECTPLAYLOBBY2 || *guid == IID_IDIRECTPLAYLOBBY3
}
