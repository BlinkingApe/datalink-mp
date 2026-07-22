//! COM vtable definitions for DirectPlay interfaces
//!
//! This module defines the vtable layout for IDirectPlay4 and related interfaces.
//! The vtables must exactly match the Windows SDK layout for COM compatibility.
//!
//! We use `extern "system"` which is `stdcall` on Windows and `C` elsewhere,
//! allowing compilation on Linux for testing.

// Windows API uses PascalCase for COM methods and parameters - we must match exactly
#![allow(non_snake_case)]

use crate::{
    BOOL, DPCAPS, DPCHAT, DPCREDENTIALS, DPID, DPLCONNECTION, DPNAME, DPSECURITYDESC,
    DPSESSIONDESC2, DWORD, GUID, HANDLE, HRESULT, LPVOID,
};

/// Callback for EnumSessions
pub type LPDPENUMSESSIONSCALLBACK2 = Option<
    unsafe extern "system" fn(
        lpThisSD: *const DPSESSIONDESC2,
        lpdwTimeOut: *mut DWORD,
        dwFlags: DWORD,
        lpContext: LPVOID,
    ) -> BOOL,
>;

/// Callback for EnumPlayers/EnumGroups
pub type LPDPENUMPLAYERSCALLBACK2 = Option<
    unsafe extern "system" fn(
        dpId: DPID,
        dwPlayerType: DWORD,
        lpName: *const DPNAME,
        dwFlags: DWORD,
        lpContext: LPVOID,
    ) -> BOOL,
>;

/// Callback for EnumConnections
pub type LPDPENUMCONNECTIONSCALLBACK = Option<
    unsafe extern "system" fn(
        lpguidSP: *const GUID,
        lpConnection: LPVOID,
        dwConnectionSize: DWORD,
        lpName: *const DPNAME,
        dwFlags: DWORD,
        lpContext: LPVOID,
    ) -> BOOL,
>;

/// Callback for DirectPlayEnumerate
pub type LPDPENUMDPCALLBACK =
    Option<unsafe extern "system" fn(lpguidSP: *const GUID, lpSPName: *const i8, dwMajorVersion: DWORD, dwMinorVersion: DWORD, lpContext: LPVOID) -> BOOL>;

/// Callback for DirectPlayEnumerateW
pub type LPDPENUMDPCALLBACKW =
    Option<unsafe extern "system" fn(lpguidSP: *const GUID, lpSPName: *const u16, dwMajorVersion: DWORD, dwMinorVersion: DWORD, lpContext: LPVOID) -> BOOL>;

/// IUnknown vtable
#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface:
        unsafe extern "system" fn(this: *mut IUnknown, riid: *const GUID, ppvObject: *mut LPVOID) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut IUnknown) -> DWORD,
    pub Release: unsafe extern "system" fn(this: *mut IUnknown) -> DWORD,
}

/// IUnknown interface
#[repr(C)]
pub struct IUnknown {
    pub lpVtbl: *const IUnknownVtbl,
}

/// IClassFactory vtable
#[repr(C)]
pub struct IClassFactoryVtbl {
    // IUnknown methods
    pub QueryInterface:
        unsafe extern "system" fn(this: *mut IClassFactory, riid: *const GUID, ppvObject: *mut LPVOID) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut IClassFactory) -> DWORD,
    pub Release: unsafe extern "system" fn(this: *mut IClassFactory) -> DWORD,
    // IClassFactory methods
    pub CreateInstance: unsafe extern "system" fn(
        this: *mut IClassFactory,
        pUnkOuter: *mut IUnknown,
        riid: *const GUID,
        ppvObject: *mut LPVOID,
    ) -> HRESULT,
    pub LockServer: unsafe extern "system" fn(this: *mut IClassFactory, fLock: BOOL) -> HRESULT,
}

/// IClassFactory interface
#[repr(C)]
pub struct IClassFactory {
    pub lpVtbl: *const IClassFactoryVtbl,
}

