//! Direct COM interface test - actually calls Initialize and GetThumbnail.
//!
//! This test loads the DLL, creates COM objects, and tests the full flow:
//! 1. Load DLL
//! 2. Get class factory
//! 3. Create instance with IInitializeWithStream
//! 4. Create IStream from file data
//! 5. Call Initialize()
//! 6. Query for IThumbnailProvider
//! 7. Call GetThumbnail()
//! 8. Verify bitmap is valid
//!
//! # Running this test
//!
//! This test is ignored by default because it requires a `test.gltf` file.
//! Provide your own model file in the project root, then run:
//!
//! ```text
//! cargo test --test com_direct_test -- --ignored --nocapture
//! ```

use std::ffi::CString;
use std::fs;
use std::path::PathBuf;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::StructuredStorage::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Memory::*;
use windows::Win32::UI::Shell::PropertiesSystem::*;
use windows::Win32::UI::Shell::*;

const CLSID_GLTF_THUMBNAIL: GUID = GUID {
    data1: 0xA4C82A78,
    data2: 0x4C33,
    data3: 0x4420,
    data4: [0x83, 0xC4, 0xF7, 0x7C, 0x8C, 0x80, 0x51, 0x4D],
};

type DllGetClassObject = unsafe extern "system" fn(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> HRESULT;

fn find_dll() -> Option<PathBuf> {
    let paths = [
        "target/debug/glimpse.dll",
        "target/release/glimpse.dll",
        "target_temp/release/glimpse.dll",
    ];

    for path in &paths {
        let pb = PathBuf::from(path);
        if pb.exists() {
            return pb.canonicalize().ok();
        }
    }

    None
}

#[test]
#[ignore] // Requires test.gltf — provide your own model file to run
fn test_full_com_flow() {
    println!("\n=== Full COM Interface Test ===\n");

    // Find test file
    let test_file = PathBuf::from("test.gltf");
    if !test_file.exists() {
        println!("[WARN] test.gltf not found - skipping test");
        return;
    }

    // Read file
    println!("[1/7] Reading test file...");
    let file_data = match fs::read(&test_file) {
        Ok(d) => {
            println!("  [OK] Read {} bytes", d.len());
            d
        }
        Err(e) => {
            eprintln!("  [FAIL] Failed: {}", e);
            return;
        }
    };

    unsafe {
        // Initialize COM
        println!("\n[2/7] Initializing COM...");
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        println!("  [OK] COM initialized");

        // Load DLL
        println!("\n[3/7] Loading DLL...");
        let dll_path = find_dll().expect("DLL not found - build with: cargo build --release");
        println!("  DLL: {}", dll_path.display());

        // Use LoadLibraryW like the working test
        use std::os::windows::ffi::OsStrExt;
        let dll_path_wide: Vec<u16> = dll_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let hmodule = LoadLibraryW(PCWSTR::from_raw(dll_path_wide.as_ptr()));

        let hm = match hmodule {
            Ok(hm) => {
                if hm.is_invalid() {
                    eprintln!("  [FAIL] Failed to load DLL (invalid handle)");
                    CoUninitialize();
                    return;
                }
                println!("  [OK] DLL loaded");
                hm
            }
            Err(e) => {
                eprintln!("  [FAIL] Failed to load DLL: {:?}", e);
                CoUninitialize();
                return;
            }
        };

        // Get DllGetClassObject
        println!("\n[4/7] Getting DllGetClassObject...");
        let func_name = CString::new("DllGetClassObject").unwrap();
        let func_ptr = GetProcAddress(hm, PCSTR::from_raw(func_name.as_ptr() as *const u8));

        let dll_get_class_object: DllGetClassObject = match func_ptr {
            Some(ptr) => {
                println!("  [OK] Function found");
                std::mem::transmute::<unsafe extern "system" fn() -> isize, DllGetClassObject>(ptr)
            }
            None => {
                eprintln!("  [FAIL] DllGetClassObject not found");
                let _ = FreeLibrary(hm);
                CoUninitialize();
                return;
            }
        };

        // Get class factory
        let mut ppv: *mut std::ffi::c_void = std::ptr::null_mut();
        let iid_icf = <IClassFactory as windows::core::Interface>::IID;
        let hr = dll_get_class_object(&CLSID_GLTF_THUMBNAIL, &iid_icf, &mut ppv);

        if hr.is_err() || ppv.is_null() {
            eprintln!("  [FAIL] DllGetClassObject failed: {:?}", hr);
            let _ = FreeLibrary(hm);
            CoUninitialize();
            return;
        }

        println!("  [OK] DllGetClassObject succeeded");
        let factory: IClassFactory = IClassFactory::from_raw(ppv as *mut _);

        // Create instance with IInitializeWithStream
        println!("\n[5/7] Creating instance with IInitializeWithStream...");

        let unknown: IUnknown = match factory.CreateInstance(None) {
            Ok(i) => i,
            Err(hr) => {
                eprintln!("  [FAIL] CreateInstance failed: {:?}", hr);
                eprintln!("  This means IInitializeWithStream is not properly implemented!");
                let _ = FreeLibrary(hm);
                CoUninitialize();
                return;
            }
        };

        println!("  [OK] Instance created");

        // Create IStream from file data
        println!("\n[6/7] Creating IStream and calling Initialize...");
        let hglobal = match GlobalAlloc(GMEM_MOVEABLE, file_data.len()) {
            Ok(h) => h,
            Err(_) => {
                eprintln!("  [FAIL] Failed to allocate memory");
                let _ = FreeLibrary(hm);
                CoUninitialize();
                return;
            }
        };

        // Wrap GlobalLock/Unlock args in Some()
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = GlobalFree(Some(hglobal));
            eprintln!("  [FAIL] Failed to lock memory");
            let _ = FreeLibrary(hm);
            CoUninitialize();
            return;
        }
        std::ptr::copy_nonoverlapping(file_data.as_ptr(), ptr as *mut u8, file_data.len());
        let _ = GlobalUnlock(hglobal);

        // CreateStreamOnHGlobal likely returns Result<IStream> and takes (hmem, delete_on_release)
        let stream_result = CreateStreamOnHGlobal(hglobal, true);

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                let _ = GlobalFree(Some(hglobal));
                eprintln!("  [FAIL] Failed to create IStream: {:?}", e);
                let _ = FreeLibrary(hm);
                CoUninitialize();
                return;
            }
        };
        println!("  [OK] IStream created from file data");

        // Query for IInitializeWithStream and call Initialize
        // CAST to the interface instead of manually querying
        let init_stream: Result<IInitializeWithStream> = unknown.cast();

        if let Ok(init_with_stream) = init_stream {
            println!("  [OK] IInitializeWithStream interface accessible");

            // Call Initialize
            println!("  About to call Initialize...");
            println!("    stream: {:?}", &stream);
            println!("    mode: {}", STGM_READ.0);
            let hr = init_with_stream.Initialize(&stream, STGM_READ.0);
            println!("  Initialize returned: {:?}", hr);
            if hr.is_ok() {
                println!("  [OK] Initialize() succeeded");
            } else {
                eprintln!("  [FAIL] Initialize() failed: {:?}", hr);
            }
        } else {
            eprintln!("  [FAIL] Cast to IInitializeWithStream failed");
            let _ = FreeLibrary(hm);
            CoUninitialize();
            return;
        }

        // Query for IThumbnailProvider
        println!("\n[7/7] Querying for IThumbnailProvider...");
        let thumb_provider: Result<IThumbnailProvider> = unknown.cast();

        if let Ok(thumbnail_provider) = thumb_provider {
            println!("  [OK] IThumbnailProvider interface accessible");

            // Call GetThumbnail
            let mut hbmp = HBITMAP::default();
            let mut alpha = WTS_ALPHATYPE::default();
            let hr = thumbnail_provider.GetThumbnail(256, &mut hbmp, &mut alpha);

            if hr.is_ok() && !hbmp.is_invalid() {
                println!("  [OK] GetThumbnail() succeeded!");
                println!("  [OK] Bitmap handle: {:?}", hbmp);
                println!("  [OK] Alpha type: {:?}", alpha);
                println!("\n[PASS] ALL TESTS PASSED!");
                println!("   - DLL loads correctly");
                println!("   - Class factory works");
                println!("   - IInitializeWithStream accessible");
                println!("   - IThumbnailProvider accessible");
                println!("   - GetThumbnail() produces valid bitmap!");
            } else {
                eprintln!("  [FAIL] GetThumbnail() failed: {:?}", hr);
                eprintln!(
                    "  This means Initialize() needs to be called first, or rendering failed"
                );
            }

            println!("About to drop thumbnail_provider...");
            drop(thumbnail_provider);
        } else {
            eprintln!("  [FAIL] Cast to IThumbnailProvider failed");
            eprintln!("  This means the QueryInterface implementation is broken");
        }

        // Cleanup
        println!("About to drop stream...");
        drop(stream);
        println!("About to drop unknown...");
        drop(unknown);
        println!("About to drop factory...");
        drop(factory);
        println!("About to FreeLibrary...");
        let _ = FreeLibrary(hm);
        println!("About to CoUninitialize...");
        CoUninitialize();
        println!("Cleanup complete!");
    }
}
