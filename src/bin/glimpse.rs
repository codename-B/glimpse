//! Provides the `glimpse-cli` tool for rendering 3D model thumbnails.
//!
//! Usage: `glimpse-cli <model_file> [size]`
//!
//! Renders a PNG thumbnail next to the input file.
//! Supports glTF/GLB, Blockbench (.bbmodel), and Vintage Story (.json).
//!
//! # Examples
//! ```text
//! glimpse-cli model.gltf 256
//! ```

use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <model_file> [size]", args[0]);
        eprintln!("  Renders a PNG thumbnail next to the input file.");
        eprintln!("  Default size: 256");
        process::exit(1);
    }

    let input = PathBuf::from(&args[1]);
    let size: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(256);

    if !input.exists() {
        eprintln!("Error: file not found: {}", input.display());
        process::exit(1);
    }

    let output = input.with_extension("png");

    eprintln!(
        "Rendering {} ({}x{})...",
        input.display(),
        size,
        size
    );

    let pixels = match glimpse::renderer::render_thumbnail_from_path(&input, size, size) {
        Some(p) => p,
        None => {
            eprintln!("Error: failed to render (unsupported format or no geometry)");
            process::exit(1);
        }
    };

    // Encode RGBA pixels to PNG
    use image::{ImageBuffer, Rgba};
    let img: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(size, size, pixels).expect("pixel buffer size mismatch");

    if let Err(e) = img.save(&output) {
        eprintln!("Error: failed to write {}: {}", output.display(), e);
        process::exit(1);
    }

    eprintln!("Saved {}", output.display());
}

