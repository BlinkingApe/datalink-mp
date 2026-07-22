//! DirectPlay 4 DLL replacement using Iroh networking
//!
//! This crate implements dplayx.dll as a drop-in replacement that routes
//! all network traffic through Iroh P2P.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)] // Many DirectPlay stubs have unused parameters
#![allow(unused_imports)]

mod com;
mod directplay;
mod exports;
mod globals;
mod ipc_client;
mod lobby;

pub use com::*;
pub use directplay::*;
pub use exports::*;
pub use globals::*;

use dp_types::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, error, info};

/// Global service provider reference count (exported)
pub static GDWDPLAYSPREFCOUNT: AtomicU32 = AtomicU32::new(0);

/// DLL module handle (reserved for future use)
#[allow(dead_code)]
static DLL_HANDLE: Mutex<usize> = Mutex::new(0);

/// Initialize logging and panic handler
///
/// If DPLAYX_LOG_FILE is set, logs are written to that file.
/// Otherwise, logs go to stdout.
fn init_logging() {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::Mutex as StdMutex;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        // Set up panic hook first
        let log_path_for_panic = std::env::var("DPLAYX_LOG_FILE").ok();
        std::panic::set_hook(Box::new(move |panic_info| {
            let msg = format!("PANIC: {}\n", panic_info);

            // Try to write to log file
            if let Some(ref path) = log_path_for_panic {
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = file.write_all(msg.as_bytes());
                    let _ = file.flush();
                }
            }

            // Also write to stderr
            eprintln!("{}", msg);
        }));

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("debug"));

        // Check for file logging
        if let Ok(log_path) = std::env::var("DPLAYX_LOG_FILE") {
            // Create parent directory if it doesn't exist
            let path = std::path::Path::new(&log_path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                let file = StdMutex::new(file);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(
                        fmt::layer()
                            .with_writer(move || file.lock().unwrap().try_clone().unwrap())
                            .with_ansi(false)
                            .with_target(false),
                    )
                    .init();
                return;
            }
        }

        // No log file specified - disable logging entirely
        // Writing to stdout in a GUI Wine app can cause crashes
        // Just initialize with a no-op subscriber
        use tracing_subscriber::layer::SubscriberExt;
        tracing_subscriber::registry().init();
    });
}

/// DLL entry point (Windows)
#[no_mangle]
#[cfg(target_os = "windows")]
pub unsafe extern "system" fn DllMain(
    hinstDLL: usize,
    fdwReason: u32,
    _lpvReserved: *mut core::ffi::c_void,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    const DLL_PROCESS_DETACH: u32 = 0;
    const DLL_THREAD_ATTACH: u32 = 2;
    const DLL_THREAD_DETACH: u32 = 3;

    match fdwReason {
        DLL_PROCESS_ATTACH => {
            init_logging();
            info!("DllMain: DLL_PROCESS_ATTACH hinstDLL={:#x}", hinstDLL);
            *DLL_HANDLE.lock() = hinstDLL;

            // Apply game bug fixes (blit fix, etc.)
            smac_fixes::apply_all_patches();

            // Install debug probes for game start investigation
            // Set SMAC_PROBE_LOG env var to enable probe logging
            smac_fixes::install_debug_probes();

            1 // TRUE
        }
        DLL_PROCESS_DETACH => {
            info!("DllMain: DLL_PROCESS_DETACH");
            globals::cleanup();
            info!("DllMain: cleanup complete");
            1 // TRUE
        }
        DLL_THREAD_ATTACH => {
            info!("DllMain: DLL_THREAD_ATTACH");
            1 // TRUE
        }
        DLL_THREAD_DETACH => {
            info!("DllMain: DLL_THREAD_DETACH");
            1 // TRUE
        }
        _ => {
            info!("DllMain: unknown fdwReason={}", fdwReason);
            1 // TRUE
        }
    }
}

/// DLL entry point (non-Windows, for testing)
#[cfg(not(target_os = "windows"))]
pub fn dll_init() {
    init_logging();
    info!("DirectPlay-Iroh bridge initialized (non-Windows)");
}

// ============================================================================
// DLL Exports
// ============================================================================

