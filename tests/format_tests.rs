//! Integration tests for multi-format model loading.
//!
//! Tests the format detection and loading for glTF, Blockbench, and Vintage Story formats.
//!
//! # Running tests that require model files
//!
//! Some tests are ignored by default because they require test model files. To run them,
//! provide your own `test.bbmodel`, `test.gltf`, and/or `test.json` files in the project
//! root, then run:
//!
//! ```text
//! cargo test -- --ignored
//! ```

use std::path::Path;

use glimpse::formats::{self, FormatLoader, LoadError};
use glimpse::renderer;

/// Helper to save RGBA pixels as PNG for visual inspection
fn save_test_png(pixels: &[u8], width: u32, height: u32, filename: &str) {
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, pixels.to_vec())
        .expect("Failed to create image buffer");
    img.save(filename).expect("Failed to save test PNG");
}

// ===========================================================================
// Format detection tests
// ===========================================================================

#[test]
fn test_gltf_loader_detection_glb_magic() {
    let loader = formats::gltf::GltfLoader;

    // GLB magic bytes: "glTF"
    let glb_data = b"glTF\x02\x00\x00\x00";
    assert!(loader.can_load(glb_data, None));
    assert!(loader.can_load(glb_data, Some("glb")));
}

#[test]
fn test_gltf_loader_detection_by_extension() {
    let loader = formats::gltf::GltfLoader;

    let random_data = b"not gltf data at all";
    assert!(loader.can_load(random_data, Some("gltf")));
    assert!(loader.can_load(random_data, Some("glb")));
    assert!(loader.can_load(random_data, Some("GLTF"))); // case insensitive
}

#[test]
fn test_bbmodel_loader_detection() {
    let loader = formats::bbmodel::BbmodelLoader;

    // Minimal Blockbench file structure
    let bbmodel_data = br#"{"meta":{"format_version":"4.0"},"elements":[]}"#;
    assert!(loader.can_load(bbmodel_data, Some("bbmodel")));
    assert!(loader.can_load(bbmodel_data, None)); // Should detect by content
}

#[test]
fn test_vintagestory_loader_detection() {
    let loader = formats::vintagestory::VintageStoryLoader;

    // Minimal Vintage Story model structure
    let vs_data = br#"{"elements":[{"from":[0,0,0],"to":[1,1,1],"faces":{"north":{}}}]}"#;
    assert!(loader.can_load(vs_data, Some("json")));
}

#[test]
fn test_vintagestory_loader_rejects_non_vs_json() {
    let loader = formats::vintagestory::VintageStoryLoader;

    // Regular JSON that's not a VS model
    let package_json = br#"{"name":"test","version":"1.0.0"}"#;
    assert!(!loader.can_load(package_json, Some("json")));

    // JSON without elements
    let config_json = br#"{"textures":{"base":"texture.png"}}"#;
    assert!(!loader.can_load(config_json, Some("json")));
}

// ===========================================================================
// Loader trait tests
// ===========================================================================

// ===========================================================================
// Blockbench parsing tests (synthetic data)
// ===========================================================================

#[test]
fn test_bbmodel_parse_simple_cube() {
    let bbmodel = br#"{
        "meta": {"format_version": "4.0"},
        "resolution": {"width": 16, "height": 16},
        "textures": [{
            "name": "pixel.png",
            "source": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP////8J+wP9o9FJCgAAAABJRU5ErkJggg==",
            "width": 1,
            "height": 1,
            "uv_width": 16,
            "uv_height": 16
        }],
        "elements": [{
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "faces": {
                "north": {"uv": [0, 0, 16, 16], "texture": 0},
                "south": {"uv": [0, 0, 16, 16], "texture": 0},
                "east": {"uv": [0, 0, 16, 16], "texture": 0},
                "west": {"uv": [0, 0, 16, 16], "texture": 0},
                "up": {"uv": [0, 0, 16, 16], "texture": 0},
                "down": {"uv": [0, 0, 16, 16], "texture": 0}
            }
        }]
    }"#;

    let loader = formats::bbmodel::BbmodelLoader;
    let result = loader.load_from_bytes(bbmodel);
    assert!(
        result.is_ok(),
        "Failed to parse bbmodel: {:?}",
        result.err()
    );

    let model = result.unwrap();
    // One cube = 6 faces = 12 triangles
    assert_eq!(model.triangles.len(), 12);
}

#[test]
fn test_bbmodel_empty_elements() {
    let bbmodel = br#"{
        "meta": {"format_version": "4.0"},
        "elements": []
    }"#;

    let loader = formats::bbmodel::BbmodelLoader;
    let result = loader.load_from_bytes(bbmodel);
    assert!(matches!(result, Err(LoadError::NoGeometry)));
}

// ===========================================================================
// Vintage Story parsing tests (synthetic data)
// ===========================================================================

