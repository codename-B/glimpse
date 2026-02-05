//! Provides rotation utilities for cube-based 3D models using glam.
//!
//! Provides Euler angle rotation around an origin point, used by both
//! Blockbench and Vintage Story format loaders.
//!
//! # Examples
//! ```
//! use glimpse::formats::shared::rotation::rotate_vertices_xyz;
//!
//! let vertices = [
//!     [0.0, 0.0, 0.0],
//!     [1.0, 0.0, 0.0],
//!     [1.0, 1.0, 0.0],
//!     [0.0, 1.0, 0.0],
//!     [0.0, 0.0, 1.0],
//!     [1.0, 0.0, 1.0],
//!     [1.0, 1.0, 1.0],
//!     [0.0, 1.0, 1.0],
//! ];
//! let rotated = rotate_vertices_xyz(&vertices, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
//! assert_eq!(rotated[0], vertices[0]);
//! ```

use crate::formats::Vec3;
use glam::{EulerRot, Mat4, Vec3 as GlamVec3};

/// The Euler rotation order for applying rotations.
///
/// Different model formats use different conventions:
/// - Blockbench "free" format uses ZYX
/// - Vintage Story / Minecraft typically use XYZ
///
/// # Examples
/// ```
/// use glimpse::formats::shared::rotation::RotationOrder;
///
/// let order = RotationOrder::XYZ;
/// assert_eq!(order, RotationOrder::XYZ);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RotationOrder {
    #[default]
    XYZ,
    ZYX,
}


/// Represents a rotation transform with origin and Euler angles (degrees).
///
/// # Examples
/// ```
/// use glimpse::formats::shared::rotation::RotationTransform;
///
/// let transform = RotationTransform::new([0.0, 0.0, 0.0], [0.0, 90.0, 0.0]);
/// assert!(!transform.is_zero());
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct RotationTransform {
    /// The point to rotate around.
    pub origin: Vec3,
    /// Rotation angles in degrees [X, Y, Z].
    pub angles: Vec3,
    /// Euler rotation order.
    pub order: RotationOrder,
}

impl RotationTransform {
    /// Creates a new rotation transform with default (XYZ) order.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::rotation::RotationTransform;
    ///
    /// let transform = RotationTransform::new([0.0, 0.0, 0.0], [0.0, 45.0, 0.0]);
    /// assert!(!transform.is_zero());
    /// ```
    pub fn new(origin: Vec3, angles: Vec3) -> Self {
        Self {
            origin,
            angles,
            order: RotationOrder::default(),
        }
    }

    /// Creates a new rotation transform with a specific Euler order.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::rotation::{RotationOrder, RotationTransform};
    ///
    /// let transform = RotationTransform::with_order([0.0, 0.0, 0.0], [0.0, 45.0, 0.0], RotationOrder::ZYX);
    /// assert_eq!(transform.order, RotationOrder::ZYX);
    /// ```
    pub fn with_order(origin: Vec3, angles: Vec3, order: RotationOrder) -> Self {
        Self {
            origin,
            angles,
            order,
        }
    }

    /// Checks whether this rotation is effectively zero (no rotation needed).
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::rotation::RotationTransform;
    ///
    /// let transform = RotationTransform::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    /// assert!(transform.is_zero());
    /// ```
    pub fn is_zero(&self) -> bool {
        self.angles[0].abs() < 0.001
            && self.angles[1].abs() < 0.001
            && self.angles[2].abs() < 0.001
    }

    /// Creates a rotation transform, returning None if rotation is effectively zero.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::rotation::RotationTransform;
    ///
    /// let transform = RotationTransform::new_if_non_zero([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    /// assert!(transform.is_none());
    /// ```
    pub fn new_if_non_zero(origin: Vec3, angles: Vec3) -> Option<Self> {
        let transform = Self::new(origin, angles);
        if transform.is_zero() {
            None
        } else {
            Some(transform)
        }
    }