/// DirectPlayCreate - Creates a DirectPlay object
///
/// # Safety
/// All pointers must be valid
#[no_mangle]
pub unsafe extern "system" fn DirectPlayCreate(
    lpGUID: *const GUID,
    lplpDP: *mut *mut IDirectPlay4,
    pUnkOuter: *mut IUnknown,
) -> HRESULT {
    init_logging();
    info!("DirectPlayCreate ENTER lpGUID={:?} lplpDP={:?} pUnkOuter={:?}", lpGUID, lplpDP, pUnkOuter);

    if lplpDP.is_null() {
        info!("DirectPlayCreate EXIT DPERR_INVALIDPARAMS (null lplpDP)");
        return DPERR_INVALIDPARAMS;
    }

    // Don't support aggregation
    if !pUnkOuter.is_null() {
        info!("DirectPlayCreate EXIT CLASS_E_NOAGGREGATION");
        return CLASS_E_NOAGGREGATION;
    }

    // Create DirectPlay object
    match directplay::DirectPlayObject::new() {
        Ok(mut obj) => {
            // If a service provider GUID was passed, auto-initialize with it
            if !lpGUID.is_null() {
                let guid = *lpGUID;
                info!("DirectPlayCreate: auto-initializing with GUID {:?}", guid);
                obj.initialize_with_guid(guid);
            }
            let ptr = Box::into_raw(Box::new(obj));
            *lplpDP = ptr as *mut IDirectPlay4;
            info!("DirectPlayCreate EXIT DP_OK ptr={:?}", ptr);
            DP_OK
        }
        Err(e) => {
            error!("DirectPlayCreate EXIT DPERR_GENERIC: {:?}", e);
            DPERR_GENERIC
        }
    }
}

/// DirectPlayEnumerateA - Enumerate service providers (ANSI)
///
/// # Safety
/// Callback and context must be valid
#[no_mangle]
pub unsafe extern "system" fn DirectPlayEnumerateA(
    lpCallback: LPDPENUMDPCALLBACK,
    lpContext: LPVOID,
) -> HRESULT {
    init_logging();
    info!("DirectPlayEnumerateA ENTER lpCallback={:?} lpContext={:?}", lpCallback, lpContext);

    if let Some(callback) = lpCallback {
        // Report our Iroh service provider
        let name = b"Iroh P2P\0";
        info!("DirectPlayEnumerateA calling callback for Iroh P2P");
        let cont = callback(
            &DPSPGUID_IROH,
            name.as_ptr() as *const i8,
            1, // Major version
            0, // Minor version
            lpContext,
        );
        info!("DirectPlayEnumerateA callback returned {}", cont);

        if cont == 0 {
            info!("DirectPlayEnumerateA EXIT DP_OK (callback stopped)");
            return DP_OK;
        }

        // Also report TCP/IP for compatibility
        let name_tcp = b"TCP/IP Connection For DirectPlay\0";
        info!("DirectPlayEnumerateA calling callback for TCP/IP");
        callback(
            &DPSPGUID_TCPIP,
            name_tcp.as_ptr() as *const i8,
            1,
            0,
            lpContext,
        );
    }

    info!("DirectPlayEnumerateA EXIT DP_OK");
    DP_OK
}

/// DirectPlayEnumerateW - Enumerate service providers (Unicode)
///
/// # Safety
/// Callback and context must be valid
#[no_mangle]
pub unsafe extern "system" fn DirectPlayEnumerateW(
    lpCallback: LPDPENUMDPCALLBACKW,
    lpContext: LPVOID,
) -> HRESULT {
    init_logging();
    info!("DirectPlayEnumerateW ENTER lpCallback={:?} lpContext={:?}", lpCallback, lpContext);

    if let Some(callback) = lpCallback {
        // Report our Iroh service provider
        let name: Vec<u16> = "Iroh P2P\0".encode_utf16().collect();
        info!("DirectPlayEnumerateW calling callback for Iroh P2P");
        let cont = callback(
            &DPSPGUID_IROH,
            name.as_ptr(),
            1, // Major version
            0, // Minor version
            lpContext,
        );
        info!("DirectPlayEnumerateW callback returned {}", cont);

        if cont == 0 {
            info!("DirectPlayEnumerateW EXIT DP_OK (callback stopped)");
            return DP_OK;
        }

        // Also report TCP/IP for compatibility
        let name_tcp: Vec<u16> = "TCP/IP Connection For DirectPlay\0"
            .encode_utf16()
            .collect();
        info!("DirectPlayEnumerateW calling callback for TCP/IP");
        callback(&DPSPGUID_TCPIP, name_tcp.as_ptr(), 1, 0, lpContext);
    }

    info!("DirectPlayEnumerateW EXIT DP_OK");
    DP_OK
}

/// Alias for DirectPlayEnumerateA
#[no_mangle]
pub unsafe extern "system" fn DirectPlayEnumerate(
    lpCallback: LPDPENUMDPCALLBACK,
    lpContext: LPVOID,
) -> HRESULT {
    info!("DirectPlayEnumerate ENTER (alias for DirectPlayEnumerateA)");
    let result = DirectPlayEnumerateA(lpCallback, lpContext);
    info!("DirectPlayEnumerate EXIT result={:#x}", result);
    result
}

