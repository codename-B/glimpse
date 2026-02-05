//! Provides cube geometry utilities for voxel-based 3D models.
//!
//! Standard cube vertex computation and face definitions used by
//! Blockbench and Vintage Story format loaders.
//!
//! # Examples
//! ```
//! use glimpse::formats::shared::compute_cube_vertices;
//!
//! let verts = compute_cube_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
//! assert_eq!(verts[0], [0.0, 0.0, 0.0]);
//! ```

use crate::formats::{Triangle, Vec2, Vec3};
use std::sync::Arc;

use crate::formats::TextureData;

/// Scale factor for Minecraft/Blockbench coordinates (16 units = 1 block).
///
/// # Examples
/// ```
/// use glimpse::formats::shared::BLOCK_SCALE;
///
/// assert_eq!(BLOCK_SCALE, 1.0 / 16.0);
/// ```
pub const BLOCK_SCALE: f32 = 1.0 / 16.0;

/// Scales a Vec3 by a scalar factor.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::scale_vec3;
///
/// let scaled = scale_vec3([2.0, 4.0, 6.0], 0.5);
/// assert_eq!(scaled, [1.0, 2.0, 3.0]);
/// ```
#[inline]
pub fn scale_vec3(v: crate::formats::Vec3, scale: f32) -> crate::formats::Vec3 {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

/// Cube face directions.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::cube::FaceDirection;
///
/// assert_eq!(FaceDirection::North.normal(), [0.0, 0.0, -1.0]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceDirection {
    North, // -Z
    South, // +Z
    East,  // +X
    West,  // -X
    Up,    // +Y
    Down,  // -Y
}

impl FaceDirection {
    /// Returns the normal vector for this face direction.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::cube::FaceDirection;
    ///
    /// let normal = FaceDirection::Up.normal();
    /// assert_eq!(normal, [0.0, 1.0, 0.0]);
    /// ```
    pub fn normal(&self) -> Vec3 {
        match self {
            FaceDirection::North => [0.0, 0.0, -1.0],
            FaceDirection::South => [0.0, 0.0, 1.0],
            FaceDirection::East => [1.0, 0.0, 0.0],
            FaceDirection::West => [-1.0, 0.0, 0.0],
            FaceDirection::Up => [0.0, 1.0, 0.0],
            FaceDirection::Down => [0.0, -1.0, 0.0],
        }
    }
}

/// Represents a face of a cube with vertex indices.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::cube::{CubeFace, FaceDirection};
///
/// let face = CubeFace {
///     direction: FaceDirection::North,
///     indices: [0, 1, 2, 3],
/// };
/// let _ = face;
/// ```
#[derive(Clone, Copy, Debug)]
pub struct CubeFace {
    /// The direction this face points.
    pub direction: FaceDirection,
    /// Indices into the 8-vertex cube array (counter-clockwise quad).
    pub indices: [usize; 4],
}

/// Standard cube face definitions.
///
/// Uses the standard vertex layout from `compute_cube_vertices`:
/// - 0: min corner (from)
/// - 1: +X from min
/// - 2: +X +Y from min
/// - 3: +Y from min
/// - 4: +Z from min
/// - 5: +X +Z from min
/// - 6: max corner (+X +Y +Z, i.e., to)
/// - 7: +Y +Z from min
///
/// Note: Different formats may use different winding orders for UV mapping.
/// These are the standard indices; format loaders may need to remap them.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::CUBE_FACES;
///
/// assert_eq!(CUBE_FACES.len(), 6);
/// ```
pub const CUBE_FACES: [CubeFace; 6] = [
    CubeFace {
        direction: FaceDirection::North,
        indices: [0, 3, 2, 1], // -Z face
    },
    CubeFace {
        direction: FaceDirection::South,
        indices: [5, 6, 7, 4], // +Z face
    },
    CubeFace {
        direction: FaceDirection::East,
        indices: [1, 2, 6, 5], // +X face
    },
    CubeFace {
        direction: FaceDirection::West,
        indices: [4, 7, 3, 0], // -X face
    },
    CubeFace {
        direction: FaceDirection::Up,
        indices: [3, 7, 6, 2], // +Y face
    },
    CubeFace {
        direction: FaceDirection::Down,
        indices: [0, 1, 5, 4], // -Y face
    },
];

/// Computes the 8 vertices of an axis-aligned cube.
///
/// Returns vertices in standard order:
/// ```text
///     3-------2      Y+
///    /|      /|      |
///   7-------6 |      |
///   | |     | |      +--- X+
///   | 0-----|-1     /
///   |/      |/     Z+
///   4-------5
/// ```
///
/// - Vertex 0: from (min corner)
/// - Vertex 6: to (max corner)
///
/// # Examples
/// ```
/// use glimpse::formats::shared::compute_cube_vertices;
///
/// let vertices = compute_cube_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
/// assert_eq!(vertices[6], [1.0, 1.0, 1.0]);
/// ```
pub fn compute_cube_vertices(from: Vec3, to: Vec3) -> [Vec3; 8] {
    [
        [from[0], from[1], from[2]], // 0: min corner
        [to[0], from[1], from[2]],   // 1: +X
        [to[0], to[1], from[2]],     // 2: +X +Y
        [from[0], to[1], from[2]],   // 3: +Y
        [from[0], from[1], to[2]],   // 4: +Z
        [to[0], from[1], to[2]],     // 5: +X +Z
        [to[0], to[1], to[2]],       // 6: max corner (+X +Y +Z)
        [from[0], to[1], to[2]],     // 7: +Y +Z
    ]
}