    /// Creates a rotation transform with specific order, returning None if effectively zero.
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::rotation::{RotationOrder, RotationTransform};
    ///
    /// let transform = RotationTransform::new_if_non_zero_with_order(
    ///     [0.0, 0.0, 0.0],
    ///     [0.0, 0.0, 0.0],
    ///     RotationOrder::XYZ,
    /// );
    /// assert!(transform.is_none());
    /// ```
    pub fn new_if_non_zero_with_order(
        origin: Vec3,
        angles: Vec3,
        order: RotationOrder,
    ) -> Option<Self> {
        let transform = Self::with_order(origin, angles, order);
        if transform.is_zero() {
            None
        } else {
            Some(transform)
        }
    }

    /// Converts to a glam transformation matrix.
    ///
    /// Creates a matrix that:
    /// 1. Translates to origin
    /// 2. Rotates using the configured Euler order
    /// 3. Translates back
    ///
    /// # Examples
    /// ```
    /// use glimpse::formats::shared::rotation::RotationTransform;
    ///
    /// let transform = RotationTransform::new([0.0, 0.0, 0.0], [0.0, 90.0, 0.0]);
    /// let matrix = transform.to_matrix();
    /// let _ = matrix;
    /// ```
    pub fn to_matrix(&self) -> Mat4 {
        let origin = GlamVec3::from_array(self.origin);
        let angles_rad = GlamVec3::new(
            self.angles[0].to_radians(),
            self.angles[1].to_radians(),
            self.angles[2].to_radians(),
        );

        // Build transformation: translate to origin, rotate, translate back
        let to_origin = Mat4::from_translation(-origin);
        // glam's from_euler expects angles in the rotation sequence order,
        // not fixed X/Y/Z order. E.g. ZYX takes (z_angle, y_angle, x_angle).
        let rotation = match self.order {
            RotationOrder::XYZ => {
                Mat4::from_euler(EulerRot::XYZ, angles_rad.x, angles_rad.y, angles_rad.z)
            }
            RotationOrder::ZYX => {
                Mat4::from_euler(EulerRot::ZYX, angles_rad.z, angles_rad.y, angles_rad.x)
            }
        };
        let from_origin = Mat4::from_translation(origin);

        from_origin * rotation * to_origin
    }
}

/// Rotates 8 cube vertices using a rotation transform.
///
/// Angles are in degrees. Uses the transform's configured Euler order.
///
/// # Arguments
/// * `vertices` - The 8 vertices of a cube
/// * `transform` - The rotation transform (origin, angles, order)
///
/// # Returns
/// The rotated vertices
///
/// # Examples
/// ```
/// use glimpse::formats::shared::rotation::{rotate_vertices, RotationTransform};
///
/// let vertices = [
///     [0.0, 0.0, 0.0],
///     [1.0, 0.0, 0.0],
///     [1.0, 1.0, 0.0],
///     [0.0, 1.0, 0.0],
///     [0.0, 0.0, 1.0],
///     [1.0, 0.0, 1.0],
///     [1.0, 1.0, 1.0],
///     [0.0, 1.0, 1.0],
/// ];
/// let transform = RotationTransform::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
/// let rotated = rotate_vertices(&vertices, &transform);
/// assert_eq!(rotated[0], vertices[0]);
/// ```
pub fn rotate_vertices(vertices: &[Vec3; 8], transform: &RotationTransform) -> [Vec3; 8] {
    if transform.is_zero() {
        return *vertices;
    }

    let matrix = transform.to_matrix();

    let mut result = [Vec3::default(); 8];
    for (i, v) in vertices.iter().enumerate() {
        let point = GlamVec3::from_array(*v);
        let rotated = matrix.transform_point3(point);
        result[i] = rotated.to_array();
    }
    result
}

/// Rotates 8 cube vertices around axes through an origin point (XYZ order).
///
/// Convenience wrapper using default XYZ Euler order.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::rotation::rotate_vertices_xyz;
///
/// let vertices = [
///     [0.0, 0.0, 0.0],
///     [1.0, 0.0, 0.0],
///     [1.0, 1.0, 0.0],
///     [0.0, 1.0, 0.0],
///     [0.0, 0.0, 1.0],
///     [1.0, 0.0, 1.0],
///     [1.0, 1.0, 1.0],
///     [0.0, 1.0, 1.0],
/// ];
/// let rotated = rotate_vertices_xyz(&vertices, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
/// assert_eq!(rotated[0], vertices[0]);
/// ```
pub fn rotate_vertices_xyz(vertices: &[Vec3; 8], origin: Vec3, angles: Vec3) -> [Vec3; 8] {
    rotate_vertices(vertices, &RotationTransform::new(origin, angles))
}

