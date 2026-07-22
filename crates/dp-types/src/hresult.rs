//! DirectPlay HRESULT error codes

use crate::HRESULT;
use thiserror::Error;

/// Success code
pub const DP_OK: HRESULT = 0;
pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;

// Standard Windows error codes
pub const E_NOTIMPL: HRESULT = 0x80004001_u32 as i32;
pub const E_NOINTERFACE: HRESULT = 0x80004002_u32 as i32;
pub const E_POINTER: HRESULT = 0x80004003_u32 as i32;
pub const E_FAIL: HRESULT = 0x80004005_u32 as i32;
pub const E_OUTOFMEMORY: HRESULT = 0x8007000E_u32 as i32;
pub const E_INVALIDARG: HRESULT = 0x80070057_u32 as i32;

pub const CLASS_E_NOAGGREGATION: HRESULT = 0x80040110_u32 as i32;
pub const CLASS_E_CLASSNOTAVAILABLE: HRESULT = 0x80040111_u32 as i32;
pub const REGDB_E_CLASSNOTREG: HRESULT = 0x80040154_u32 as i32;

// ============================================================================
// DirectPlay Error Codes (DPERR_*)
// ============================================================================

/// Facility code for DirectPlay (per Windows SDK)
const FACILITY_DPLAY: u32 = 0x877;

/// Create a DirectPlay error code
const fn make_dperr(code: u32) -> HRESULT {
    (0x80000000 | (FACILITY_DPLAY << 16) | code) as i32
}

/// Access denied
pub const DPERR_ACCESSDENIED: HRESULT = make_dperr(0x000A);
/// Already initialized
pub const DPERR_ALREADYINITIALIZED: HRESULT = make_dperr(0x0014);
/// Sending player isn't in group
pub const DPERR_APPNOTSTARTED: HRESULT = make_dperr(0x001E);
/// Authentication failed
pub const DPERR_AUTHENTICATIONFAILED: HRESULT = make_dperr(0x0834);
/// Connection has been refused
pub const DPERR_BUFFERTOOSMALL: HRESULT = make_dperr(0x0028);
/// Busy with another operation
pub const DPERR_BUSY: HRESULT = make_dperr(0x0032);
/// Connection was lost
pub const DPERR_CANCELFAILED: HRESULT = make_dperr(0x0037);
/// Cannot complete request
pub const DPERR_CANCELLED: HRESULT = make_dperr(0x0038);
/// Cannot create player
pub const DPERR_CANTADDPLAYER: HRESULT = make_dperr(0x003C);
/// Cannot create group
pub const DPERR_CANTCREATEGROUP: HRESULT = make_dperr(0x0046);
/// Cannot create player
pub const DPERR_CANTCREATEPLAYER: HRESULT = make_dperr(0x0050);
/// Cannot create process
pub const DPERR_CANTCREATEPROCESS: HRESULT = make_dperr(0x005A);
/// Cannot create session
pub const DPERR_CANTCREATESESSION: HRESULT = make_dperr(0x0064);
/// Cannot load SSP
pub const DPERR_CANTLOADSSPI: HRESULT = make_dperr(0x006E);
/// Cannot load CAPI
pub const DPERR_CANTLOADCAPI: HRESULT = make_dperr(0x0078);
/// Couldn't connect
pub const DPERR_CAPSNOTAVAILABLEYET: HRESULT = make_dperr(0x0082);
/// Connection lost
pub const DPERR_CONNECTIONLOST: HRESULT = make_dperr(0x008C);
/// Conversation failed
pub const DPERR_CONVERSATIONFAILED: HRESULT = make_dperr(0x008D);
/// Encryption error
pub const DPERR_ENCRYPTIONERROR: HRESULT = make_dperr(0x008E);
/// Exception error
pub const DPERR_EXCEPTION: HRESULT = make_dperr(0x0096);
/// Generic failure
pub const DPERR_GENERIC: HRESULT = E_FAIL;
/// Invalid flags
pub const DPERR_INVALIDFLAGS: HRESULT = make_dperr(0x00A0);
/// Invalid group
pub const DPERR_INVALIDGROUP: HRESULT = make_dperr(0x00AA);
/// Invalid interface
pub const DPERR_INVALIDINTERFACE: HRESULT = make_dperr(0x00B4);
/// Invalid object
pub const DPERR_INVALIDOBJECT: HRESULT = make_dperr(0x00BE);
/// Invalid parameter
pub const DPERR_INVALIDPARAMS: HRESULT = make_dperr(0x00C8);
/// Invalid password
pub const DPERR_INVALIDPASSWORD: HRESULT = make_dperr(0x00D2);
/// Invalid player
pub const DPERR_INVALIDPLAYER: HRESULT = make_dperr(0x00DC);
/// Logged in elsewhere
pub const DPERR_LOGONDENIED: HRESULT = make_dperr(0x0834);
/// No connection to server
pub const DPERR_NOCONNECTION: HRESULT = make_dperr(0x00E6);
/// No messages available
pub const DPERR_NOMESSAGES: HRESULT = make_dperr(0x00F0);
/// No name servers
pub const DPERR_NONAMESERVERFOUND: HRESULT = make_dperr(0x00FA);
/// No players
pub const DPERR_NOPLAYERS: HRESULT = make_dperr(0x0104);
/// No sessions available
pub const DPERR_NOSESSIONS: HRESULT = make_dperr(0x010E);
/// Not logged in
pub const DPERR_NOTLOBBIED: HRESULT = make_dperr(0x011C);
/// Notification failed
pub const DPERR_NOTLOGGEDIN: HRESULT = make_dperr(0x0118);
/// Not available
pub const DPERR_OUTOFMEMORY: HRESULT = E_OUTOFMEMORY;
/// Out of memory
pub const DPERR_PENDING: HRESULT = make_dperr(0x1000);
/// Receiving error
pub const DPERR_PLAYERLOST: HRESULT = make_dperr(0x0122);
/// Send error
pub const DPERR_SENDTOOBIG: HRESULT = make_dperr(0x012C);
/// Session lost
pub const DPERR_SESSIONLOST: HRESULT = make_dperr(0x0136);
/// Not initialized
pub const DPERR_SIGNFAILED: HRESULT = make_dperr(0x0140);
/// Timed out
pub const DPERR_TIMEOUT: HRESULT = make_dperr(0x014A);
/// Unknown application GUID
pub const DPERR_UNAVAILABLE: HRESULT = make_dperr(0x0154);
/// Unknown group
pub const DPERR_UNINITIALIZED: HRESULT = make_dperr(0x015E);
/// Unknown player
pub const DPERR_UNKNOWNAPPLICATION: HRESULT = make_dperr(0x0168);
/// Unknown session
pub const DPERR_UNSUPPORTED: HRESULT = make_dperr(0x0172);
/// User cancelled
pub const DPERR_USERCANCEL: HRESULT = make_dperr(0x017C);

