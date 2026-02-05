//! Provides texture loading utilities for model formats.
//!
//! Provides functions for loading textures from various sources,
//! including base64-encoded data URLs.
//!
//! # Examples
//! ```
//! use glimpse::formats::shared::texture::load_texture_from_data_url;
//!
//! let data_url = "data:image/png;base64,\
//! iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP////8J+wP9o9FJCgAAAABJRU5ErkJggg==";
//! let texture = load_texture_from_data_url(data_url);
//! assert!(texture.is_some());
//! ```

use std::sync::Arc;

use crate::formats::TextureData;

/// Loads a texture from a base64-encoded data URL.
///
/// Supports data URLs in the format: `data:image/png;base64,<encoded_data>`
/// Returns None if the source is empty, not a data URL, or decoding fails.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::texture::load_texture_from_data_url;
///
/// let data_url = "data:image/png;base64,\
/// iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAEElEQVR4AQEFAPr/AP////8J+wP9o9FJCgAAAABJRU5ErkJggg==";
/// let texture = load_texture_from_data_url(data_url);
/// assert!(texture.is_some());
/// ```
pub fn load_texture_from_data_url(source: &str) -> Option<Arc<TextureData>> {
    if source.is_empty() || !source.starts_with("data:") {
        return None;
    }

    let comma_pos = source.find(',')?;
    let encoded = &source[(comma_pos + 1)..];

    // Decode base64
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;

    // Decode image
    use image::GenericImageView;
    let img = image::load_from_memory(&bytes).ok()?;
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();

    Some(Arc::new(TextureData {
        width,
        height,
        data: rgba.into_raw(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_source() {
        assert!(load_texture_from_data_url("").is_none());
    }

    #[test]
    fn test_non_data_url() {
        assert!(load_texture_from_data_url("https://example.com/image.png").is_none());
        assert!(load_texture_from_data_url("file:///path/to/image.png").is_none());
    }

    /// Generate a valid 1x1 white PNG as base64 for use in tests
    fn create_test_png_base64() -> String {
        use image::{ImageBuffer, Rgba};
        use std::io::Cursor;

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([255, 255, 255, 255]));

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png).unwrap();

        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(buffer.into_inner())
    }

    #[test]
    fn test_valid_1x1_png() {
        let base64_data = create_test_png_base64();
        let data_url = format!("data:image/png;base64,{}", base64_data);

        let texture = load_texture_from_data_url(&data_url);
        assert!(
            texture.is_some(),
            "load_texture_from_data_url returned None"
        );

        let tex = texture.unwrap();
        assert_eq!(tex.width, 1);
        assert_eq!(tex.height, 1);
        assert_eq!(tex.data.len(), 4);
        // Verify it's white
        assert_eq!(tex.data[0], 255); // R
        assert_eq!(tex.data[1], 255); // G
        assert_eq!(tex.data[2], 255); // B
        assert_eq!(tex.data[3], 255); // A
    }
}