/// Applies a rotation transform to cube vertices.
///
/// # Examples
/// ```
/// use glimpse::formats::shared::rotation::{apply_rotation, RotationTransform};
///
/// let vertices = [
///     [0.0, 0.0, 0.0],
///     [1.0, 0.0, 0.0],
///     [1.0, 1.0, 0.0],
///     [0.0, 1.0, 0.0],
///     [0.0, 0.0, 1.0],
///     [1.0, 0.0, 1.0],
///     [1.0, 1.0, 1.0],
///     [0.0, 1.0, 1.0],
/// ];
/// let transform = RotationTransform::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
/// let rotated = apply_rotation(&vertices, transform);
/// assert_eq!(rotated[0], vertices[0]);
/// ```
pub fn apply_rotation(vertices: &[Vec3; 8], transform: RotationTransform) -> [Vec3; 8] {
    rotate_vertices(vertices, &transform)
}

/// Applies a chain of rotation transforms to cube vertices (in order).
///
/// # Examples
/// ```
/// use glimpse::formats::shared::rotation::{apply_rotations, RotationTransform};
///
/// let vertices = [
///     [0.0, 0.0, 0.0],
///     [1.0, 0.0, 0.0],
///     [1.0, 1.0, 0.0],
///     [0.0, 1.0, 0.0],
///     [0.0, 0.0, 1.0],
///     [1.0, 0.0, 1.0],
///     [1.0, 1.0, 1.0],
///     [0.0, 1.0, 1.0],
/// ];
/// let transforms = [
///     RotationTransform::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
///     RotationTransform::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
/// ];
/// let rotated = apply_rotations(&vertices, &transforms);
/// assert_eq!(rotated[0], vertices[0]);
/// ```
pub fn apply_rotations(vertices: &[Vec3; 8], transforms: &[RotationTransform]) -> [Vec3; 8] {
    let mut result = *vertices;
    for transform in transforms {
        result = rotate_vertices(&result, transform);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_transform_is_zero() {
        let zero = RotationTransform::new([0.0; 3], [0.0; 3]);
        assert!(zero.is_zero());

        let non_zero = RotationTransform::new([0.0; 3], [45.0, 0.0, 0.0]);
        assert!(!non_zero.is_zero());

        let tiny = RotationTransform::new([0.0; 3], [0.0001, 0.0, 0.0]);
        assert!(tiny.is_zero());
    }

    #[test]
    fn test_no_rotation_preserves_vertices() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];

        let result = rotate_vertices_xyz(&vertices, [0.5, 0.5, 0.5], [0.0, 0.0, 0.0]);

        for (orig, rotated) in vertices.iter().zip(result.iter()) {
            assert!((orig[0] - rotated[0]).abs() < 0.0001);
            assert!((orig[1] - rotated[1]).abs() < 0.0001);
            assert!((orig[2] - rotated[2]).abs() < 0.0001);
        }
    }

    #[test]
    fn test_90_degree_y_rotation() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];

        // Rotate 90 degrees around Y axis through origin (0,0,0)
        let result = rotate_vertices_xyz(&vertices, [0.0, 0.0, 0.0], [0.0, 90.0, 0.0]);

        // After 90° Y rotation: (x, y, z) -> (z, y, -x)
        // Vertex [1,0,0] should become approximately [0, 0, -1]
        assert!((result[1][0] - 0.0).abs() < 0.0001);
        assert!((result[1][1] - 0.0).abs() < 0.0001);
        assert!((result[1][2] - (-1.0)).abs() < 0.0001);
    }

    #[test]
    fn test_rotation_around_offset_origin() {
        // Cube centered at (0.5, 0.5, 0.5)
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];

        // Rotate 180 degrees around Y axis through cube center
        let result = rotate_vertices_xyz(&vertices, [0.5, 0.5, 0.5], [0.0, 180.0, 0.0]);

        // After 180° Y rotation around center, vertex [0,0,0] should become [1,0,1]
        assert!((result[0][0] - 1.0).abs() < 0.0001);
        assert!((result[0][1] - 0.0).abs() < 0.0001);
        assert!((result[0][2] - 1.0).abs() < 0.0001);
    }
}
