//! Provides macros for reducing COM boilerplate.
//!
//! These macros handle the repetitive patterns in COM method implementations:
//! - Extracting the inner data from wrapper pointers
//! - Generating VTable structures
//!
//! # Examples
//! ```ignore
//! use windows::Win32::Foundation::S_OK;
//!
//! unsafe extern "system" fn example(this: *mut std::ffi::c_void) -> windows::core::HRESULT {
//!     glimpse::com_method!(this, inner: () => {
//!         let _ = inner;
//!         S_OK
//!     })
//! }
//! ```

/// Executes a COM method body with automatic wrapper extraction.
///
/// This macro handles the common pattern of:
/// 1. Validating the `this` pointer
/// 2. Extracting `&T` from the COM wrapper's raw pointer
/// 3. Executing the method body
///
/// Returns the HRESULT from the body, or `E_POINTER`/`E_FAIL` on invalid input.
///
/// # Examples
///
/// ```ignore
/// unsafe extern "system" fn my_method(this: *mut c_void, arg: u32) -> HRESULT {
///     com_method!(this, inner: MyDataType => {
///         // `inner` is &MyDataType
///         // Return an HRESULT
///         S_OK
///     })
/// }
/// ```
///
/// # Panics
///
/// Panics in the body are caught and converted to `E_FAIL`, so they do not
/// unwind across FFI boundaries.
#[macro_export]
macro_rules! com_method {
    ($this:expr, $inner:ident : $T:ty => $body:expr) => {{
        use windows::core::HRESULT;
        use windows::Win32::Foundation::{E_FAIL, E_POINTER};
        use $crate::com::helpers::ComWrapper;

        unsafe {
            if $this.is_null() {
                return E_POINTER;
            }

            let wrapper = $this as *mut ComWrapper<$T>;
            let inner_ptr = (*wrapper).inner;

            if inner_ptr.is_null() {
                return E_FAIL;
            }

            let $inner = &*inner_ptr;

            // catch_unwind prevents panics from unwinding across FFI.
            // AssertUnwindSafe is appropriate here: in unsafe COM
            // code where aborting on panic is the only alternative.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> HRESULT { $body })) {
                Ok(hr) => hr,
                Err(_) => E_FAIL,
            }
        }
    }};
}

/// Executes a COM method that returns `windows::core::Result<()>`, converting to HRESULT.
///
/// Similar to `com_method!` but for methods that use Result for error handling.
/// Converts Ok(_) to S_OK and Err(e) to e.code() HRESULT.
///
/// # Examples
///
/// ```ignore
/// unsafe extern "system" fn my_method(this: *mut c_void) -> HRESULT {
///     com_method_result!(this, inner: MyDataType => {
///         let guard = inner.data.lock_or_fail()?;
///         do_something(&guard)?;
///         Ok(())
///     })
/// }
/// ```
#[macro_export]
macro_rules! com_method_result {
    ($this:expr, $inner:ident : $T:ty => $body:expr) => {{
        use windows::Win32::Foundation::{E_FAIL, E_POINTER, S_OK};
        use $crate::com::helpers::ComWrapper;

        unsafe {
            if $this.is_null() {
                return E_POINTER;
            }

            let wrapper = $this as *mut ComWrapper<$T>;
            let inner_ptr = (*wrapper).inner;

            if inner_ptr.is_null() {
                return E_FAIL;
            }

            let $inner = &*inner_ptr;

            // catch_unwind prevents panics from unwinding across FFI.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let result: windows::core::Result<()> = (|| $body)();
                match result {
                    Ok(()) => S_OK,
                    Err(e) => e.code(),
                }
            })) {
                Ok(hr) => hr,
                Err(_) => E_FAIL,
            }
        }
    }};
}

/// Defines a COM VTable structure with IUnknown base methods.
///
/// This macro generates a repr(C) VTable struct and a static instance
/// populated with the provided function pointers.
///
/// # Examples
///
/// ```ignore
/// define_vtable! {
///     name: IThumbnailProvider_Vtbl,
///     static_name: ITHUMBNAILPROVIDER_VTABLE,
///     iunknown: (query_interface_impl, add_ref_impl, release_impl),
///     methods: {
///         GetThumbnail: unsafe extern "system" fn(
///             *mut c_void, u32, *mut HBITMAP, *mut WTS_ALPHATYPE
///         ) -> HRESULT = get_thumbnail_impl,
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_vtable {
    (
        name: $vtbl_name:ident,
        static_name: $static_name:ident,
        iunknown: ($qi:expr, $addref:expr, $release:expr),
        methods: {
            $($method_name:ident : $method_sig:ty = $method_impl:expr),* $(,)?
        }
    ) => {
        #[repr(C)]
        #[allow(non_snake_case)]
        struct $vtbl_name {
            base: IUnknown_Vtbl,
            $($method_name: $method_sig,)*
        }

        static $static_name: $vtbl_name = $vtbl_name {
            base: IUnknown_Vtbl {
                QueryInterface: $qi,
                AddRef: $addref,
                Release: $release,
            },
            $($method_name: $method_impl,)*
        };
    };
}

// Re-export macros at crate root
pub use crate::com_method;
pub use crate::com_method_result;
pub use crate::define_vtable;
