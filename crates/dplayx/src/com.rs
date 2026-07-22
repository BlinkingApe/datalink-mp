//! COM interface implementations

use crate::directplay::DirectPlayObject;
use crate::globals;
use dp_types::*;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, info};

/// IClassFactory vtable for DirectPlay
static DIRECTPLAY_CLASS_FACTORY_VTBL: IClassFactoryVtbl = IClassFactoryVtbl {
    QueryInterface: DirectPlayClassFactory_QueryInterface,
    AddRef: DirectPlayClassFactory_AddRef,
    Release: DirectPlayClassFactory_Release,
    CreateInstance: DirectPlayClassFactory_CreateInstance,
    LockServer: DirectPlayClassFactory_LockServer,
};

/// DirectPlay class factory
#[repr(C)]
pub struct DirectPlayClassFactory {
    pub vtbl: *const IClassFactoryVtbl,
    ref_count: AtomicU32,
}

impl DirectPlayClassFactory {
    pub fn new() -> Self {
        globals::inc_object_count();
        Self {
            vtbl: &DIRECTPLAY_CLASS_FACTORY_VTBL,
            ref_count: AtomicU32::new(1),
        }
    }
}

impl Drop for DirectPlayClassFactory {
    fn drop(&mut self) {
        globals::dec_object_count();
    }
}

// IClassFactory implementation

unsafe extern "system" fn DirectPlayClassFactory_QueryInterface(
    this: *mut IClassFactory,
    riid: *const GUID,
    ppvObject: *mut LPVOID,
) -> HRESULT {
    info!("ClassFactory::QueryInterface ENTER this={:?} riid={:?}", this, riid);

    if this.is_null() || riid.is_null() || ppvObject.is_null() {
        info!("ClassFactory::QueryInterface EXIT E_INVALIDARG (null param)");
        return E_INVALIDARG;
    }

    let iid = &*riid;
    info!("ClassFactory::QueryInterface iid={:?}", iid);

    if *iid == IID_IUNKNOWN || *iid == IID_ICLASSFACTORY {
        DirectPlayClassFactory_AddRef(this);
        *ppvObject = this as LPVOID;
        info!("ClassFactory::QueryInterface EXIT S_OK");
        return S_OK;
    }

    *ppvObject = std::ptr::null_mut();
    info!("ClassFactory::QueryInterface EXIT E_NOINTERFACE");
    E_NOINTERFACE
}

unsafe extern "system" fn DirectPlayClassFactory_AddRef(this: *mut IClassFactory) -> DWORD {
    info!("ClassFactory::AddRef ENTER this={:?}", this);
    if this.is_null() {
        info!("ClassFactory::AddRef EXIT 0 (null this)");
        return 0;
    }

    let factory = &*(this as *const DirectPlayClassFactory);
    let count = factory.ref_count.fetch_add(1, Ordering::SeqCst) + 1;
    info!("ClassFactory::AddRef EXIT count={}", count);
    count
}

unsafe extern "system" fn DirectPlayClassFactory_Release(this: *mut IClassFactory) -> DWORD {
    info!("ClassFactory::Release ENTER this={:?}", this);
    if this.is_null() {
        info!("ClassFactory::Release EXIT 0 (null this)");
        return 0;
    }

    let factory = &*(this as *const DirectPlayClassFactory);
    let count = factory.ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    info!("ClassFactory::Release count={}", count);

    if count == 0 {
        info!("ClassFactory::Release dropping factory");
        let _ = Box::from_raw(this as *mut DirectPlayClassFactory);
    }

    info!("ClassFactory::Release EXIT count={}", count);
    count
}

unsafe extern "system" fn DirectPlayClassFactory_CreateInstance(
    this: *mut IClassFactory,
    pUnkOuter: *mut IUnknown,
    riid: *const GUID,
    ppvObject: *mut LPVOID,
) -> HRESULT {
    info!("ClassFactory::CreateInstance ENTER this={:?} pUnkOuter={:?} riid={:?}", this, pUnkOuter, riid);

    if ppvObject.is_null() {
        info!("ClassFactory::CreateInstance EXIT E_INVALIDARG (null ppvObject)");
        return E_INVALIDARG;
    }

    *ppvObject = std::ptr::null_mut();

    // Don't support aggregation
    if !pUnkOuter.is_null() {
        info!("ClassFactory::CreateInstance EXIT CLASS_E_NOAGGREGATION");
        return CLASS_E_NOAGGREGATION;
    }

    if riid.is_null() {
        info!("ClassFactory::CreateInstance EXIT E_INVALIDARG (null riid)");
        return E_INVALIDARG;
    }

    let iid = &*riid;
    info!("ClassFactory::CreateInstance iid={:?}", iid);

    // Check for supported interfaces
    if *iid == IID_IDIRECTPLAY4
        || *iid == IID_IDIRECTPLAY4A
        || *iid == IID_IDIRECTPLAY3
        || *iid == IID_IDIRECTPLAY3A
        || *iid == IID_IDIRECTPLAY2
        || *iid == IID_IDIRECTPLAY2A
        || *iid == IID_IUNKNOWN
    {
        info!("ClassFactory::CreateInstance creating DirectPlayObject");
        match DirectPlayObject::new() {
            Ok(obj) => {
                let ptr = Box::into_raw(Box::new(obj));
                *ppvObject = ptr as LPVOID;
                info!("ClassFactory::CreateInstance EXIT S_OK ptr={:?}", ptr);
                return S_OK;
            }
            Err(e) => {
                info!("ClassFactory::CreateInstance EXIT DPERR_GENERIC: {:?}", e);
                return DPERR_GENERIC;
            }
        }
    }

    info!("ClassFactory::CreateInstance EXIT E_NOINTERFACE");
    E_NOINTERFACE
}

unsafe extern "system" fn DirectPlayClassFactory_LockServer(
    this: *mut IClassFactory,
    fLock: BOOL,
) -> HRESULT {
    info!("ClassFactory::LockServer ENTER this={:?} fLock={}", this, fLock);

    if fLock != 0 {
        let count = globals::inc_object_count();
        info!("ClassFactory::LockServer incremented count to {}", count);
    } else {
        let count = globals::dec_object_count();
        info!("ClassFactory::LockServer decremented count to {}", count);
    }

    info!("ClassFactory::LockServer EXIT S_OK");
    S_OK
}
