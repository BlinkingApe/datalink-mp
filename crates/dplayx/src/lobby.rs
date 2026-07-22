//! IDirectPlayLobby3 implementation
//!
//! This provides a minimal lobby object that allows DirectPlayLobbyCreateA to succeed.
//! Most methods are stubs that log and return appropriate error codes.

use dp_types::*;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::info;

/// IDirectPlayLobby3 vtable
/// Must match Windows SDK vtable layout for COM compatibility
#[repr(C)]
pub struct IDirectPlayLobby3Vtbl {
    // IUnknown methods (slots 0-2)
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        riid: *const GUID,
        ppvObject: *mut LPVOID,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut IDirectPlayLobby3) -> DWORD,
    pub Release: unsafe extern "system" fn(this: *mut IDirectPlayLobby3) -> DWORD,

    // IDirectPlayLobby methods (slots 3-13)
    pub Connect: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        lplpDP: *mut *mut IDirectPlay2,
        pUnk: *mut IUnknown,
    ) -> HRESULT,
    pub CreateAddress: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        guidSP: *const GUID,
        guidDataType: *const GUID,
        lpData: LPVOID,
        dwDataSize: DWORD,
        lpAddress: LPVOID,
        lpdwAddressSize: *mut DWORD,
    ) -> HRESULT,
    pub EnumAddress: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        lpEnumAddressCallback: LPVOID, // LPDPENUMADDRESSCALLBACK
        lpAddress: LPVOID,
        dwAddressSize: DWORD,
        lpContext: LPVOID,
    ) -> HRESULT,
    pub EnumAddressTypes: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        lpEnumAddressTypeCallback: LPVOID, // LPDPLENUMADDRESSTYPESCALLBACK
        guidSP: *const GUID,
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub EnumLocalApplications: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        lpEnumLocalAppCallback: LPVOID, // LPDPLENUMLOCALAPPLICATIONSCALLBACK
        lpContext: LPVOID,
        dwFlags: DWORD,
    ) -> HRESULT,
    pub GetConnectionSettings: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwAppID: DWORD,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub ReceiveLobbyMessage: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        dwAppID: DWORD,
        lpdwMessageFlags: *mut DWORD,
        lpData: LPVOID,
        lpdwDataSize: *mut DWORD,
    ) -> HRESULT,
    pub RunApplication: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        lpdwAppId: *mut DWORD,
        lpConn: *const DPLCONNECTION,
        hReceiveEvent: HANDLE,
    ) -> HRESULT,
    pub SendLobbyMessage: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        dwAppID: DWORD,
        lpData: LPVOID,
        dwDataSize: DWORD,
    ) -> HRESULT,
    pub SetConnectionSettings: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        dwAppID: DWORD,
        lpConn: *const DPLCONNECTION,
    ) -> HRESULT,
    pub SetLobbyMessageEvent: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        dwAppID: DWORD,
        hReceiveEvent: HANDLE,
    ) -> HRESULT,

    // IDirectPlayLobby2 methods (slot 14)
    pub CreateCompoundAddress: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        lpElements: LPVOID, // LPDPCOMPOUNDADDRESSELEMENT
        dwElementCount: DWORD,
        lpAddress: LPVOID,
        lpdwAddressSize: *mut DWORD,
    ) -> HRESULT,

    // IDirectPlayLobby3 methods (slots 15-18)
    pub ConnectEx: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        riid: *const GUID,
        lplpDP: *mut LPVOID,
        pUnk: *mut IUnknown,
    ) -> HRESULT,
    pub RegisterApplication: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        lpAppDesc: LPVOID, // LPDPAPPLICATIONDESC
    ) -> HRESULT,
    pub UnregisterApplication: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
        guidApplication: *const GUID,
    ) -> HRESULT,
    pub WaitForConnectionSettings: unsafe extern "system" fn(
        this: *mut IDirectPlayLobby3,
        dwFlags: DWORD,
    ) -> HRESULT,
}