/// Helper to check if HRESULT indicates success
#[inline]
pub const fn succeeded(hr: HRESULT) -> bool {
    hr >= 0
}

/// Helper to check if HRESULT indicates failure
#[inline]
pub const fn failed(hr: HRESULT) -> bool {
    hr < 0
}

/// DirectPlay error enum for Rust error handling
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DpError {
    #[error("Access denied")]
    AccessDenied,
    #[error("Already initialized")]
    AlreadyInitialized,
    #[error("Buffer too small")]
    BufferTooSmall,
    #[error("Busy")]
    Busy,
    #[error("Operation cancelled")]
    Cancelled,
    #[error("Cannot add player")]
    CantAddPlayer,
    #[error("Cannot create group")]
    CantCreateGroup,
    #[error("Cannot create player")]
    CantCreatePlayer,
    #[error("Cannot create session")]
    CantCreateSession,
    #[error("Connection lost")]
    ConnectionLost,
    #[error("Generic failure")]
    Generic,
    #[error("Invalid flags")]
    InvalidFlags,
    #[error("Invalid group")]
    InvalidGroup,
    #[error("Invalid object")]
    InvalidObject,
    #[error("Invalid parameters")]
    InvalidParams,
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Invalid player")]
    InvalidPlayer,
    #[error("No connection")]
    NoConnection,
    #[error("No messages available")]
    NoMessages,
    #[error("No sessions available")]
    NoSessions,
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Pending")]
    Pending,
    #[error("Player lost")]
    PlayerLost,
    #[error("Send too big")]
    SendTooBig,
    #[error("Session lost")]
    SessionLost,
    #[error("Timeout")]
    Timeout,
    #[error("Unavailable")]
    Unavailable,
    #[error("Uninitialized")]
    Uninitialized,
    #[error("Unsupported")]
    Unsupported,
    #[error("User cancelled")]
    UserCancel,
    #[error("Unknown error: {0:#X}")]
    Unknown(HRESULT),
}