/// IDirectPlay4 vtable (Unicode version)
/// This must exactly match the Windows SDK vtable layout
#[repr(C)]
pub struct IDirectPlay4Vtbl {
    // IUnknown methods (slots 0-2)
    pub QueryInterface:
        unsafe extern "system" fn(this: *mut IDirectPlay4, riid: *const GUID, ppvObject: *mut LPVOID) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut IDirectPlay4) -> DWORD,
    pub Release: unsafe extern "system" fn(this: *mut IDirectPlay4) -> DWORD,

    // IDirectPlay2 methods (slots 3-31)
    pub AddPlayerToGroup:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID, idPlayer: DPID) -> HRESULT,
    pub Close: unsafe extern "system" fn(this: *mut IDirectPlay4) -> HRESULT,
    pub CreateGroup: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpidGroup: *mut DPID,
        lpGroupName: *const DPNAME,
        lpData: LPVOID,
        dwDataSize: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub CreatePlayer: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpidPlayer: *mut DPID,
        lpPlayerName: *const DPNAME,
        hEvent: HANDLE,
        lpData: LPVOID,
        dwDataSize: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub DeletePlayerFromGroup:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID, idPlayer: DPID) -> HRESULT,
    pub DestroyGroup: unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID) -> HRESULT,
    pub DestroyPlayer: unsafe extern "system" fn(this: *mut IDirectPlay4, idPlayer: DPID) -> HRESULT,
    pub EnumGroupPlayers: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idGroup: DPID,
        lpguidInstance: *const GUID,
        lpEnumPlayersCallback2: LPDPENUMPLAYERSCALLBACK2,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub EnumGroups: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpguidInstance: *const GUID,
        lpEnumPlayersCallback2: LPDPENUMPLAYERSCALLBACK2,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub EnumPlayers: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpguidInstance: *const GUID,
        lpEnumPlayersCallback2: LPDPENUMPLAYERSCALLBACK2,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub EnumSessions: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpsd: *const DPSESSIONDESC2,
        dwTimeout: DWORD,
        lpEnumSessionsCallback2: LPDPENUMSESSIONSCALLBACK2,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetCaps: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpDPCaps: *mut DPCAPS,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetGroupData: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idGroup: DPID,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetGroupName: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idGroup: DPID,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub GetMessageCount:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idPlayer: DPID, lpdwCount: *mut DWORD) -> HRESULT,
    pub GetPlayerAddress: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub GetPlayerCaps: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        lpPlayerCaps: *mut DPCAPS,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetPlayerData: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetPlayerName: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub GetSessionDesc: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub Initialize:
        unsafe extern "system" fn(this: *mut IDirectPlay4, lpGUID: *const GUID) -> HRESULT,
    pub Open: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpsd: *mut DPSESSIONDESC2,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub Receive: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpidFrom: *mut DPID,
        lpidTo: *mut DPID,
        dwFlags: DWORD,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub Send: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idFrom: DPID,
        idTo: DPID,
        dwFlags: DWORD,
        lpData: LPVOID,
        dwDataSize: DWORD,
    ) -> HRESULT,
    pub SetGroupData: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idGroup: DPID,
        lpData: LPVOID,
        dwDataSize: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub SetGroupName: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idGroup: DPID,
        lpGroupName: *const DPNAME,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub SetPlayerData: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        lpData: LPVOID,
        dwDataSize: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub SetPlayerName: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        lpPlayerName: *const DPNAME,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub SetSessionDesc: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpSessDesc: *mut DPSESSIONDESC2,
        dwFlags: DWORD,
    ) -> HRESULT,

    // IDirectPlay3 methods (slots 32-46)
    pub AddGroupToGroup:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idParentGroup: DPID, idGroup: DPID) -> HRESULT,
    pub CreateGroupInGroup: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idParentGroup: DPID,
        lpidGroup: *mut DPID,
        lpGroupName: *const DPNAME,
        lpData: LPVOID,
        dwDataSize: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub DeleteGroupFromGroup:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idParentGroup: DPID, idGroup: DPID) -> HRESULT,
    pub EnumConnections: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpguidApplication: *const GUID,
        lpEnumCallback: LPDPENUMCONNECTIONSCALLBACK,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub EnumGroupsInGroup: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idGroup: DPID,
        lpguidInstance: *const GUID,
        lpEnumCallback: LPDPENUMPLAYERSCALLBACK2,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetGroupConnectionSettings: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        dwFlags: DWORD,
        idGroup: DPID,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub InitializeConnection:
        unsafe extern "system" fn(this: *mut IDirectPlay4, lpConnection: LPVOID, dwFlags: DWORD) -> HRESULT,
    pub SecureOpen: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        lpsd: *const DPSESSIONDESC2,
        dwFlags: DWORD,
        lpSecurity: *const DPSECURITYDESC,
        lpCredentials: *const DPCREDENTIALS,
    ) -> HRESULT,
    pub SendChatMessage: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idFrom: DPID,
        idTo: DPID,
        dwFlags: DWORD,
        lpChatMessage: *const DPCHAT,
    ) -> HRESULT,
    pub SetGroupConnectionSettings: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        dwFlags: DWORD,
        idGroup: DPID,
        lpConnection: *mut DPLCONNECTION,
    ) -> HRESULT,
    pub StartSession:
        unsafe extern "system" fn(this: *mut IDirectPlay4, dwFlags: DWORD, idGroup: DPID) -> HRESULT,
    pub GetGroupFlags:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID, lpdwFlags: *mut DWORD) -> HRESULT,
    pub GetGroupParent:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID, lpidParent: *mut DPID) -> HRESULT,
    pub GetPlayerAccount: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idPlayer: DPID,
        dwFlags: DWORD,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub GetPlayerFlags:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idPlayer: DPID, lpdwFlags: *mut DWORD) -> HRESULT,

    // IDirectPlay4 methods (slots 47-52)
    pub GetGroupOwner:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID, lpidOwner: *mut DPID) -> HRESULT,
    pub SetGroupOwner:
        unsafe extern "system" fn(this: *mut IDirectPlay4, idGroup: DPID, idOwner: DPID) -> HRESULT,
    pub SendEx: unsafe extern "system" fn(
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
    ) -> HRESULT,
    pub GetMessageQueue: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        idFrom: DPID,
        idTo: DPID,
        dwFlags: DWORD,
        lpdwNumMsgs: *mut DWORD,
        lpdwNumBytes: *mut DWORD,
    ) -> HRESULT,
    pub CancelMessage:
        unsafe extern "system" fn(this: *mut IDirectPlay4, dwMsgID: DWORD, dwFlags: DWORD) -> HRESULT,
    pub CancelPriority: unsafe extern "system" fn(
        this: *mut IDirectPlay4,
        dwMinPriority: DWORD,
        dwMaxPriority: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT,
}

/// IDirectPlay4 interface
#[repr(C)]
pub struct IDirectPlay4 {
    pub lpVtbl: *const IDirectPlay4Vtbl,
}

// Verify vtable slot counts
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vtable_size() {
        // IDirectPlay4 should have 53 methods total
        // 3 from IUnknown + 29 from IDirectPlay2 + 15 from IDirectPlay3 + 6 from IDirectPlay4
        let vtbl_size = std::mem::size_of::<IDirectPlay4Vtbl>();
        let ptr_size = std::mem::size_of::<usize>();
        let expected_methods = 53;
        assert_eq!(vtbl_size, ptr_size * expected_methods);
    }
}