/// Placeholder for IDirectPlay2 interface pointer
#[repr(C)]
pub struct IDirectPlay2 {
    pub lpVtbl: LPVOID,
}

/// IDirectPlayLobby3 interface
#[repr(C)]
pub struct IDirectPlayLobby3 {
    pub lpVtbl: *const IDirectPlayLobby3Vtbl,
}

/// Static vtable for DirectPlayLobby
static DIRECTPLAYLOBBY3_VTBL: IDirectPlayLobby3Vtbl = IDirectPlayLobby3Vtbl {
    QueryInterface: Lobby_QueryInterface,
    AddRef: Lobby_AddRef,
    Release: Lobby_Release,
    Connect: Lobby_Connect,
    CreateAddress: Lobby_CreateAddress,
    EnumAddress: Lobby_EnumAddress,
    EnumAddressTypes: Lobby_EnumAddressTypes,
    EnumLocalApplications: Lobby_EnumLocalApplications,
    GetConnectionSettings: Lobby_GetConnectionSettings,
    ReceiveLobbyMessage: Lobby_ReceiveLobbyMessage,
    RunApplication: Lobby_RunApplication,
    SendLobbyMessage: Lobby_SendLobbyMessage,
    SetConnectionSettings: Lobby_SetConnectionSettings,
    SetLobbyMessageEvent: Lobby_SetLobbyMessageEvent,
    CreateCompoundAddress: Lobby_CreateCompoundAddress,
    ConnectEx: Lobby_ConnectEx,
    RegisterApplication: Lobby_RegisterApplication,
    UnregisterApplication: Lobby_UnregisterApplication,
    WaitForConnectionSettings: Lobby_WaitForConnectionSettings,
};

/// DirectPlayLobby object
#[repr(C)]
pub struct DirectPlayLobbyObject {
    /// Vtable pointer (must be first for COM compatibility)
    vtbl: *const IDirectPlayLobby3Vtbl,
    /// Reference count
    ref_count: AtomicU32,
}

impl DirectPlayLobbyObject {
    /// Create a new DirectPlayLobby object
    pub fn new() -> Box<Self> {
        info!("DirectPlayLobbyObject::new()");
        Box::new(Self {
            vtbl: &DIRECTPLAYLOBBY3_VTBL,
            ref_count: AtomicU32::new(1),
        })
    }

    /// Convert to raw pointer for returning to caller
    pub fn into_raw(self: Box<Self>) -> *mut Self {
        Box::into_raw(self)
    }
}

// ============================================================================
// IUnknown methods
// ============================================================================

unsafe extern "system" fn Lobby_QueryInterface(
    this: *mut IDirectPlayLobby3,
    riid: *const GUID,
    ppvObject: *mut LPVOID,
) -> HRESULT {
    info!("Lobby_QueryInterface ENTER riid={:?}", *riid);

    if ppvObject.is_null() {
        return E_POINTER;
    }

    let riid = &*riid;

    // Check for IID_IDirectPlayLobby (W and A variants), IID_IDirectPlayLobby2, IID_IDirectPlayLobby3, or IUnknown
    if *riid == IID_IDIRECTPLAYLOBBY
        || *riid == IID_IDIRECTPLAYLOBBYA
        || *riid == IID_IDIRECTPLAYLOBBY2
        || *riid == IID_IDIRECTPLAYLOBBY2A
        || *riid == IID_IDIRECTPLAYLOBBY3
        || *riid == IID_IDIRECTPLAYLOBBY3A
        || *riid == IID_IUNKNOWN
    {
        // Return self
        *ppvObject = this as LPVOID;
        Lobby_AddRef(this);
        info!("Lobby_QueryInterface EXIT S_OK");
        return S_OK;
    }

    info!("Lobby_QueryInterface EXIT E_NOINTERFACE");
    *ppvObject = std::ptr::null_mut();
    E_NOINTERFACE
}