impl From<HRESULT> for DpError {
    fn from(hr: HRESULT) -> Self {
        match hr {
            DPERR_ACCESSDENIED => DpError::AccessDenied,
            DPERR_ALREADYINITIALIZED => DpError::AlreadyInitialized,
            DPERR_BUFFERTOOSMALL => DpError::BufferTooSmall,
            DPERR_BUSY => DpError::Busy,
            DPERR_CANCELLED => DpError::Cancelled,
            DPERR_CANTADDPLAYER => DpError::CantAddPlayer,
            DPERR_CANTCREATEGROUP => DpError::CantCreateGroup,
            DPERR_CANTCREATEPLAYER => DpError::CantCreatePlayer,
            DPERR_CANTCREATESESSION => DpError::CantCreateSession,
            DPERR_CONNECTIONLOST => DpError::ConnectionLost,
            DPERR_GENERIC => DpError::Generic, // Note: DPERR_GENERIC == E_FAIL
            DPERR_INVALIDFLAGS => DpError::InvalidFlags,
            DPERR_INVALIDGROUP => DpError::InvalidGroup,
            DPERR_INVALIDOBJECT => DpError::InvalidObject,
            DPERR_INVALIDPARAMS | E_INVALIDARG => DpError::InvalidParams,
            DPERR_INVALIDPASSWORD => DpError::InvalidPassword,
            DPERR_INVALIDPLAYER => DpError::InvalidPlayer,
            DPERR_NOCONNECTION => DpError::NoConnection,
            DPERR_NOMESSAGES => DpError::NoMessages,
            DPERR_NOSESSIONS => DpError::NoSessions,
            DPERR_NOTLOGGEDIN => DpError::NotLoggedIn,
            DPERR_OUTOFMEMORY => DpError::OutOfMemory, // Note: DPERR_OUTOFMEMORY == E_OUTOFMEMORY
            DPERR_PENDING => DpError::Pending,
            DPERR_PLAYERLOST => DpError::PlayerLost,
            DPERR_SENDTOOBIG => DpError::SendTooBig,
            DPERR_SESSIONLOST => DpError::SessionLost,
            DPERR_TIMEOUT => DpError::Timeout,
            DPERR_UNAVAILABLE => DpError::Unavailable,
            DPERR_UNINITIALIZED => DpError::Uninitialized,
            DPERR_UNSUPPORTED => DpError::Unsupported,
            DPERR_USERCANCEL => DpError::UserCancel,
            _ => DpError::Unknown(hr),
        }
    }
}

impl From<DpError> for HRESULT {
    fn from(err: DpError) -> Self {
        match err {
            DpError::AccessDenied => DPERR_ACCESSDENIED,
            DpError::AlreadyInitialized => DPERR_ALREADYINITIALIZED,
            DpError::BufferTooSmall => DPERR_BUFFERTOOSMALL,
            DpError::Busy => DPERR_BUSY,
            DpError::Cancelled => DPERR_CANCELLED,
            DpError::CantAddPlayer => DPERR_CANTADDPLAYER,
            DpError::CantCreateGroup => DPERR_CANTCREATEGROUP,
            DpError::CantCreatePlayer => DPERR_CANTCREATEPLAYER,
            DpError::CantCreateSession => DPERR_CANTCREATESESSION,
            DpError::ConnectionLost => DPERR_CONNECTIONLOST,
            DpError::Generic => DPERR_GENERIC,
            DpError::InvalidFlags => DPERR_INVALIDFLAGS,
            DpError::InvalidGroup => DPERR_INVALIDGROUP,
            DpError::InvalidObject => DPERR_INVALIDOBJECT,
            DpError::InvalidParams => DPERR_INVALIDPARAMS,
            DpError::InvalidPassword => DPERR_INVALIDPASSWORD,
            DpError::InvalidPlayer => DPERR_INVALIDPLAYER,
            DpError::NoConnection => DPERR_NOCONNECTION,
            DpError::NoMessages => DPERR_NOMESSAGES,
            DpError::NoSessions => DPERR_NOSESSIONS,
            DpError::NotLoggedIn => DPERR_NOTLOGGEDIN,
            DpError::OutOfMemory => DPERR_OUTOFMEMORY,
            DpError::Pending => DPERR_PENDING,
            DpError::PlayerLost => DPERR_PLAYERLOST,
            DpError::SendTooBig => DPERR_SENDTOOBIG,
            DpError::SessionLost => DPERR_SESSIONLOST,
            DpError::Timeout => DPERR_TIMEOUT,
            DpError::Unavailable => DPERR_UNAVAILABLE,
            DpError::Uninitialized => DPERR_UNINITIALIZED,
            DpError::Unsupported => DPERR_UNSUPPORTED,
            DpError::UserCancel => DPERR_USERCANCEL,
            DpError::Unknown(hr) => hr,
        }
    }
}

/// Result type using DpError
pub type DpResult<T> = Result<T, DpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_succeeded() {
        assert!(succeeded(DP_OK));
        assert!(succeeded(S_FALSE));
        assert!(!succeeded(DPERR_GENERIC));
    }

    #[test]
    fn test_error_roundtrip() {
        let hr = DPERR_INVALIDPARAMS;
        let err = DpError::from(hr);
        let hr2: HRESULT = err.into();
        assert_eq!(hr, hr2);
    }
}
