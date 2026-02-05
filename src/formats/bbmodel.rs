//! Provides a Blockbench (.bbmodel) format loader.
//!
//! Blockbench is a popular free 3D modeling tool for creating Minecraft-style
//! block models. The .bbmodel format is JSON-based with embedded textures.
//!
//! # Examples
//! ```
//! use glimpse::formats::{self, FormatLoader};
//!
//! let loader = formats::bbmodel::BbmodelLoader;
//! assert!(loader.extensions().contains(&"bbmodel"));
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use super::shared::cube::{
    apply_uv_rotation, compute_cube_vertices, quad_to_triangles, scale_vec3, BLOCK_SCALE,
};
use super::shared::json::{json_str_or_none, parse_vec3};
use super::shared::rotation::{rotate_vertices, RotationOrder, RotationTransform};
use super::shared::texture::load_texture_from_data_url;
use super::{FormatLoader, LoadError, LoadResult, ModelData, TextureData, Triangle};

/// The Blockbench format loader.
///
/// # Examples
/// ```
/// use glimpse::formats::{self, FormatLoader};
///
/// let loader = formats::bbmodel::BbmodelLoader;
/// assert_eq!(loader.name(), "Blockbench");
/// ```
pub struct BbmodelLoader;

impl FormatLoader for BbmodelLoader {
    fn name(&self) -> &'static str {
        "Blockbench"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bbmodel"]
    }

    fn can_load(&self, data: &[u8], extension: Option<&str>) -> bool {
        // Check extension
        if let Some(ext) = extension {
            if ext.to_lowercase() == "bbmodel" {
                return true;
            }
        }

        // Check for Blockbench JSON structure
        if let Ok(text) = std::str::from_utf8(data) {
            let sample = &text[..text.len().min(2000)];
            // Blockbench files have "meta" with "format_version" and "elements" array
            return sample.contains("\"meta\"") && sample.contains("\"format_version\"");
        }

        false
    }

    fn load_from_bytes(&self, data: &[u8]) -> LoadResult {
        let text = std::str::from_utf8(data)
            .map_err(|_| LoadError::InvalidData("Invalid UTF-8 in bbmodel file".to_string()))?;

        // Use json5 for more lenient parsing
        let model: BbmodelFile = json5::from_str(text)
            .map_err(|e| LoadError::InvalidData(format!("Failed to parse bbmodel: {}", e)))?;

        convert_bbmodel_to_triangles(model)
    }

    fn load_from_path(&self, path: &Path) -> LoadResult {
        let data = std::fs::read(path)?;
        self.load_from_bytes(&data)
    }
}

// ---- Blockbench JSON structure ----

