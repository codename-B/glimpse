//! Provides the glimpse Windows Shell thumbnail handler for 3D model files.
//!
//! This DLL registers as a COM server that Windows Explorer calls to generate
//! thumbnail previews for 3D model files including glTF/GLB, Blockbench (.bbmodel),
//! and Vintage Story (.json) models. It uses a software rasterizer to render
//! small previews of 3D models.
//!
//! # Build
//! ```text
//! cargo build --release
//! ```
//!
//! # Install
//! See [INSTALLER.md](../INSTALLER.md) for registration and setup instructions.
//!
//! # Examples
//! ```no_run
//! use glimpse::renderer;
//!
//! let pixels = renderer::render_thumbnail(b"", None, 64, 64);
//! assert!(pixels.is_none());
//! ```

// COM macros intentionally wrap unsafe boilerplate so callers don't have to.
#![allow(clippy::macro_metavars_in_unsafe)]

// COM abstraction layer - must be declared first for macro availability
#[macro_use]
pub mod com;

pub mod formats;
pub mod provider;
pub mod renderer;

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;

// DllMain - required for DLL initialization
#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_hinst: HMODULE, _reason: u32, _reserved: *mut c_void) -> BOOL {
    BOOL::from(true)
}

/// The CLSID for the thumbnail handler COM object.
/// Replace with a unique GUID if redistributing.
///
/// # Examples
/// ```
/// let _ = glimpse::CLSID_GLTF_THUMBNAIL;
/// ```
// {A4C82A78-4C33-4420-83C4-F77C8C80514D}
pub const CLSID_GLTF_THUMBNAIL: GUID = GUID {
    data1: 0xA4C82A78,
    data2: 0x4C33,
    data3: 0x4420,
    data4: [0x83, 0xC4, 0xF7, 0x7C, 0x8C, 0x80, 0x51, 0x4D],
};

/// Tracks the global count of live COM objects and server locks.
///
/// # Examples
/// ```
/// use std::sync::atomic::Ordering;
///
/// let _ = glimpse::LOCK_COUNT.load(Ordering::Relaxed);
/// ```
pub static LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------
// COM class factory
// ---------------------------------------------------------------------------

#[windows::core::implement(IClassFactory)]
struct GltfClassFactory {}

impl IClassFactory_Impl for GltfClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: windows_core::Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        unsafe {
            if ppvobject.is_null() {
                return Err(E_POINTER.into());
            }
            *ppvobject = std::ptr::null_mut();

            // Aggregation not supported
            if !punkouter.is_null() {
                return Err(HRESULT(0x80040110u32 as i32).into()); // CLASS_E_NOAGGREGATION
            }

            let provider = provider::GltfThumbnailProvider::new();
            // Use custom QueryInterface that supports both IThumbnailProvider and IInitializeWithStream
            let hr = provider::query_interface_for_provider(&provider, riid, ppvobject);
            if hr.is_ok() {
                LOCK_COUNT.fetch_add(1, Ordering::Relaxed);
                Ok(())
            } else {
                Err(hr.into())
            }
        }
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            LOCK_COUNT.fetch_add(1, Ordering::Relaxed);
        } else {
            LOCK_COUNT.fetch_sub(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DLL exports
// ---------------------------------------------------------------------------

/// Called by COM runtime to obtain a class factory for the given CLSID.
#[no_mangle]
unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    if rclsid.is_null() || *rclsid != CLSID_GLTF_THUMBNAIL {
        return HRESULT(0x80040111u32 as i32); // CLASS_E_CLASSNOTAVAILABLE
    }

    let factory = GltfClassFactory {};
    let unknown: IUnknown = factory.into();
    unsafe { unknown.query(&*riid, ppv) }
}

/// Called by COM runtime to check if this DLL can be unloaded from memory.
#[no_mangle]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LOCK_COUNT.load(Ordering::Relaxed) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

/// Stub — use the PowerShell registration script instead.
#[no_mangle]
extern "system" fn DllRegisterServer() -> HRESULT {
    E_NOTIMPL
}

/// Stub — use the PowerShell registration script instead.
#[no_mangle]
extern "system" fn DllUnregisterServer() -> HRESULT {
    E_NOTIMPL
}
