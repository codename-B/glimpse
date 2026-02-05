//! Provides the format loader trait and common types for multi-format model support.
//!
//! This module defines a common interface for loading 3D models from various
//! formats (glTF, Blockbench, Vintage Story, etc.) into a unified representation
//! that can be rendered by the software rasterizer.
//!
//! # Examples
//! ```
//! use glimpse::formats;
//!
//! let result = formats::load_model(b"invalid", None);
//! assert!(result.is_err());
//! ```

pub mod bbmodel;
pub mod gltf;
pub mod shared;
pub mod vintagestory;

use std::path::Path;
use std::sync::Arc;

// ---- Math type aliases ----
/// A 2D vector type used by format loaders.
///
/// # Examples
/// ```
/// use glimpse::formats::Vec2;
///
/// let v: Vec2 = [0.0, 1.0];
/// assert_eq!(v, [0.0, 1.0]);
/// ```
pub type Vec2 = [f32; 2];
/// A 3D vector type used by format loaders.
///
/// # Examples
/// ```
/// use glimpse::formats::Vec3;
///
/// let v: Vec3 = [1.0, 2.0, 3.0];
/// assert_eq!(v, [1.0, 2.0, 3.0]);
/// ```
pub type Vec3 = [f32; 3];
/// A 4x4 matrix type used by format loaders.
///
/// # Examples
/// ```
/// use glimpse::formats::Mat4;
///
/// let m: Mat4 = [
///     [1.0, 0.0, 0.0, 0.0],
///     [0.0, 1.0, 0.0, 0.0],
///     [0.0, 0.0, 1.0, 0.0],
///     [0.0, 0.0, 0.0, 1.0],
/// ];
/// assert_eq!(m[0][0], 1.0);
/// ```
pub type Mat4 = [[f32; 4]; 4];

/// Represents loaded texture data for sampling.
///
/// # Examples
/// ```
/// use glimpse::formats::TextureData;
///
/// let tex = TextureData {
///     width: 1,
///     height: 1,
///     data: vec![255, 255, 255, 255],
/// };
/// assert_eq!(tex.width, 1);
/// ```
#[derive(Clone)]
pub struct TextureData {
    /// The texture width in pixels.
    pub width: u32,
    /// The texture height in pixels.
    pub height: u32,
    /// RGBA pixel data stored row-major.
    pub data: Vec<u8>, // RGBA pixels
}

impl TextureData {
    /// Samples the texture at UV coordinates (with wrapping).
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::TextureData;
    ///
    /// let tex = TextureData {
    ///     width: 1,
    ///     height: 1,
    ///     data: vec![255, 255, 255, 255],
    /// };
    /// let sample = tex.sample(0.5, 0.5);
    /// assert_eq!(sample, [1.0, 1.0, 1.0, 1.0]);
    /// ```
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        // Wrap UVs to [0, 1)
        let u = u.fract();
        let v = v.fract();
        let u = if u < 0.0 { u + 1.0 } else { u };
        let v = if v < 0.0 { v + 1.0 } else { v };

        let x = ((u * self.width as f32) as u32).min(self.width.saturating_sub(1));
        let y = ((v * self.height as f32) as u32).min(self.height.saturating_sub(1));
        let idx = ((y * self.width + x) * 4) as usize;

        if idx + 3 < self.data.len() {
            [
                self.data[idx] as f32 / 255.0,
                self.data[idx + 1] as f32 / 255.0,
                self.data[idx + 2] as f32 / 255.0,
                self.data[idx + 3] as f32 / 255.0,
            ]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        }
    }
}

/// Represents a triangle with position, UV, color, and optional texture.
///
/// # Examples
/// ```
/// use std::sync::Arc;
///
/// use glimpse::formats::{TextureData, Triangle};
///
/// let tex = Arc::new(TextureData {
///     width: 1,
///     height: 1,
///     data: vec![255, 255, 255, 255],
/// });
/// let tri = Triangle {
///     verts: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
///     uvs: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
///     color: [1.0, 1.0, 1.0],
///     texture: Some(tex),
/// };
/// let _ = tri;
/// ```
pub struct Triangle {
    /// Triangle vertex positions.
    pub verts: [Vec3; 3],
    /// Triangle UV coordinates.
    pub uvs: [Vec2; 3],
    /// Base RGB color.
    pub color: [f32; 3],
    /// Optional texture data.
    pub texture: Option<Arc<TextureData>>,
}

/// Represents loaded model data ready for rendering.
///
/// # Examples
/// ```
/// use glimpse::formats::{ModelData, Triangle};
///
/// let tri = Triangle {
///     verts: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
///     uvs: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
///     color: [1.0, 1.0, 1.0],
///     texture: None,
/// };
/// let model = ModelData { triangles: vec![tri] };
/// assert_eq!(model.triangles.len(), 1);
/// ```
pub struct ModelData {
    /// Triangles ready for rasterization.
    pub triangles: Vec<Triangle>,
}

/// The result type for format loading.
///
/// # Examples
/// ```
/// use glimpse::formats::{LoadError, LoadResult};
///
/// let result: LoadResult = Err(LoadError::UnrecognizedFormat);
/// assert!(result.is_err());
/// ```
pub type LoadResult = Result<ModelData, LoadError>;