#[test]
fn test_vintagestory_parse_simple_cube() {
    let vs_model = br#"{
        "elements": [{
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "faces": {
                "north": {},
                "south": {},
                "east": {},
                "west": {},
                "up": {},
                "down": {}
            }
        }]
    }"#;

    let loader = formats::vintagestory::VintageStoryLoader;
    let result = loader.load_from_bytes(vs_model);
    assert!(
        result.is_ok(),
        "Failed to parse VS model: {:?}",
        result.err()
    );

    let model = result.unwrap();
    // One cube = 6 faces = 12 triangles
    assert_eq!(model.triangles.len(), 12);
}

#[test]
fn test_vintagestory_with_comments() {
    // Vintage Story uses JSON5 which supports comments
    let vs_model = br#"{
        // This is a comment
        "elements": [{
            "from": [0, 0, 0],
            "to": [8, 8, 8],
            "faces": {
                "north": {},
                "south": {}
            }
        }]
    }"#;

    let loader = formats::vintagestory::VintageStoryLoader;
    let result = loader.load_from_bytes(vs_model);
    assert!(
        result.is_ok(),
        "JSON5 comments should be supported: {:?}",
        result.err()
    );
}

#[test]
fn test_vintagestory_with_rotation() {
    let vs_model = br#"{
        "elements": [{
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "rotationX": 45,
            "rotationY": 90,
            "rotationOrigin": [8, 8, 8],
            "faces": {
                "north": {},
                "up": {}
            }
        }]
    }"#;

    let loader = formats::vintagestory::VintageStoryLoader;
    let result = loader.load_from_bytes(vs_model);
    assert!(
        result.is_ok(),
        "Rotation should be parsed: {:?}",
        result.err()
    );
}

// ===========================================================================
// Auto-detection tests
// ===========================================================================

#[test]
fn test_find_loader_by_extension() {
    let glb_data = b"glTF\x02\x00\x00\x00";

    let loader = formats::find_loader(glb_data, Some("glb"));
    assert!(loader.is_some());
    assert_eq!(loader.unwrap().name(), "glTF");

    let loader = formats::find_loader(b"anything", Some("bbmodel"));
    assert!(loader.is_some());
    assert_eq!(loader.unwrap().name(), "Blockbench");
}

#[test]
fn test_find_loader_by_content() {
    // GLB magic should be detected without extension
    let glb_data = b"glTF\x02\x00\x00\x00extra data here";
    let loader = formats::find_loader(glb_data, None);
    assert!(loader.is_some());
    assert_eq!(loader.unwrap().name(), "glTF");
}

#[test]
fn test_load_model_unknown_format() {
    let random_data = b"this is not any known model format";
    let result = formats::load_model(random_data, None);
    assert!(matches!(result, Err(LoadError::UnrecognizedFormat)));
}

#[test]
#[ignore] // Requires test.bbmodel and test.json — provide your own model files to run
fn test_auto_detection_without_extension() {
    // Tests what happens when Windows calls IInitializeWithStream
    // which provides bytes but NO filename/extension

    let bbmodel_path = Path::new("test.bbmodel");
    if bbmodel_path.exists() {
        let data = std::fs::read(bbmodel_path).expect("Failed to read test.bbmodel");
        let loader = formats::bbmodel::BbmodelLoader;
        assert!(
            loader.can_load(&data, None),
            "bbmodel should be detectable without extension"
        );
        assert!(
            renderer::render_thumbnail(&data, None, 128, 128).is_some(),
            "bbmodel should render without extension hint"
        );
    }

    let json_path = Path::new("test.json");
    if json_path.exists() {
        let data = std::fs::read(json_path).expect("Failed to read test.json");
        let loader = formats::vintagestory::VintageStoryLoader;
        // VS json detection requires extension hint (json is too generic)
        let can_load_no_ext = loader.can_load(&data, None);
        println!("  VS json can_load(None): {}", can_load_no_ext);
    }
}

// ===========================================================================
// Renderer integration tests (synthetic data)
// ===========================================================================

#[test]
fn test_render_bbmodel_cube() {
    let bbmodel = br#"{
        "meta": {"format_version": "4.0"},
        "resolution": {"width": 16, "height": 16},
        "textures": [{
            "name": "gray.png",
            "source": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP////8J+wP9o9FJCgAAAABJRU5ErkJggg==",
            "width": 1,
            "height": 1,
            "uv_width": 16,
            "uv_height": 16
        }],
        "elements": [{
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "faces": {
                "north": {"uv": [0, 0, 16, 16], "texture": 0},
                "south": {"uv": [0, 0, 16, 16], "texture": 0},
                "east": {"uv": [0, 0, 16, 16], "texture": 0},
                "west": {"uv": [0, 0, 16, 16], "texture": 0},
                "up": {"uv": [0, 0, 16, 16], "texture": 0},
                "down": {"uv": [0, 0, 16, 16], "texture": 0}
            }
        }]
    }"#;

    let result = renderer::render_thumbnail(bbmodel, Some("bbmodel"), 128, 128);
    assert!(result.is_some(), "Rendering bbmodel should produce pixels");

    let pixels = result.unwrap();
    assert_eq!(pixels.len(), 128 * 128 * 4);

    let non_black_pixels = pixels
        .chunks(4)
        .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .count();
    assert!(
        non_black_pixels > 100,
        "Should have rendered visible content"
    );
}