/// Default UV coordinates for a full face (0-1 range).
///
/// # Examples
/// ```
/// use glimpse::formats::shared::cube::DEFAULT_UVS;
///
/// assert_eq!(DEFAULT_UVS[0], [0.0, 0.0]);
/// ```
pub const DEFAULT_UVS: [Vec2; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

/// Creates two triangles from a quad (4 vertices).
///
/// # Arguments
/// * `vertices` - The 8 cube vertices
/// * `indices` - 4 vertex indices forming the quad
/// * `uvs` - UV coordinates for each corner of the quad
/// * `color` - RGB color
/// * `texture` - Optional texture
///
/// # Returns
/// Two triangles: (0,1,2) and (0,2,3)
///
/// # Examples
/// ```
/// use glimpse::formats::shared::cube::{compute_cube_vertices, quad_to_triangles, DEFAULT_UVS};
///
/// let vertices = compute_cube_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
/// let tris = quad_to_triangles(&vertices, [0, 1, 2, 3], DEFAULT_UVS, [1.0, 1.0, 1.0], None);
/// assert_eq!(tris.len(), 2);
/// ```
pub fn quad_to_triangles(
    vertices: &[Vec3; 8],
    indices: [usize; 4],
    uvs: [Vec2; 4],
    color: [f32; 3],
    texture: Option<Arc<TextureData>>,
) -> [Triangle; 2] {
    [
        Triangle {
            verts: [
                vertices[indices[0]],
                vertices[indices[1]],
                vertices[indices[2]],
            ],
            uvs: [uvs[0], uvs[1], uvs[2]],
            color,
            texture: texture.clone(),
        },
        Triangle {
            verts: [
                vertices[indices[0]],
                vertices[indices[2]],
                vertices[indices[3]],
            ],
            uvs: [uvs[0], uvs[2], uvs[3]],
            color,
            texture,
        },
    ]
}

/// Applies UV rotation (0, 90, 180, 270 degrees clockwise).
///
/// With CCW corner order (TL->TR->BR->BL), UV rotation shifts the corner assignments.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::cube::apply_uv_rotation;
///
/// let uvs = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
/// let rotated = apply_uv_rotation(uvs, 90.0);
/// assert_eq!(rotated[0], uvs[3]);
/// ```
pub fn apply_uv_rotation(uvs: [Vec2; 4], rotation_degrees: f32) -> [Vec2; 4] {
    let steps = ((rotation_degrees / 90.0).round() as i32).rem_euclid(4);
    let mut result = uvs;
    result.rotate_right(steps as usize);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_cube_vertices() {
        let vertices = compute_cube_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);

        // Check min corner
        assert_eq!(vertices[0], [0.0, 0.0, 0.0]);
        // Check max corner
        assert_eq!(vertices[6], [1.0, 1.0, 1.0]);
        // Check a middle vertex
        assert_eq!(vertices[5], [1.0, 0.0, 1.0]); // +X +Z
    }

    #[test]
    fn test_face_normals() {
        assert_eq!(FaceDirection::North.normal(), [0.0, 0.0, -1.0]);
        assert_eq!(FaceDirection::Up.normal(), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_uv_rotation() {
        let uvs: [Vec2; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

        // 90 degree rotation should shift by 1
        let rotated = apply_uv_rotation(uvs, 90.0);
        assert_eq!(rotated[0], uvs[3]); // First element becomes last

        // 0 degree rotation should be identity
        let no_rotation = apply_uv_rotation(uvs, 0.0);
        assert_eq!(no_rotation, uvs);

        // 360 degree rotation should be identity
        let full_rotation = apply_uv_rotation(uvs, 360.0);
        assert_eq!(full_rotation, uvs);
    }

    #[test]
    fn test_quad_to_triangles() {
        let vertices = compute_cube_vertices([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let triangles = quad_to_triangles(
            &vertices,
            [0, 1, 2, 3],
            DEFAULT_UVS,
            [1.0, 1.0, 1.0],
            None,
        );

        assert_eq!(triangles.len(), 2);
        // First triangle uses indices 0, 1, 2
        assert_eq!(triangles[0].verts[0], vertices[0]);
        assert_eq!(triangles[0].verts[1], vertices[1]);
        assert_eq!(triangles[0].verts[2], vertices[2]);
        // Second triangle uses indices 0, 2, 3
        assert_eq!(triangles[1].verts[0], vertices[0]);
        assert_eq!(triangles[1].verts[1], vertices[2]);
        assert_eq!(triangles[1].verts[2], vertices[3]);
    }
}

