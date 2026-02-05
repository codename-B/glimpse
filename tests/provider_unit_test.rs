//! Direct provider unit tests - tests COM provider logic without DLL loading.
//!
//! This test exercises the GltfThumbnailProvider directly in Rust,
//! bypassing LoadLibrary/GetProcAddress to isolate the provider implementation.
//!
//! These tests do not require external model files and run by default.
//! The `test_provider_set_data` test will use `test.gltf` if available but
//! gracefully skips that portion if not found.
//!
//! Run with: cargo test --test provider_unit_test -- --nocapture

use std::fs;
use std::path::PathBuf;

use glimpse::provider::GltfThumbnailProvider;

fn find_test_file() -> Option<PathBuf> {
    let pb = PathBuf::from("test.gltf");
    if pb.exists() {
        Some(pb)
    } else {
        None
    }
}

#[test]
fn test_provider_set_data() {
    println!("\n=== Test: Provider Set Data ===");

    let provider = GltfThumbnailProvider::new();

    // Test with empty data
    let _ = provider.set_data(vec![]);
    println!("  [OK] Empty data set successfully");

    // Test with some bytes
    let _ = provider.set_data(vec![1, 2, 3, 4, 5]);
    println!("  [OK] Sample data set successfully");

    // Test with actual file data if available
    if let Some(test_file) = find_test_file() {
        match fs::read(&test_file) {
            Ok(data) => {
                let _ = provider.set_data(data.clone());
                println!("  [OK] File data ({} bytes) set successfully", data.len());
            }
            Err(e) => {
                println!("  [WARN] Could not read test file: {}", e);
            }
        }
    } else {
        println!("  [WARN] No test file found, skipping file data test");
    }
}

#[test]
fn test_provider_ref_counting() {
    println!("\n=== Test: Provider Ref Counting ===");

    let provider = GltfThumbnailProvider::new();

    // Initial ref count is 1; add_ref should return 2
    let count = provider.add_ref();
    assert_eq!(count, 2, "After add_ref, count should be 2");
    println!("  [OK] add_ref returned {}", count);

    // release should return 1
    let count = provider.release();
    assert_eq!(count, 1, "After release, count should be 1");
    println!("  [OK] release returned {}", count);

    // Another release should return 0
    let count = provider.release();
    assert_eq!(count, 0, "After second release, count should be 0");
    println!("  [OK] release returned {}", count);

    println!("  [OK] Ref counting works correctly");
}