unsafe extern "system" fn Lobby_AddRef(this: *mut IDirectPlayLobby3) -> DWORD {
    let obj = this as *mut DirectPlayLobbyObject;
    let count = (*obj).ref_count.fetch_add(1, Ordering::SeqCst) + 1;
    info!("Lobby_AddRef -> {}", count);
    count
}

unsafe extern "system" fn Lobby_Release(this: *mut IDirectPlayLobby3) -> DWORD {
    let obj = this as *mut DirectPlayLobbyObject;
    let count = (*obj).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    info!("Lobby_Release -> {}", count);

    if count == 0 {
        info!("Lobby_Release: destroying object");
        drop(Box::from_raw(obj));
    }

    count
}

// ============================================================================
// IDirectPlayLobby methods
// ============================================================================

unsafe extern "system" fn Lobby_Connect(
    _this: *mut IDirectPlayLobby3,
    dwFlags: DWORD,
    _lplpDP: *mut *mut IDirectPlay2,
    _pUnk: *mut IUnknown,
) -> HRESULT {
    info!("Lobby_Connect ENTER dwFlags={:#x}", dwFlags);
    // This would need to return a DirectPlay interface if the game was lobby-launched
    // For now, return error indicating not launched from lobby
    info!("Lobby_Connect EXIT DPERR_NOTLOBBIED");
    DPERR_NOTLOBBIED
}

