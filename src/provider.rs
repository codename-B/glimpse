//! Implements the COM object that provides `IThumbnailProvider` and `IInitializeWithStream`.
//!
//! Windows Explorer instantiates this object, initializes it with file data
//! via IInitializeWithStream, then calls GetThumbnail to obtain an HBITMAP.
//!
//! # Examples
//! ```
//! use glimpse::provider::GltfThumbnailProvider;
//!
//! let provider = GltfThumbnailProvider::new();
//! assert!(provider.set_data(vec![1, 2, 3]).is_ok());
//! ```


use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::{Error, Interface, Result, GUID, HRESULT};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::*;

use crate::com::helpers::{ComWrapper, MutexExt};
use crate::renderer;

// ---------------------------------------------------------------------------
// Interface GUIDs
// ---------------------------------------------------------------------------

const IID_IINITIALIZEWITHSTREAM: GUID = GUID::from_u128(0xb824b49d_22ac_4161_ac8a_9916e8fa3f7f);
const IID_ITHUMBNAILPROVIDER: GUID = GUID::from_u128(0xe357fccd_a995_4576_b01f_234630154e96);
const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
const IID_IINITIALIZEWITHFILE: GUID = GUID::from_u128(0xb7d14566_0509_4cce_a71f_0a554233bd9b);

// ---------------------------------------------------------------------------
// Internal data structure
// ---------------------------------------------------------------------------

/// Source of model data - either bytes from a stream or a file path.
enum GltfSource {
    Bytes(Vec<u8>),
    Path(std::path::PathBuf),
}

/// Internal data shared across COM interface wrappers via Arc.
struct GltfThumbnailProviderData {
    source: Mutex<Option<GltfSource>>,
    ref_count: AtomicU32,
    /// Stable IUnknown wrapper pointer for COM identity rule compliance.
    /// QueryInterface(IID_IUnknown) must always return the same pointer.
    iunknown_wrapper: AtomicPtr<c_void>,
}

// Type alias for the wrapper
type ProviderWrapper = ComWrapper<GltfThumbnailProviderData>;

// ---------------------------------------------------------------------------
// COM object
// ---------------------------------------------------------------------------

/// Represents the thumbnail provider COM object.
///
/// This struct holds an Arc to the shared data, allowing multiple interface
/// pointers to reference the same underlying object.
///
/// # Examples
/// ```
/// use glimpse::provider::GltfThumbnailProvider;
///
/// let _provider = GltfThumbnailProvider::new();
/// ```
pub struct GltfThumbnailProvider {
    inner: Arc<GltfThumbnailProviderData>,
}

impl GltfThumbnailProvider {
    /// Creates a new provider with ref count initialized to 1.
    ///
    /// # Examples
    /// ```
    /// use glimpse::provider::GltfThumbnailProvider;
    ///
    /// let _provider = GltfThumbnailProvider::new();
    /// ```
    pub fn new() -> Self {
        Self {
            inner: Arc::new(GltfThumbnailProviderData {
                source: Mutex::new(None),
                ref_count: AtomicU32::new(1),
                iunknown_wrapper: AtomicPtr::new(std::ptr::null_mut()),
            }),
        }
    }

    /// Sets the model data directly (for testing).
    ///
    /// # Errors
    /// Returns an error if the mutex is poisoned.
    ///
    /// # Examples
    /// ```
    /// use glimpse::provider::GltfThumbnailProvider;
    ///
    /// let provider = GltfThumbnailProvider::new();
    /// assert!(provider.set_data(vec![1, 2, 3]).is_ok());
    /// ```
    pub fn set_data(&self, data: Vec<u8>) -> Result<()> {
        let mut guard = self.inner.source.lock_or_fail()?;
        *guard = Some(GltfSource::Bytes(data));
        Ok(())
    }