#[derive(Deserialize)]
#[allow(dead_code)]
struct BbmodelFile {
    #[serde(default)]
    meta: BbmodelMeta,
    #[serde(default)]
    textures: Vec<BbmodelTexture>,
    #[serde(default)]
    elements: Vec<BbmodelElement>,
    #[serde(default)]
    outliner: Vec<serde_json::Value>,
    #[serde(default)]
    groups: Vec<BbmodelGroup>,
    resolution: Option<BbmodelResolution>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
struct BbmodelMeta {
    #[serde(default)]
    model_format: String,
}

/// Determines the Euler rotation order from the Blockbench model format.
///
/// Blockbench formats can specify `euler_order` as either "XYZ" or "ZYX".
/// The "free" and "bedrock" formats use ZYX; "java_block" uses single-axis
/// rotation where order doesn't matter.
fn euler_order_for_format(format: &str) -> RotationOrder {
    match format {
        // Java block models only allow single-axis rotation, order irrelevant
        "java_block" => RotationOrder::XYZ,
        // Most Blockbench formats (free, bedrock, etc.) use ZYX
        _ => RotationOrder::ZYX,
    }
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
struct BbmodelGroup {
    #[serde(default)]
    uuid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    origin: Option<[f32; 3]>,
    #[serde(default)]
    rotation: Option<[f32; 3]>,
    #[serde(default)]
    children: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct BbmodelResolution {
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct BbmodelTexture {
    #[serde(default)]
    source: String, // Base64 data URL
    #[serde(default)]
    name: String,
    #[serde(default)]
    uuid: serde_json::Value,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    uv_width: Option<u32>,
    #[serde(default)]
    uv_height: Option<u32>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct BbmodelElement {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    from: [f32; 3],
    #[serde(default)]
    to: [f32; 3],
    #[serde(default)]
    faces: BbmodelFaces,
    #[serde(default)]
    rotation: Option<serde_json::Value>, // Can be [x,y,z] array or {angle, axis} object
    #[serde(default)]
    origin: Option<[f32; 3]>,
    #[serde(default)]
    uuid: serde_json::Value,
    #[serde(default)]
    color: Option<serde_json::Value>, // Can be integer color index
}

#[derive(Deserialize, Default)]
struct BbmodelFaces {
    north: Option<BbmodelFace>,
    south: Option<BbmodelFace>,
    east: Option<BbmodelFace>,
    west: Option<BbmodelFace>,
    up: Option<BbmodelFace>,
    down: Option<BbmodelFace>,
}

/// Blockbench face vertex indices (maps Blockbench vertex order to internal order)
const BB_FACE_INDICES: [([usize; 4], [f32; 3]); 6] = [
    ([2, 3, 0, 1], [0.0, 0.0, -1.0]), // North (-Z)
    ([7, 6, 5, 4], [0.0, 0.0, 1.0]),  // South (+Z)
    ([6, 2, 1, 5], [1.0, 0.0, 0.0]),  // East (+X)
    ([3, 7, 4, 0], [-1.0, 0.0, 0.0]), // West (-X)
    ([3, 2, 6, 7], [0.0, 1.0, 0.0]),  // Up (+Y)
    ([4, 5, 1, 0], [0.0, -1.0, 0.0]), // Down (-Y)
];

impl BbmodelFaces {
    /// Iterates over faces with their vertex indices.
    fn iter(&self) -> impl Iterator<Item = ([usize; 4], &Option<BbmodelFace>)> {
        [
            &self.north,
            &self.south,
            &self.east,
            &self.west,
            &self.up,
            &self.down,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, face)| (BB_FACE_INDICES[i].0, face))
    }
}

#[derive(Deserialize, Clone)]
struct BbmodelFace {
    #[serde(default)]
    uv: [f32; 4], // [u1, v1, u2, v2] in pixel coordinates
    #[serde(default)]
    texture: Option<serde_json::Value>, // Can be number or null
    #[serde(default)]
    rotation: Option<f32>,
    #[serde(default)]
    mirror_u: bool, // Flip texture horizontally
    #[serde(default)]
    mirror_v: bool, // Flip texture vertically
}

/// Parses rotation from either [x,y,z] array or {angle, axis} object.
fn parse_element_rotation(value: &serde_json::Value) -> Option<([f32; 3], Option<[f32; 3]>)> {
    match value {
        serde_json::Value::Array(_) => Some((parse_vec3(value)?, None)),
        serde_json::Value::Object(obj) => {
            let angle = obj.get("angle").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let axis = obj.get("axis").and_then(|v| v.as_str()).unwrap_or("y");
            let origin = obj.get("origin").and_then(parse_vec3);
            let rotation = match axis {
                "x" => [angle, 0.0, 0.0],
                "y" => [0.0, angle, 0.0],
                "z" => [0.0, 0.0, angle],
                _ => [0.0, angle, 0.0],
            };
            Some((rotation, origin))
        }
        _ => None,
    }
}

fn group_rotation_transform(
    group: &BbmodelGroup,
    order: RotationOrder,
) -> Option<RotationTransform> {
    RotationTransform::new_if_non_zero_with_order(
        group.origin.unwrap_or([0.0; 3]),
        group.rotation.unwrap_or([0.0; 3]),
        order,
    )
}

fn outliner_rotation_transform(
    obj: &serde_json::Map<String, serde_json::Value>,
    order: RotationOrder,
) -> Option<RotationTransform> {
    RotationTransform::new_if_non_zero_with_order(
        obj.get("origin").and_then(parse_vec3).unwrap_or([0.0; 3]),
        obj.get("rotation").and_then(parse_vec3).unwrap_or([0.0; 3]),
        order,
    )
}

fn collect_element_parent_rotations(
    node: &serde_json::Value,
    groups_by_uuid: &HashMap<&str, &BbmodelGroup>,
    parent_rotations: &[RotationTransform],
    element_parent_rotations: &mut HashMap<String, Vec<RotationTransform>>,
    order: RotationOrder,
) {
    match node {
        serde_json::Value::Array(children) => {
            for child in children {
                collect_element_parent_rotations(
                    child,
                    groups_by_uuid,
                    parent_rotations,
                    element_parent_rotations,
                    order,
                );
            }
        }
        serde_json::Value::String(uuid) => {
            if uuid.is_empty() {
                return;
            }

            if let Some(group) = groups_by_uuid.get(uuid.as_str()) {
                let mut next_rotations = parent_rotations.to_vec();
                if let Some(rotation) = group_rotation_transform(group, order) {
                    next_rotations.push(rotation);
                }
                for child in &group.children {
                    collect_element_parent_rotations(
                        child,
                        groups_by_uuid,
                        &next_rotations,
                        element_parent_rotations,
                        order,
                    );
                }
            } else {
                element_parent_rotations
                    .entry(uuid.clone())
                    .or_insert_with(|| parent_rotations.to_vec());
            }
        }
        serde_json::Value::Object(obj) => {
            let uuid = obj
                .get("uuid")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty());

            let mut next_rotations = parent_rotations.to_vec();
            let mut treated_as_group = false;

            if let Some(group_uuid) = uuid {
                if let Some(group) = groups_by_uuid.get(group_uuid) {
                    treated_as_group = true;
                    if let Some(rotation) = group_rotation_transform(group, order) {
                        next_rotations.push(rotation);
                    }
                } else if let Some(rotation) = outliner_rotation_transform(obj, order) {
                    treated_as_group = true;
                    next_rotations.push(rotation);
                }
            }

            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                let chain = if treated_as_group {
                    &next_rotations
                } else {
                    parent_rotations
                };
                for child in children {
                    collect_element_parent_rotations(
                        child,
                        groups_by_uuid,
                        chain,
                        element_parent_rotations,
                        order,
                    );
                }

                if children.is_empty() {
                    if let Some(element_uuid) = uuid {
                        if !treated_as_group {
                            element_parent_rotations
                                .entry(element_uuid.to_string())
                                .or_insert_with(|| parent_rotations.to_vec());
                        }
                    }
                }
            } else if treated_as_group {
                if let Some(group_uuid) = uuid {
                    if let Some(group) = groups_by_uuid.get(group_uuid) {
                        for child in &group.children {
                            collect_element_parent_rotations(
                                child,
                                groups_by_uuid,
                                &next_rotations,
                                element_parent_rotations,
                                order,
                            );
                        }
                    }
                }
            } else if let Some(element_uuid) = uuid {
                element_parent_rotations
                    .entry(element_uuid.to_string())
                    .or_insert_with(|| parent_rotations.to_vec());
            }
        }
        _ => {}
    }
}

fn build_element_parent_rotation_map(
    model: &BbmodelFile,
    order: RotationOrder,
) -> HashMap<String, Vec<RotationTransform>> {
    let groups_by_uuid: HashMap<&str, &BbmodelGroup> = model
        .groups
        .iter()
        .filter(|g| !g.uuid.is_empty())
        .map(|g| (g.uuid.as_str(), g))
        .collect();

    let mut element_parent_rotations = HashMap::new();

    for node in &model.outliner {
        collect_element_parent_rotations(
            node,
            &groups_by_uuid,
            &[],
            &mut element_parent_rotations,
            order,
        );
    }

    // Ensure every element UUID gets an entry, even if outliner data is missing.
    for element in &model.elements {
        if let Some(uuid) = json_str_or_none(&element.uuid) {
            element_parent_rotations
                .entry(uuid.to_string())
                .or_default();
        }
    }

    element_parent_rotations
}

/// Converts a Blockbench model to triangles.
fn convert_bbmodel_to_triangles(model: BbmodelFile) -> LoadResult {
    let mut triangles = Vec::new();
    let euler_order = euler_order_for_format(&model.meta.model_format);
    let element_parent_rotations = build_element_parent_rotation_map(&model, euler_order);

    // Load textures
    let textures: Vec<Option<Arc<TextureData>>> =
        model.textures.iter().map(load_bbmodel_texture).collect();

    // Get UV resolution for normalization
    // In Blockbench, UVs are in pixel coordinates based on the project resolution,
    // not the actual texture dimensions. The resolution field is used instead.
    let (uv_width, uv_height) = model
        .resolution
        .as_ref()
        .map(|r| (r.width as f32, r.height as f32))
        .unwrap_or((16.0, 16.0));

    // Get per-texture UV dimensions if available (first texture)
    // This follows Blockbench's per_texture_uv_size behavior
    let (tex_uv_width, tex_uv_height) = model
        .textures
        .first()
        .and_then(|tex| match (tex.uv_width, tex.uv_height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some((w as f32, h as f32)),
            _ => None,
        })
        .unwrap_or((uv_width, uv_height));

    // Convert each element (cube) to triangles
    for element in &model.elements {
        let parent_rotations = json_str_or_none(&element.uuid)
            .and_then(|uuid| element_parent_rotations.get(uuid))
            .map(Vec::as_slice)
            .unwrap_or(&[]);

        let cubes = convert_cube_to_triangles(
            element,
            &textures,
            tex_uv_width,
            tex_uv_height,
            parent_rotations,
            euler_order,
        );
        triangles.extend(cubes);
    }

    if triangles.is_empty() {
        return Err(LoadError::NoGeometry);
    }

    // Blockbench orientation is opposite of the expected thumbnail view.
    // Apply a bbmodel-only 180deg yaw so glTF/GLB behavior remains unchanged.
    rotate_triangles_y_180(&mut triangles);

    Ok(ModelData { triangles })
}

/// Rotates all triangles 180° around the Y axis through their collective center.
fn rotate_triangles_y_180(triangles: &mut [Triangle]) {
    if triangles.is_empty() {
        return;
    }

    let (min, max) = {
        let mut min = glam::Vec3::splat(f32::INFINITY);
        let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
        for tri in triangles.iter() {
            for v in &tri.verts {
                let p = glam::Vec3::from_array(*v);
                min = min.min(p);
                max = max.max(p);
            }
        }
        (min, max)
    };
    let center = (min + max) * 0.5;

    // 180° Y rotation = reflect X and Z through center
    for tri in triangles.iter_mut() {
        for v in &mut tri.verts {
            v[0] = 2.0 * center.x - v[0];
            v[2] = 2.0 * center.z - v[2];
        }
    }
}

/// Loads a Blockbench texture from a base64 data URL.
fn load_bbmodel_texture(texture: &BbmodelTexture) -> Option<Arc<TextureData>> {
    load_texture_from_data_url(&texture.source)
}

/// Converts a cube element to 12 triangles (2 per face).
fn convert_cube_to_triangles(
    element: &BbmodelElement,
    textures: &[Option<Arc<TextureData>>],
    tex_width: f32,
    tex_height: f32,
    parent_rotations: &[RotationTransform],
    euler_order: RotationOrder,
) -> Vec<Triangle> {
    let mut triangles = Vec::with_capacity(12);

    // Blockbench uses Minecraft coordinate system where coordinates are in 1/16 blocks
    // Scale factor to convert to a reasonable world space (16 units = 1 block)
    let scale = BLOCK_SCALE;

    // Get cube corners (from/to define opposite corners)
    let from = scale_vec3(element.from, scale);
    let to = scale_vec3(element.to, scale);

    // Compute the 8 vertices of the cube
    let vertices = compute_cube_vertices(from, to);

    // Apply this element's own rotation first.
    let mut vertices = if let Some(ref rot_value) = element.rotation {
        if let Some((angles, rot_origin)) = parse_element_rotation(rot_value) {
            let origin = rot_origin
                .or(element.origin)
                .map(|o| scale_vec3(o, scale))
                .unwrap_or([0.0; 3]);
            let transform = RotationTransform::with_order(origin, angles, euler_order);
            rotate_vertices(&vertices, &transform)
        } else {
            vertices
        }
    } else {
        vertices
    };

    // Then apply parent group rotations from nearest parent up to root.
    // Each parent transform already has the correct euler order stored.
    for parent in parent_rotations.iter().rev() {
        let scaled = RotationTransform::with_order(
            scale_vec3(parent.origin, scale),
            parent.angles,
            parent.order,
        );
        vertices = rotate_vertices(&vertices, &scaled);
    }

    // Default color (light gray)
    let default_color = [0.85, 0.85, 0.85];

    for (indices, face_opt) in element.faces.iter() {
        let face = match face_opt {
            Some(f) => f,
            None => continue,
        };

        // Check if face has a texture reference
        // In Blockbench, texture: null means the face should not be rendered
        let texture_ref = match &face.texture {
            Some(t) if !t.is_null() => t,
            _ => continue, // Skip faces with null or missing texture
        };

        // Get texture for this face
        let texture = texture_ref
            .as_u64()
            .and_then(|idx| textures.get(idx as usize))
            .and_then(|t| t.clone());

        // Calculate UV coordinates from pixel coordinates
        // Blockbench UVs are in pixel coordinates [u1, v1, u2, v2]
        let uv = &face.uv;
        let mut u1 = uv[0] / tex_width;
        let mut v1 = uv[1] / tex_height;
        let mut u2 = uv[2] / tex_width;
        let mut v2 = uv[3] / tex_height;

        // Apply explicit mirror flags (separate from implicit u1>u2 or v1>v2 mirroring)
        if face.mirror_u {
            std::mem::swap(&mut u1, &mut u2);
        }
        if face.mirror_v {
            std::mem::swap(&mut v1, &mut v2);
        }

        // UV corners for the quad (apply rotation if specified)
        let uvs = apply_uv_rotation(
            [[u1, v1], [u2, v1], [u2, v2], [u1, v2]],
            face.rotation.unwrap_or(0.0),
        );

        // Create two triangles for this face
        let tris = quad_to_triangles(&vertices, indices, uvs, default_color, texture);
        triangles.extend(tris);
    }

    triangles
}
