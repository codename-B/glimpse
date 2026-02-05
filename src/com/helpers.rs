//! Provides COM helper types and traits for panic-free COM implementation.
//!
//! This module provides abstractions to reduce boilerplate in COM method
//! implementations while eliminating panic points that could crash Explorer.
//!
//! # Examples
//! ```
//! use std::sync::Mutex;
//!
//! use glimpse::com::helpers::MutexExt;
//!
//! let mutex = Mutex::new(1u32);
//! let guard = mutex.lock_or_fail().expect("lock should succeed");
//! assert_eq!(*guard, 1);
//! ```

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};

use windows::core::{Error, Result};
use windows::Win32::Foundation::E_FAIL;

/// Provides panic-free `Mutex` access in COM contexts.
///
/// In a DLL loaded by Explorer, panicking on a poisoned mutex would crash
/// the shell. This trait provides a method that returns an error
/// instead of panicking.
///
/// # Examples
/// ```
/// use std::sync::Mutex;
///
/// use glimpse::com::helpers::MutexExt;
///
/// let mutex = Mutex::new("value");
/// let guard = mutex.lock_or_fail().expect("lock should succeed");
/// assert_eq!(*guard, "value");
/// ```
pub trait MutexExt<T> {
    /// Locks the mutex, returning `E_FAIL` if poisoned instead of panicking.
    ///
    /// # Examples
    /// ```
    /// use std::sync::Mutex;
    ///
    /// use glimpse::com::helpers::MutexExt;
    ///
    /// let mutex = Mutex::new(42);
    /// let guard = mutex.lock_or_fail().expect("lock should succeed");
    /// assert_eq!(*guard, 42);
    /// ```
    fn lock_or_fail(&self) -> Result<MutexGuard<'_, T>>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_or_fail(&self) -> Result<MutexGuard<'_, T>> {
        self.lock().map_err(|_| Error::from(E_FAIL))
    }
}

/// Represents a COM object wrapper structure.
///
/// Per COM specification, the first field must be a pointer to the VTable.
/// This structure is what Windows receives as a COM object pointer.
///
/// Layout:
/// ```text
/// +0: vtbl pointer -> points to static VTable
/// +8: inner pointer -> points to Arc<T> data (via Arc::into_raw)
/// ```
///
/// # Examples
/// ```
/// use glimpse::com::helpers::ComWrapper;
///
/// let wrapper: ComWrapper<u8> = ComWrapper {
///     vtbl: std::ptr::null(),
///     inner: std::ptr::null(),
/// };
/// let _ = wrapper;
/// ```
#[repr(C)]
pub struct ComWrapper<T> {
    /// Pointer to the appropriate VTable for this interface.
    pub vtbl: *const c_void,
    /// Pointer to the inner data, created via `Arc::into_raw`.
    pub inner: *const T,
}

/// Creates a new COM wrapper with the given vtable and inner data.
///
/// # Safety
///
/// The caller must ensure:
/// - `vtbl` points to a valid, static VTable
/// - `inner` was created via `Arc::into_raw` and the Arc is kept alive
///
/// # Examples
/// ```ignore
/// use glimpse::com::helpers::create_wrapper;
///
/// let vtbl: *const std::ffi::c_void = std::ptr::null();
/// let inner: *const u8 = std::ptr::null();
/// let _wrapper = unsafe { create_wrapper(vtbl, inner) };
/// ```
pub unsafe fn create_wrapper<T>(vtbl: *const c_void, inner: *const T) -> *mut ComWrapper<T> {
    Box::into_raw(Box::new(ComWrapper { vtbl, inner }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_mutex_ext_success() {
        let mutex = Mutex::new(42);
        let guard = mutex.lock_or_fail();
        assert!(guard.is_ok());
        assert_eq!(*guard.unwrap(), 42);
    }

    #[test]
    fn test_com_wrapper_layout() {
        use std::mem::size_of;

        // ComWrapper should be exactly 2 pointers in size
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<ComWrapper<()>>(), 16);

        #[cfg(target_pointer_width = "32")]
        assert_eq!(size_of::<ComWrapper<()>>(), 8);
    }
}
