//! Provides a Vintage Story JSON model format loader.
//!
//! Vintage Story uses a JSON5-based model format similar to Minecraft/Blockbench
//! but with some differences. This loader renders shapes only (solid color)
//! since textures are external files.
//!
//! # Examples
//! ```
//! use glimpse::formats::{self, FormatLoader};
//!
//! let loader = formats::vintagestory::VintageStoryLoader;
//! assert!(loader.extensions().contains(&"json"));
//! ```

use std::path::Path;

use serde::Deserialize;

use super::shared::cube::{
    apply_uv_rotation, compute_cube_vertices, quad_to_triangles, scale_vec3, BLOCK_SCALE,
    DEFAULT_UVS,
};
use super::shared::rotation::{rotate_vertices, RotationTransform};
use super::{FormatLoader, LoadError, LoadResult, ModelData, Triangle, Vec3};

/// The Vintage Story format loader.
///
/// # Examples
/// ```
/// use glimpse::formats::{self, FormatLoader};
///
/// let loader = formats::vintagestory::VintageStoryLoader;
/// assert_eq!(loader.name(), "Vintage Story");
/// ```
pub struct VintageStoryLoader;

impl FormatLoader for VintageStoryLoader {
    fn name(&self) -> &'static str {
        "Vintage Story"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn can_load(&self, data: &[u8], extension: Option<&str>) -> bool {
        // If extension is provided and it's not "json", reject immediately
        if let Some(ext) = extension {
            if ext.to_lowercase() != "json" {
                return false;
            }
        }

        // Content-based detection for Vintage Story format
        // VS files have "elements" array with cardinal face names and from/to cube definitions
        // Note: JSON5 allows unquoted keys, so check for both quoted and unquoted versions
        if let Ok(text) = std::str::from_utf8(data) {
            let sample = &text[..text.len().min(4000)];

            // Must have "elements" array (quoted or unquoted key)
            let has_elements = sample.contains("\"elements\"") || sample.contains("elements:");

            if !has_elements {
                return false;
            }

            // Must have at least one cardinal face name (north, south, east, west, up, down)
            // Check both quoted and unquoted versions
            let has_faces = sample.contains("\"north\"")
                || sample.contains("\"south\"")
                || sample.contains("\"east\"")
                || sample.contains("\"west\"")
                || sample.contains("\"up\"")
                || sample.contains("\"down\"")
                || sample.contains("north:")
                || sample.contains("south:")
                || sample.contains("east:")
                || sample.contains("west:")
                || sample.contains("up:")
                || sample.contains("down:");

            // Should have "from" and "to" for cube definitions
            let has_cube_def = (sample.contains("\"from\"") || sample.contains("from:"))
                && (sample.contains("\"to\"") || sample.contains("to:"));

            return has_faces && has_cube_def;
        }

        false
    }

    fn load_from_bytes(&self, data: &[u8]) -> LoadResult {
        let text = std::str::from_utf8(data)
            .map_err(|_| LoadError::InvalidData("Invalid UTF-8 in VS model file".to_string()))?;

        // Use json5 crate to handle comments in VS files
        let model: VsModelFile = json5::from_str(text)
            .map_err(|e| LoadError::InvalidData(format!("Failed to parse VS JSON: {}", e)))?;

        convert_vs_model_to_triangles(model)
    }

    fn load_from_path(&self, path: &Path) -> LoadResult {
        let data = std::fs::read(path)?;
        self.load_from_bytes(&data)
    }
}

// ---- Vintage Story JSON structure ----