    /// Increments the reference count and returns the new count.
    ///
    /// # Examples
    /// ```
    /// use glimpse::provider::GltfThumbnailProvider;
    ///
    /// let provider = GltfThumbnailProvider::new();
    /// let count = provider.add_ref();
    /// assert!(count >= 2);
    /// ```
    pub fn add_ref(&self) -> u32 {
        self.inner.ref_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Decrements the reference count and returns the new count.
    ///
    /// # Examples
    /// ```
    /// use glimpse::provider::GltfThumbnailProvider;
    ///
    /// let provider = GltfThumbnailProvider::new();
    /// let _ = provider.add_ref();
    /// let count = provider.release();
    /// assert!(count >= 1);
    /// ```
    pub fn release(&self) -> u32 {
        self.inner.ref_count.fetch_sub(1, Ordering::Release) - 1
    }
}

impl Default for GltfThumbnailProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IUnknown VTable structure (base for all interfaces)
// ---------------------------------------------------------------------------

#[repr(C)]
#[allow(non_snake_case)]
struct IUnknown_Vtbl {
    QueryInterface:
        unsafe extern "system" fn(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

// ---------------------------------------------------------------------------
// IUnknown implementations
// ---------------------------------------------------------------------------

unsafe extern "system" fn query_interface_impl(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    if riid.is_null() {
        return E_INVALIDARG;
    }

    let riid = *riid;

    com_method!(this, inner: GltfThumbnailProviderData => {
        // Increment ref count for the new interface pointer
        inner.ref_count.fetch_add(1, Ordering::Relaxed);

        // All wrappers share the same raw pointer to the data (no Arc cloning).
        // The single Arc ref created in query_interface_for_provider owns the data.
        let data_ptr = inner as *const GltfThumbnailProviderData;

        if riid == IID_IINITIALIZEWITHSTREAM {
            let wrapper = Box::into_raw(Box::new(ProviderWrapper {
                vtbl: &IINITIALIZEWITHSTREAM_VTABLE as *const _ as *const c_void,
                inner: data_ptr,
            }));
            *ppv = wrapper as *mut c_void;
            S_OK
        } else if riid == IID_IINITIALIZEWITHFILE {
            let wrapper = Box::into_raw(Box::new(ProviderWrapper {
                vtbl: &IINITIALIZEWITHFILE_VTABLE as *const _ as *const c_void,
                inner: data_ptr,
            }));
            *ppv = wrapper as *mut c_void;
            S_OK
        } else if riid == IID_ITHUMBNAILPROVIDER {
            let wrapper = Box::into_raw(Box::new(ProviderWrapper {
                vtbl: &ITHUMBNAILPROVIDER_VTABLE as *const _ as *const c_void,
                inner: data_ptr,
            }));
            *ppv = wrapper as *mut c_void;
            S_OK
        } else if riid == IID_IUNKNOWN {
            // COM identity rule: IUnknown must always return the same pointer.
            *ppv = get_or_create_iunknown_wrapper(inner, data_ptr);
            S_OK
        } else {
            // Undo the ref count increment — no matching interface found
            inner.ref_count.fetch_sub(1, Ordering::Relaxed);
            E_NOINTERFACE
        }
    })
}

unsafe extern "system" fn add_ref_impl(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }

    let wrapper = this as *mut ProviderWrapper;
    let inner_ptr = (*wrapper).inner;

    if inner_ptr.is_null() {
        return 0;
    }

    (*inner_ptr).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release_impl(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }

    let wrapper = this as *mut ProviderWrapper;
    let inner_ptr = (*wrapper).inner;

    if inner_ptr.is_null() {
        return 0;
    }

    let count = (*inner_ptr).ref_count.fetch_sub(1, Ordering::Release) - 1;

    if count == 0 {
        // Last reference: free all wrappers and reclaim the Arc to free the data
        std::sync::atomic::fence(Ordering::Acquire);

        // Free the stable IUnknown wrapper if it exists and isn't the
        // wrapper currently being Released (avoid double-free).
        let iunknown_ptr = (*inner_ptr).iunknown_wrapper.load(Ordering::Relaxed);
        if !iunknown_ptr.is_null() && iunknown_ptr != this {
            drop(Box::from_raw(iunknown_ptr as *mut ProviderWrapper));
        }

        drop(Box::from_raw(wrapper));
        drop(Arc::from_raw(inner_ptr));
        // Tell COM runtime this object is gone
        crate::LOCK_COUNT.fetch_sub(1, Ordering::Relaxed);
    }

    count
}

// ---------------------------------------------------------------------------
// IThumbnailProvider::GetThumbnail
// ---------------------------------------------------------------------------

unsafe extern "system" fn get_thumbnail_impl(
    this: *mut c_void,
    cx: u32,
    phbmp: *mut HBITMAP,
    pdwalpha: *mut WTS_ALPHATYPE,
) -> HRESULT {
    if phbmp.is_null() || pdwalpha.is_null() {
        return E_POINTER;
    }

    com_method_result!(this, inner: GltfThumbnailProviderData => {
        let guard = inner.source.lock_or_fail()?;
        let source = guard.as_ref().ok_or(Error::from(E_UNEXPECTED))?;

        let size = cx.clamp(32, 1024);

        let pixels = match source {
            GltfSource::Bytes(data) => renderer::render_thumbnail(data, None, size, size),
            GltfSource::Path(path) => renderer::render_thumbnail_from_path(path, size, size),
        }
        .ok_or(Error::from(E_FAIL))?;

        let hbmp = create_hbitmap_from_rgba(&pixels, size, size)?;

        *phbmp = hbmp;
        *pdwalpha = WTSAT_ARGB;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// IInitializeWithStream::Initialize
// ---------------------------------------------------------------------------

unsafe extern "system" fn initialize_stream_impl(
    this: *mut c_void,
    pstream: *mut c_void,
    grfmode: u32,
) -> HRESULT {
    if pstream.is_null() {
        return E_INVALIDARG;
    }

    com_method_result!(this, inner: GltfThumbnailProviderData => {
        // ManuallyDrop prevents Release on the caller-owned stream,
        // even if read_stream_to_vec fails and returns early via `?`.
        let stream = std::mem::ManuallyDrop::new(IStream::from_raw(pstream));

        let bytes = read_stream_to_vec(&stream)?;

        let mut guard = inner.source.lock_or_fail()?;
        *guard = Some(GltfSource::Bytes(bytes));

        let _ = grfmode; // Unused but part of interface
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// IInitializeWithFile::Initialize
// ---------------------------------------------------------------------------

unsafe extern "system" fn initialize_file_impl(
    this: *mut c_void,
    pszfilepath: *const u16,
    grfmode: u32,
) -> HRESULT {
    if pszfilepath.is_null() {
        return E_INVALIDARG;
    }

    com_method_result!(this, inner: GltfThumbnailProviderData => {
        // Convert wide string to PathBuf
        let mut len = 0;
        while *pszfilepath.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(pszfilepath, len);
        let path = std::path::PathBuf::from(String::from_utf16_lossy(slice));

        let mut guard = inner.source.lock_or_fail()?;
        *guard = Some(GltfSource::Path(path));

        let _ = grfmode; // Unused but part of interface
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Stable IUnknown pointer (COM identity rule)
// ---------------------------------------------------------------------------

/// Gets or creates the stable IUnknown wrapper pointer.
///
/// COM requires that `QueryInterface(IID_IUnknown)` always returns the same
/// pointer value for a given object, so clients can compare identity.
/// This function lazily allocates a wrapper on first call and returns
/// the same pointer on subsequent calls using atomic compare-exchange.
///
/// # Safety
///
/// `inner` must point to valid, live `GltfThumbnailProviderData`.
/// `data_ptr` must be the raw pointer to the same data (for the wrapper's `inner` field).
unsafe fn get_or_create_iunknown_wrapper(
    inner: &GltfThumbnailProviderData,
    data_ptr: *const GltfThumbnailProviderData,
) -> *mut c_void {
    let existing = inner.iunknown_wrapper.load(Ordering::Acquire);
    if !existing.is_null() {
        return existing;
    }

    // Allocate a new wrapper (uses IThumbnailProvider vtable since it inherits IUnknown)
    let wrapper = Box::into_raw(Box::new(ProviderWrapper {
        vtbl: &ITHUMBNAILPROVIDER_VTABLE as *const _ as *const c_void,
        inner: data_ptr,
    }));
    let wrapper_ptr = wrapper as *mut c_void;

    // Try to store atomically; if another thread won the race, use theirs
    match inner.iunknown_wrapper.compare_exchange(
        std::ptr::null_mut(),
        wrapper_ptr,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => wrapper_ptr,
        Err(existing) => {
            // Race lost: free the duplicate wrapper, return the winner's pointer
            drop(Box::from_raw(wrapper));
            existing
        }
    }
}

// ---------------------------------------------------------------------------
// VTable definitions
// ---------------------------------------------------------------------------

define_vtable! {
    name: IThumbnailProvider_Vtbl,
    static_name: ITHUMBNAILPROVIDER_VTABLE,
    iunknown: (query_interface_impl, add_ref_impl, release_impl),
    methods: {
        GetThumbnail: unsafe extern "system" fn(
            *mut c_void, u32, *mut HBITMAP, *mut WTS_ALPHATYPE
        ) -> HRESULT = get_thumbnail_impl,
    }
}

define_vtable! {
    name: IInitializeWithStream_Vtbl,
    static_name: IINITIALIZEWITHSTREAM_VTABLE,
    iunknown: (query_interface_impl, add_ref_impl, release_impl),
    methods: {
        Initialize: unsafe extern "system" fn(
            *mut c_void, *mut c_void, u32
        ) -> HRESULT = initialize_stream_impl,
    }
}

define_vtable! {
    name: IInitializeWithFile_Vtbl,
    static_name: IINITIALIZEWITHFILE_VTABLE,
    iunknown: (query_interface_impl, add_ref_impl, release_impl),
    methods: {
        Initialize: unsafe extern "system" fn(
            *mut c_void, *const u16, u32
        ) -> HRESULT = initialize_file_impl,
    }
}

// ---------------------------------------------------------------------------
// Public QueryInterface for use in lib.rs
// ---------------------------------------------------------------------------

/// Queries for a specific interface from a provider instance.
///
/// Used by the class factory to return the requested interface.
///
/// # Safety
///
/// `riid` must point to a valid GUID. `ppv` must point to a valid, writable pointer location.
///
/// # Examples
/// ```ignore
/// use std::ptr;
///
/// use windows::Win32::UI::Shell::IThumbnailProvider;
///
/// use glimpse::provider::{query_interface_for_provider, GltfThumbnailProvider};
///
/// let provider = GltfThumbnailProvider::new();
/// let mut out = ptr::null_mut();
/// let _ = unsafe { query_interface_for_provider(&provider, &IThumbnailProvider::IID, &mut out) };
/// ```
pub unsafe fn query_interface_for_provider(
    provider: &GltfThumbnailProvider,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    if riid.is_null() {
        return E_INVALIDARG;
    }

    let riid = *riid;

    // ref_count is already 1 from new(), matching the single returned interface pointer.
    // Arc::into_raw creates a single raw pointer that all wrappers share.
    // This Arc ref is reclaimed in release_impl when ref_count hits 0.
    let data_ptr = Arc::into_raw(Arc::clone(&provider.inner));

    if riid == IID_IINITIALIZEWITHSTREAM {
        let wrapper = Box::into_raw(Box::new(ProviderWrapper {
            vtbl: &IINITIALIZEWITHSTREAM_VTABLE as *const _ as *const c_void,
            inner: data_ptr,
        }));
        *ppv = wrapper as *mut c_void;
        S_OK
    } else if riid == IID_IINITIALIZEWITHFILE {
        let wrapper = Box::into_raw(Box::new(ProviderWrapper {
            vtbl: &IINITIALIZEWITHFILE_VTABLE as *const _ as *const c_void,
            inner: data_ptr,
        }));
        *ppv = wrapper as *mut c_void;
        S_OK
    } else if riid == IID_ITHUMBNAILPROVIDER {
        let wrapper = Box::into_raw(Box::new(ProviderWrapper {
            vtbl: &ITHUMBNAILPROVIDER_VTABLE as *const _ as *const c_void,
            inner: data_ptr,
        }));
        *ppv = wrapper as *mut c_void;
        S_OK
    } else if riid == IID_IUNKNOWN {
        // COM identity rule: IUnknown must always return the same pointer.
        *ppv = get_or_create_iunknown_wrapper(&*data_ptr, data_ptr);
        S_OK
    } else {
        // Unsupported interface: reclaim the Arc ref
        drop(Arc::from_raw(data_ptr));
        E_NOINTERFACE
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reads the full contents of an `IStream` into a `Vec<u8>`.
///
/// Propagates any read error immediately via `?` rather than silently
/// accepting partial data. Per the ISequentialStream::Read contract:
/// - S_OK: bytes_read == bytes requested (success)
/// - S_FALSE: end-of-stream, fewer bytes read (possibly zero)
/// - Any failure HRESULT: read failed, must not use partial data
fn read_stream_to_vec(stream: &IStream) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(256 * 1024); // pre-alloc 256 KB
    let mut buf = [0u8; 65536];

    loop {
        let mut bytes_read: u32 = 0;
        // Propagate any error HRESULT immediately — never accept partial data.
        unsafe {
            stream
                .Read(
                    buf.as_mut_ptr().cast(),
                    buf.len() as u32,
                    Some(&mut bytes_read),
                )
                .ok()?
        };

        if bytes_read == 0 {
            break; // EOF (S_FALSE with 0 bytes, or S_OK with 0 requested)
        }

        data.extend_from_slice(&buf[..bytes_read as usize]);
    }

    if data.is_empty() {
        return Err(E_FAIL.into());
    }
    Ok(data)
}

/// Creates a top-down 32-bit HBITMAP from an RGBA pixel buffer.
///
/// Windows expects BGRA byte order in DIB sections, so R and B are swizzled.
///
/// # Safety
///
/// The caller must ensure that `pixels` contains at least `width * height * 4` bytes
/// of valid RGBA data.
unsafe fn create_hbitmap_from_rgba(pixels: &[u8], width: u32, height: u32) -> Result<HBITMAP> {
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // negative = top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [RGBQUAD::default()],
    };

    let mut bits: *mut c_void = std::ptr::null_mut();
    let hdc = GetDC(None);

    let hbmp = CreateDIBSection(Some(hdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;

    ReleaseDC(None, hdc);

    if bits.is_null() {
        return Err(E_FAIL.into());
    }

    // Copy and swizzle RGBA → BGRA
    let pixel_count = (width * height) as usize;
    let expected_bytes = pixel_count * 4;
    if pixels.len() < expected_bytes {
        return Err(E_INVALIDARG.into());
    }
    let dst = std::slice::from_raw_parts_mut(bits as *mut u8, expected_bytes);

    for i in 0..pixel_count {
        let s = i * 4;
        dst[s] = pixels[s + 2]; // B ← R
        dst[s + 1] = pixels[s + 1]; // G
        dst[s + 2] = pixels[s]; // R ← B
        dst[s + 3] = pixels[s + 3]; // A
    }

    Ok(hbmp)
}