/// Errors that can occur during format loading.
///
/// # Examples
/// ```
/// use glimpse::formats::LoadError;
///
/// let err = LoadError::NoGeometry;
/// assert_eq!(format!("{}", err), "No geometry found");
/// ```
#[derive(Debug)]
pub enum LoadError {
    /// Represents invalid or corrupted file data.
    InvalidData(String),
    /// Indicates the file format is not recognized.
    UnrecognizedFormat,
    /// Represents an IO error reading the file.
    IoError(std::io::Error),
    /// Indicates no geometry was found in the model.
    NoGeometry,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            LoadError::UnrecognizedFormat => write!(f, "Unrecognized format"),
            LoadError::IoError(e) => write!(f, "IO error: {}", e),
            LoadError::NoGeometry => write!(f, "No geometry found"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        LoadError::IoError(e)
    }
}

/// A trait for format-specific model loaders.
///
/// # Examples
/// ```
/// use glimpse::formats::{self, FormatLoader};
///
/// let loader = formats::gltf::GltfLoader;
/// assert_eq!(loader.name(), "glTF");
/// ```
pub trait FormatLoader: Send + Sync {
    /// Returns the human-readable name for this format.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::{self, FormatLoader};
    ///
    /// let loader = formats::gltf::GltfLoader;
    /// assert_eq!(loader.name(), "glTF");
    /// ```
    fn name(&self) -> &'static str;

    /// Returns the file extensions this loader handles (lowercase, without dot).
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::{self, FormatLoader};
    ///
    /// let loader = formats::gltf::GltfLoader;
    /// assert!(loader.extensions().contains(&"gltf"));
    /// ```
    fn extensions(&self) -> &'static [&'static str];

    /// Checks whether this loader can handle the given data.
    ///
    /// This should be a quick check (e.g., magic bytes, initial JSON structure)
    /// without fully parsing the file.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::{self, FormatLoader};
    ///
    /// let loader = formats::gltf::GltfLoader;
    /// assert!(loader.can_load(b"glTF", Some("glb")));
    /// ```
    fn can_load(&self, data: &[u8], extension: Option<&str>) -> bool;

    /// Loads a model from raw bytes.
    ///
    /// # Errors
    /// Returns an error if the data cannot be parsed or contains no geometry.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::{self, FormatLoader};
    ///
    /// let loader = formats::gltf::GltfLoader;
    /// let result = loader.load_from_bytes(b"invalid");
    /// assert!(result.is_err());
    /// ```
    fn load_from_bytes(&self, data: &[u8]) -> LoadResult;

    /// Loads a model from a file path.
    ///
    /// Default implementation reads the file and calls `load_from_bytes`,
    /// but loaders can override this to resolve external resources.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Examples
    /// ```
    /// use std::path::Path;
    ///
    /// use glimpse::formats::{self, FormatLoader};
    ///
    /// let loader = formats::gltf::GltfLoader;
    /// let result = loader.load_from_path(Path::new("does_not_exist.gltf"));
    /// assert!(result.is_err());
    /// ```
    fn load_from_path(&self, path: &Path) -> LoadResult {
        let data = std::fs::read(path)?;
        self.load_from_bytes(&data)
    }
}

/// Returns all registered format loaders.
///
/// # Examples
/// ```
/// use glimpse::formats;
///
/// let loaders = formats::get_loaders();
/// assert!(!loaders.is_empty());
/// ```
pub fn get_loaders() -> Vec<Box<dyn FormatLoader>> {
    vec![
        Box::new(gltf::GltfLoader),
        Box::new(bbmodel::BbmodelLoader),
        Box::new(vintagestory::VintageStoryLoader),
    ]
}

/// Finds a loader that can handle the given data and extension.
///
/// # Examples
/// ```
/// use glimpse::formats;
///
/// let loader = formats::find_loader(b"glTF", Some("glb"));
/// assert!(loader.is_some());
/// ```
pub fn find_loader(data: &[u8], extension: Option<&str>) -> Option<Box<dyn FormatLoader>> {
    let mut loaders = get_loaders();

    // First, try to match by extension if provided
    if let Some(ext) = extension {
        let ext_lower = ext.to_lowercase();
        if let Some(idx) = loaders.iter().position(|loader| {
            loader.extensions().contains(&ext_lower.as_str())
                && loader.can_load(data, Some(&ext_lower))
        }) {
            return Some(loaders.swap_remove(idx));
        }
    }

    // Fall back to content-based detection
    loaders.into_iter().find(|loader| loader.can_load(data, extension))
}

/// Loads a model from bytes, auto-detecting the format.
///
/// # Errors
/// Returns an error if no loader recognizes the data or parsing fails.
///
/// # Examples
/// ```
/// use glimpse::formats::{self, LoadError};
///
/// let result = formats::load_model(b"invalid", None);
/// assert!(matches!(result, Err(LoadError::UnrecognizedFormat)));
/// ```
pub fn load_model(data: &[u8], extension: Option<&str>) -> LoadResult {
    find_loader(data, extension)
        .ok_or(LoadError::UnrecognizedFormat)?
        .load_from_bytes(data)
}

/// Loads a model from a file path, auto-detecting the format.
///
/// # Errors
/// Returns an error if the file cannot be read or the format is unrecognized.
///
/// # Examples
/// ```
/// use std::path::Path;
///
/// use glimpse::formats;
///
/// let result = formats::load_model_from_path(Path::new("does_not_exist.gltf"));
/// assert!(result.is_err());
/// ```
pub fn load_model_from_path(path: &Path) -> LoadResult {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());

    let data = std::fs::read(path)?;

    let loader = find_loader(&data, extension.as_deref()).ok_or(LoadError::UnrecognizedFormat)?;

    // Use path-based loading for formats that need external resource resolution
    loader.load_from_path(path)
}