#[test]
fn test_render_vintagestory_cube() {
    let vs_model = br#"{
        "elements": [{
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "faces": {
                "north": {},
                "south": {},
                "east": {},
                "west": {},
                "up": {},
                "down": {}
            }
        }]
    }"#;

    let result = renderer::render_thumbnail(vs_model, Some("json"), 128, 128);
    assert!(result.is_some(), "Rendering VS model should produce pixels");

    let pixels = result.unwrap();
    assert_eq!(pixels.len(), 128 * 128 * 4);

    let non_black_pixels = pixels
        .chunks(4)
        .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .count();
    assert!(
        non_black_pixels > 100,
        "Should have rendered visible content"
    );
}

// ===========================================================================
// Real file tests — ignored by default, provide your own models to run
// ===========================================================================

#[test]
#[ignore] // Requires test.bbmodel — provide your own model file to run
fn test_real_bbmodel() {
    let path = Path::new("test.bbmodel");
    if !path.exists() {
        println!("Skipping test - test.bbmodel not found");
        return;
    }

    let data = std::fs::read(path).expect("Failed to read test.bbmodel");
    let loader = formats::bbmodel::BbmodelLoader;

    let result = loader.load_from_bytes(&data);
    assert!(
        result.is_ok(),
        "Failed to parse test.bbmodel: {:?}",
        result.err()
    );

    let model = result.unwrap();
    assert!(!model.triangles.is_empty(), "Should have triangles");

    // Verify textures loaded
    let textured = model
        .triangles
        .iter()
        .filter(|t| t.texture.is_some())
        .count();
    assert!(textured > 0, "Should have textured triangles");

    // Render via path and bytes
    let pixels = renderer::render_thumbnail_from_path(path, 256, 256);
    assert!(pixels.is_some(), "render from path failed");

    let pixels_bytes = renderer::render_thumbnail(&data, Some("bbmodel"), 256, 256);
    assert!(pixels_bytes.is_some(), "render from bytes failed");

    let pixels = pixels.unwrap();
    let non_black = pixels
        .chunks(4)
        .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .count();
    assert!(non_black > 1000, "Should have significant rendered content");

    save_test_png(&pixels, 256, 256, "test_output_test_bb.png");
}

#[test]
#[ignore] // Requires test.json — provide your own Vintage Story model to run
fn test_real_vs_json() {
    let path = Path::new("test.json");
    if !path.exists() {
        println!("Skipping test - test.json not found");
        return;
    }

    let data = std::fs::read(path).expect("Failed to read test.json");
    let loader = formats::vintagestory::VintageStoryLoader;

    assert!(loader.can_load(&data, Some("json")));

    let result = loader.load_from_bytes(&data);
    assert!(
        result.is_ok(),
        "Failed to parse test.json: {:?}",
        result.err()
    );

    let model = result.unwrap();
    assert!(!model.triangles.is_empty(), "Should have triangles");

    let pixels = renderer::render_thumbnail(&data, Some("json"), 256, 256);
    assert!(pixels.is_some(), "render failed for test.json");

    let pixels = pixels.unwrap();
    let non_black = pixels
        .chunks(4)
        .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .count();
    assert!(non_black > 1000, "Should have significant rendered content");

    save_test_png(&pixels, 256, 256, "test_output_test_vs.png");
}

#[test]
#[ignore] // Requires test.gltf — provide your own glTF model to run
fn test_real_gltf() {
    let path = Path::new("test.gltf");
    if !path.exists() {
        println!("Skipping test - test.gltf not found");
        return;
    }

    let data = std::fs::read(path).expect("Failed to read test.gltf");
    let loader = formats::gltf::GltfLoader;

    assert!(loader.can_load(&data, Some("gltf")));

    let result = loader.load_from_bytes(&data);
    assert!(
        result.is_ok(),
        "Failed to parse test.gltf: {:?}",
        result.err()
    );

    let model = result.unwrap();
    assert!(!model.triangles.is_empty(), "Should have triangles");

    let pixels = renderer::render_thumbnail_from_path(path, 256, 256);
    assert!(pixels.is_some(), "render failed for test.gltf");

    let pixels = pixels.unwrap();
    let non_black = pixels
        .chunks(4)
        .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
        .count();
    assert!(non_black > 100, "Should have rendered content");

    save_test_png(&pixels, 256, 256, "test_output_test_gltf.png");
}