#[derive(Deserialize)]
#[allow(dead_code)]
struct VsModelFile {
    #[serde(default)]
    elements: Vec<VsElement>,
    #[serde(default, rename = "textureWidth")]
    texture_width: Option<u32>,
    #[serde(default, rename = "textureHeight")]
    texture_height: Option<u32>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct VsElement {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    from: [f32; 3],
    #[serde(default)]
    to: [f32; 3],
    #[serde(default)]
    faces: VsFaces,
    #[serde(default, rename = "rotationOrigin")]
    rotation_origin: Option<[f32; 3]>,
    #[serde(default, rename = "rotationX")]
    rotation_x: Option<f32>,
    #[serde(default, rename = "rotationY")]
    rotation_y: Option<f32>,
    #[serde(default, rename = "rotationZ")]
    rotation_z: Option<f32>,
    #[serde(default)]
    children: Vec<VsElement>,
}

#[derive(Deserialize, Default)]
struct VsFaces {
    north: Option<VsFace>,
    south: Option<VsFace>,
    east: Option<VsFace>,
    west: Option<VsFace>,
    up: Option<VsFace>,
    down: Option<VsFace>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct VsFace {
    #[serde(default)]
    texture: Option<String>,
    #[serde(default)]
    uv: Option<[f32; 4]>,
    #[serde(default)]
    rotation: Option<f32>,
    #[serde(default)]
    enabled: Option<bool>,
}

/// Converts a Vintage Story model to triangles.
fn convert_vs_model_to_triangles(model: VsModelFile) -> LoadResult {
    let mut triangles = Vec::new();

    let tex_width = model.texture_width.unwrap_or(16) as f32;
    let tex_height = model.texture_height.unwrap_or(16) as f32;

    // Convert each element (cube) to triangles
    // Root elements use zero offset; children accumulate parent positions.
    for element in &model.elements {
        convert_vs_element_recursive(
            element,
            &mut triangles,
            &[],
            [0.0; 3],
            tex_width,
            tex_height,
        );
    }

    if triangles.is_empty() {
        return Err(LoadError::NoGeometry);
    }

    Ok(ModelData { triangles })
}

/// Returns the rotation angles from a VS element.
fn vs_element_rotation(element: &VsElement) -> Vec3 {
    [
        element.rotation_x.unwrap_or(0.0),
        element.rotation_y.unwrap_or(0.0),
        element.rotation_z.unwrap_or(0.0),
    ]
}

/// Recursively converts VS elements to triangles.
fn convert_vs_element_recursive(
    element: &VsElement,
    triangles: &mut Vec<Triangle>,
    parent_rotations: &[RotationTransform],
    parent_offset: Vec3,
    tex_width: f32,
    tex_height: f32,
) {
    let cubes = convert_vs_cube_to_triangles(
        element,
        parent_rotations,
        parent_offset,
        tex_width,
        tex_height,
    );
    triangles.extend(cubes);

    let elem_angles = vs_element_rotation(element);
    let raw_origin = element.rotation_origin.unwrap_or([0.0; 3]);
    let world_origin = [
        raw_origin[0] + parent_offset[0],
        raw_origin[1] + parent_offset[1],
        raw_origin[2] + parent_offset[2],
    ];
    let elem_transform = RotationTransform::new_if_non_zero(world_origin, elem_angles);

    let child_rotations;
    let rotations_for_children = if let Some(transform) = elem_transform {
        child_rotations = parent_rotations
            .iter()
            .copied()
            .chain(std::iter::once(transform))
            .collect::<Vec<_>>();
        &child_rotations[..]
    } else {
        parent_rotations
    };

    let child_offset = [
        parent_offset[0] + element.from[0],
        parent_offset[1] + element.from[1],
        parent_offset[2] + element.from[2],
    ];

    for child in &element.children {
        convert_vs_element_recursive(
            child,
            triangles,
            rotations_for_children,
            child_offset,
            tex_width,
            tex_height,
        );
    }
}

/// Converts a VS cube element to 12 triangles (2 per face).
fn convert_vs_cube_to_triangles(
    element: &VsElement,
    parent_rotations: &[RotationTransform],
    parent_offset: Vec3,
    tex_width: f32,
    tex_height: f32,
) -> Vec<Triangle> {
    let mut triangles = Vec::with_capacity(12);

    let scale = BLOCK_SCALE;

    // Child coordinates are relative to parent; add the accumulated offset
    // to convert to world space before scaling.
    let world_from = [
        element.from[0] + parent_offset[0],
        element.from[1] + parent_offset[1],
        element.from[2] + parent_offset[2],
    ];
    let world_to = [
        element.to[0] + parent_offset[0],
        element.to[1] + parent_offset[1],
        element.to[2] + parent_offset[2],
    ];

    let from = scale_vec3(world_from, scale);
    let to = scale_vec3(world_to, scale);

    // Compute the 8 vertices of the cube using shared utility
    let vertices = compute_cube_vertices(from, to);

    // Apply this element's own rotation first
    let elem_angles = vs_element_rotation(element);
    let mut vertices = if elem_angles[0].abs() > 0.001
        || elem_angles[1].abs() > 0.001
        || elem_angles[2].abs() > 0.001
    {
        let origin = element
            .rotation_origin
            .map(|o| {
                // Rotation origin is also in local space; offset to world space
                scale_vec3(
                    [
                        o[0] + parent_offset[0],
                        o[1] + parent_offset[1],
                        o[2] + parent_offset[2],
                    ],
                    scale,
                )
            })
            .unwrap_or_else(|| {
                [
                    (from[0] + to[0]) / 2.0,
                    (from[1] + to[1]) / 2.0,
                    (from[2] + to[2]) / 2.0,
                ]
            });
        let transform = RotationTransform::new(origin, elem_angles);
        rotate_vertices(&vertices, &transform)
    } else {
        vertices
    };

    // Then apply parent rotations from nearest parent outward to root,
    // each around the parent's own origin.
    for parent in parent_rotations.iter().rev() {
        let scaled = RotationTransform::with_order(
            scale_vec3(parent.origin, scale),
            parent.angles,
            parent.order,
        );
        vertices = rotate_vertices(&vertices, &scaled);
    }

    // Solid gray color for untextured rendering
    let default_color = [0.75, 0.75, 0.78];

    // Face definitions: (vertex indices for quad, face data)
    // Note: Vintage Story uses different winding order than standard CUBE_FACES
    let face_defs: [([usize; 4], &Option<VsFace>); 6] = [
        // North face (-Z)
        ([0, 3, 2, 1], &element.faces.north),
        // South face (+Z)
        ([5, 6, 7, 4], &element.faces.south),
        // East face (+X)
        ([1, 2, 6, 5], &element.faces.east),
        // West face (-X)
        ([4, 7, 3, 0], &element.faces.west),
        // Up face (+Y)
        ([3, 7, 6, 2], &element.faces.up),
        // Down face (-Y)
        ([0, 1, 5, 4], &element.faces.down),
    ];

    for (indices, face_opt) in face_defs {
        // Skip faces not defined in the model
        let Some(face) = face_opt else {
            continue;
        };

        // Skip explicitly disabled faces
        if face.enabled == Some(false) {
            continue;
        }

        // Compute UVs from face data, or use defaults
        let uvs = if let Some(uv) = &face.uv {
            // VS UVs are in pixel coordinates [u1, v1, u2, v2]
            let u1 = uv[0] / tex_width;
            let v1 = uv[1] / tex_height;
            let u2 = uv[2] / tex_width;
            let v2 = uv[3] / tex_height;

            let corners = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];
            apply_uv_rotation(corners, face.rotation.unwrap_or(0.0))
        } else {
            DEFAULT_UVS
        };

        // Create two triangles for this face using shared utility
        let tris = quad_to_triangles(&vertices, indices, uvs, default_color, None);
        triangles.extend(tris);
    }

    triangles
}