unsafe extern "system" fn Lobby_CreateAddress(
    _this: *mut IDirectPlayLobby3,
    _guidSP: *const GUID,
    _guidDataType: *const GUID,
    _lpData: LPVOID,
    _dwDataSize: DWORD,
    _lpAddress: LPVOID,
    _lpdwAddressSize: *mut DWORD,
) -> HRESULT {
    info!("Lobby_CreateAddress ENTER");
    info!("Lobby_CreateAddress EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_EnumAddress(
    _this: *mut IDirectPlayLobby3,
    _lpEnumAddressCallback: LPVOID,
    _lpAddress: LPVOID,
    _dwAddressSize: DWORD,
    _lpContext: LPVOID,
) -> HRESULT {
    info!("Lobby_EnumAddress ENTER");
    info!("Lobby_EnumAddress EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_EnumAddressTypes(
    _this: *mut IDirectPlayLobby3,
    _lpEnumAddressTypeCallback: LPVOID,
    _guidSP: *const GUID,
    _lpContext: LPVOID,
    _dwFlags: DWORD,
) -> HRESULT {
    info!("Lobby_EnumAddressTypes ENTER");
    info!("Lobby_EnumAddressTypes EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_EnumLocalApplications(
    _this: *mut IDirectPlayLobby3,
    _lpEnumLocalAppCallback: LPVOID,
    _lpContext: LPVOID,
    _dwFlags: DWORD,
) -> HRESULT {
    info!("Lobby_EnumLocalApplications ENTER");
    info!("Lobby_EnumLocalApplications EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_GetConnectionSettings(
    _this: *mut IDirectPlayLobby3,
    dwAppID: DWORD,
    lpData: LPVOID,
    lpdwDataSize: *mut DWORD,
) -> HRESULT {
    info!(
        "Lobby_GetConnectionSettings ENTER dwAppID={} lpData={:?} lpdwDataSize={:?}",
        dwAppID, lpData, lpdwDataSize
    );
    // This is called to get connection settings when launched from lobby
    // Return DPERR_NOTLOBBIED since we weren't actually launched from a lobby
    info!("Lobby_GetConnectionSettings EXIT DPERR_NOTLOBBIED");
    DPERR_NOTLOBBIED
}

unsafe extern "system" fn Lobby_ReceiveLobbyMessage(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
    _dwAppID: DWORD,
    _lpdwMessageFlags: *mut DWORD,
    _lpData: LPVOID,
    _lpdwDataSize: *mut DWORD,
) -> HRESULT {
    info!("Lobby_ReceiveLobbyMessage ENTER");
    info!("Lobby_ReceiveLobbyMessage EXIT DPERR_NOMESSAGES");
    DPERR_NOMESSAGES
}

unsafe extern "system" fn Lobby_RunApplication(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
    _lpdwAppId: *mut DWORD,
    _lpConn: *const DPLCONNECTION,
    _hReceiveEvent: HANDLE,
) -> HRESULT {
    info!("Lobby_RunApplication ENTER");
    info!("Lobby_RunApplication EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_SendLobbyMessage(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
    _dwAppID: DWORD,
    _lpData: LPVOID,
    _dwDataSize: DWORD,
) -> HRESULT {
    info!("Lobby_SendLobbyMessage ENTER");
    info!("Lobby_SendLobbyMessage EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_SetConnectionSettings(
    _this: *mut IDirectPlayLobby3,
    dwFlags: DWORD,
    dwAppID: DWORD,
    lpConn: *const DPLCONNECTION,
) -> HRESULT {
    info!(
        "Lobby_SetConnectionSettings ENTER dwFlags={:#x} dwAppID={} lpConn={:?}",
        dwFlags, dwAppID, lpConn
    );
    // Accept but do nothing - we're not actually implementing lobby functionality
    info!("Lobby_SetConnectionSettings EXIT DP_OK");
    DP_OK
}

unsafe extern "system" fn Lobby_SetLobbyMessageEvent(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
    _dwAppID: DWORD,
    _hReceiveEvent: HANDLE,
) -> HRESULT {
    info!("Lobby_SetLobbyMessageEvent ENTER");
    info!("Lobby_SetLobbyMessageEvent EXIT DP_OK");
    DP_OK
}

// ============================================================================
// IDirectPlayLobby2 methods
// ============================================================================

unsafe extern "system" fn Lobby_CreateCompoundAddress(
    _this: *mut IDirectPlayLobby3,
    _lpElements: LPVOID,
    _dwElementCount: DWORD,
    _lpAddress: LPVOID,
    _lpdwAddressSize: *mut DWORD,
) -> HRESULT {
    info!("Lobby_CreateCompoundAddress ENTER");
    info!("Lobby_CreateCompoundAddress EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

// ============================================================================
// IDirectPlayLobby3 methods
// ============================================================================

unsafe extern "system" fn Lobby_ConnectEx(
    _this: *mut IDirectPlayLobby3,
    dwFlags: DWORD,
    riid: *const GUID,
    _lplpDP: *mut LPVOID,
    _pUnk: *mut IUnknown,
) -> HRESULT {
    info!("Lobby_ConnectEx ENTER dwFlags={:#x} riid={:?}", dwFlags, *riid);
    // Same as Connect but allows requesting specific interface version
    info!("Lobby_ConnectEx EXIT DPERR_NOTLOBBIED");
    DPERR_NOTLOBBIED
}

unsafe extern "system" fn Lobby_RegisterApplication(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
    _lpAppDesc: LPVOID,
) -> HRESULT {
    info!("Lobby_RegisterApplication ENTER");
    info!("Lobby_RegisterApplication EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_UnregisterApplication(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
    _guidApplication: *const GUID,
) -> HRESULT {
    info!("Lobby_UnregisterApplication ENTER");
    info!("Lobby_UnregisterApplication EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

unsafe extern "system" fn Lobby_WaitForConnectionSettings(
    _this: *mut IDirectPlayLobby3,
    _dwFlags: DWORD,
) -> HRESULT {
    info!("Lobby_WaitForConnectionSettings ENTER");
    info!("Lobby_WaitForConnectionSettings EXIT DPERR_UNSUPPORTED");
    DPERR_UNSUPPORTED
}

// ============================================================================
// Constants needed
// ============================================================================

const S_OK: HRESULT = 0;
const E_POINTER: HRESULT = 0x80004003u32 as i32;
const E_NOINTERFACE: HRESULT = 0x80004002u32 as i32;
const DPERR_NOTLOBBIED: HRESULT = 0x88770090u32 as i32;
const DPERR_NOMESSAGES: HRESULT = 0x88770028u32 as i32;

// IID for IUnknown
const IID_IUNKNOWN: GUID = GUID {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
