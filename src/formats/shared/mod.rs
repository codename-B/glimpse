//! Provides shared utilities for format loaders.
//!
//! This module provides common functionality used by multiple format loaders:
//! - Cube geometry (vertices, faces, triangles)
//! - Rotation transforms
//! - JSON parsing helpers
//! - Texture loading
//!
//! # Examples
//! ```
//! use glimpse::formats::shared::scale_vec3;
//!
//! let scaled = scale_vec3([1.0, 2.0, 3.0], 0.5);
//! assert_eq!(scaled, [0.5, 1.0, 1.5]);
//! ```

pub mod cube;
pub mod json;
pub mod rotation;
pub mod texture;

pub use cube::{compute_cube_vertices, scale_vec3, CubeFace, BLOCK_SCALE, CUBE_FACES};
pub use json::{json_str_or_none, parse_vec3};
pub use rotation::{rotate_vertices, rotate_vertices_xyz, RotationOrder, RotationTransform};