/// DirectPlayLobbyCreateA - Create a lobby object (ANSI)
///
/// # Safety
/// All pointers must be valid
#[no_mangle]
pub unsafe extern "system" fn DirectPlayLobbyCreateA(
    lpGUID: *const GUID,
    lplpDPL: *mut LPVOID,
    pUnkOuter: *mut IUnknown,
    lpData: LPVOID,
    dwDataSize: DWORD,
) -> HRESULT {
    init_logging();
    info!("DirectPlayLobbyCreateA ENTER lpGUID={:?} lplpDPL={:?} pUnkOuter={:?} dwDataSize={}", lpGUID, lplpDPL, pUnkOuter, dwDataSize);

    // Validate parameters
    if lplpDPL.is_null() {
        info!("DirectPlayLobbyCreateA EXIT DPERR_INVALIDPARAMS (null lplpDPL)");
        return DPERR_INVALIDPARAMS;
    }

    // Don't support aggregation
    if !pUnkOuter.is_null() {
        info!("DirectPlayLobbyCreateA EXIT CLASS_E_NOAGGREGATION");
        return CLASS_E_NOAGGREGATION;
    }

    // lpData and dwDataSize must be 0 per documentation
    if !lpData.is_null() || dwDataSize != 0 {
        info!("DirectPlayLobbyCreateA EXIT DPERR_INVALIDPARAMS (lpData/dwDataSize not zero)");
        return DPERR_INVALIDPARAMS;
    }

    // Create lobby object
    let lobby_obj = lobby::DirectPlayLobbyObject::new();
    let ptr = lobby_obj.into_raw();
    *lplpDPL = ptr as LPVOID;

    info!("DirectPlayLobbyCreateA EXIT DP_OK ptr={:?}", ptr);
    DP_OK
}

/// DirectPlayLobbyCreateW - Create a lobby object (Unicode)
///
/// # Safety
/// All pointers must be valid
#[no_mangle]
pub unsafe extern "system" fn DirectPlayLobbyCreateW(
    lpGUID: *const GUID,
    lplpDPL: *mut LPVOID,
    pUnkOuter: *mut IUnknown,
    lpData: LPVOID,
    dwDataSize: DWORD,
) -> HRESULT {
    init_logging();
    info!("DirectPlayLobbyCreateW ENTER lpGUID={:?} lplpDPL={:?} pUnkOuter={:?} dwDataSize={}", lpGUID, lplpDPL, pUnkOuter, dwDataSize);

    // Unicode version shares same implementation since lobby doesn't store strings
    DirectPlayLobbyCreateA(lpGUID, lplpDPL, pUnkOuter, lpData, dwDataSize)
}

// ============================================================================
// COM Exports
// ============================================================================

/// DllGetClassObject - COM class factory entry point
///
/// # Safety
/// All GUIDs and pointers must be valid
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut LPVOID,
) -> HRESULT {
    init_logging();
    info!("DllGetClassObject ENTER rclsid={:?} riid={:?} ppv={:?}", rclsid, riid, ppv);

    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        info!("DllGetClassObject EXIT E_INVALIDARG (null param)");
        return E_INVALIDARG;
    }

    let clsid = &*rclsid;
    let iid = &*riid;
    info!("DllGetClassObject clsid={:?} iid={:?}", clsid, iid);

    // Check if requesting DirectPlay class
    if *clsid == CLSID_DIRECTPLAY {
        info!("DllGetClassObject: CLSID_DIRECTPLAY requested");
        if *iid == IID_ICLASSFACTORY || *iid == IID_IUNKNOWN {
            let factory = com::DirectPlayClassFactory::new();
            let ptr = Box::into_raw(Box::new(factory));
            *ppv = ptr as LPVOID;
            info!("DllGetClassObject EXIT S_OK factory={:?}", ptr);
            return S_OK;
        }
        info!("DllGetClassObject EXIT E_NOINTERFACE (wrong IID)");
        return E_NOINTERFACE;
    }

    // Check if requesting DirectPlayLobby class
    if *clsid == CLSID_DIRECTPLAYLOBBY {
        info!("DllGetClassObject EXIT CLASS_E_CLASSNOTAVAILABLE (lobby not implemented)");
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    info!("DllGetClassObject EXIT CLASS_E_CLASSNOTAVAILABLE (unknown CLSID)");
    CLASS_E_CLASSNOTAVAILABLE
}

/// DllCanUnloadNow - Check if DLL can be unloaded
#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    let obj_count = globals::get_object_count();
    let sp_ref_count = GDWDPLAYSPREFCOUNT.load(Ordering::SeqCst);
    info!("DllCanUnloadNow ENTER obj_count={} sp_ref_count={}", obj_count, sp_ref_count);

    let result = if obj_count == 0 && sp_ref_count == 0 {
        S_OK
    } else {
        S_FALSE
    };
    info!("DllCanUnloadNow EXIT result={:#x}", result);
    result
}

/// DllRegisterServer - Register COM classes
#[no_mangle]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    info!("DllRegisterServer ENTER");
    // Registration would write to Windows registry
    // For Wine, this is typically not needed as we use DLL override
    info!("DllRegisterServer EXIT S_OK");
    S_OK
}

/// DllUnregisterServer - Unregister COM classes
#[no_mangle]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    info!("DllUnregisterServer ENTER");
    info!("DllUnregisterServer EXIT S_OK");
    S_OK
}

/// Export the global reference count
#[no_mangle]
pub static mut gdwDPlaySPRefCount: DWORD = 0;
