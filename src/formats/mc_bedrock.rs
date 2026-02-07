//! Provides a Minecraft Bedrock Edition geometry format loader.
//!
//! Bedrock geometry files use a bone-based hierarchy with cubes defined by
//! origin + size. They are identified by the `"minecraft:geometry"` key.
//! Textures are external files, so models render with solid color.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::shared::cube::{
    apply_uv_rotation, compute_cube_vertices, quad_to_triangles, scale_vec3, BLOCK_SCALE,
    DEFAULT_UVS,
};
use super::shared::rotation::{rotate_vertices, RotationOrder, RotationTransform};
use super::{FormatLoader, LoadError, LoadResult, ModelData, Triangle};

pub struct McBedrockLoader;

impl FormatLoader for McBedrockLoader {
    fn name(&self) -> &'static str {
        "Minecraft Bedrock"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn can_load(&self, data: &[u8], extension: Option<&str>) -> bool {
        if let Some(ext) = extension {
            if ext.to_lowercase() != "json" {
                return false;
            }
        }

        if let Ok(text) = std::str::from_utf8(data) {
            let sample = &text[..text.len().min(4000)];
            return sample.contains("\"minecraft:geometry\"");
        }

        false
    }

    fn load_from_bytes(&self, data: &[u8]) -> LoadResult {
        let text = std::str::from_utf8(data).map_err(|_| {
            LoadError::InvalidData("Invalid UTF-8 in Bedrock geometry file".to_string())
        })?;

        let file: BedrockFile = serde_json::from_str(text).map_err(|e| {
            LoadError::InvalidData(format!("Failed to parse Bedrock geometry: {}", e))
        })?;

        convert_bedrock_to_triangles(file)
    }

    fn load_from_path(&self, path: &Path) -> LoadResult {
        let data = std::fs::read(path)?;
        self.load_from_bytes(&data)
    }
}

// ---- Bedrock JSON structure ----

#[derive(Deserialize)]
struct BedrockFile {
    #[serde(rename = "minecraft:geometry")]
    geometry: Vec<BedrockGeometry>,
}

#[derive(Deserialize)]
struct BedrockGeometry {
    description: BedrockDescription,
    #[serde(default)]
    bones: Vec<BedrockBone>,
}

#[derive(Deserialize)]
struct BedrockDescription {
    #[serde(default = "default_16")]
    texture_width: f32,
    #[serde(default = "default_16")]
    texture_height: f32,
}

fn default_16() -> f32 {
    16.0
}

#[derive(Deserialize)]
struct BedrockBone {
    name: String,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    pivot: [f32; 3],
    #[serde(default)]
    rotation: Option<[f32; 3]>,
    #[serde(default)]
    cubes: Vec<BedrockCube>,
}

#[derive(Deserialize)]
struct BedrockCube {
    origin: [f32; 3],
    size: [f32; 3],
    #[serde(default)]
    pivot: Option<[f32; 3]>,
    #[serde(default)]
    rotation: Option<[f32; 3]>,
    #[serde(default)]
    uv: serde_json::Value,
}

#[derive(Deserialize)]
struct BedrockFaceUv {
    uv: [f32; 2],
    uv_size: [f32; 2],
    #[serde(default)]
    uv_rotation: Option<f32>,
}

#[derive(Deserialize, Default)]
struct BedrockPerFaceUv {
    north: Option<BedrockFaceUv>,
    south: Option<BedrockFaceUv>,
    east: Option<BedrockFaceUv>,
    west: Option<BedrockFaceUv>,
    up: Option<BedrockFaceUv>,
    down: Option<BedrockFaceUv>,
}

/// Bedrock face vertex indices (same winding as Blockbench).
const BEDROCK_FACE_DEFS: [([usize; 4], FaceName); 6] = [
    ([2, 3, 0, 1], FaceName::North),
    ([7, 6, 5, 4], FaceName::South),
    ([6, 2, 1, 5], FaceName::East),
    ([3, 7, 4, 0], FaceName::West),
    ([3, 2, 6, 7], FaceName::Up),
    ([4, 5, 1, 0], FaceName::Down),
];

#[derive(Clone, Copy)]
enum FaceName {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

fn convert_bedrock_to_triangles(file: BedrockFile) -> LoadResult {
    let geometry = file
        .geometry
        .into_iter()
        .next()
        .ok_or(LoadError::NoGeometry)?;

    let tex_width = geometry.description.texture_width;
    let tex_height = geometry.description.texture_height;

    let bone_chains = build_bone_rotation_chains(&geometry.bones);

    let mut triangles = Vec::new();

    for (bone_idx, bone) in geometry.bones.iter().enumerate() {
        let bone_chain = &bone_chains[bone_idx];

        for cube in &bone.cubes {
            let cube_tris = convert_bedrock_cube(cube, bone_chain, tex_width, tex_height);
            triangles.extend(cube_tris);
        }
    }

    if triangles.is_empty() {
        return Err(LoadError::NoGeometry);
    }

    rotate_triangles_y_180(&mut triangles);

    Ok(ModelData { triangles })
}

/// Builds rotation transform chains for each bone (bone → parent → ... → root).
fn build_bone_rotation_chains(bones: &[BedrockBone]) -> Vec<Vec<RotationTransform>> {
    let name_to_idx: HashMap<&str, usize> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.as_str(), i))
        .collect();

    let mut chains = Vec::with_capacity(bones.len());

    for bone in bones {
        let mut chain = Vec::new();

        // Add this bone's rotation
        if let Some(rotation) = &bone.rotation {
            if let Some(transform) = RotationTransform::new_if_non_zero_with_order(
                bone.pivot,
                *rotation,
                RotationOrder::ZYX,
            ) {
                chain.push(transform);
            }
        }

        // Walk up parent chain
        let mut current_parent = bone.parent.as_deref();
        let mut visited = std::collections::HashSet::new();
        while let Some(parent_name) = current_parent {
            if !visited.insert(parent_name) {
                break; // Avoid infinite loops from circular references
            }
            if let Some(&parent_idx) = name_to_idx.get(parent_name) {
                let parent = &bones[parent_idx];
                if let Some(rotation) = &parent.rotation {
                    if let Some(transform) = RotationTransform::new_if_non_zero_with_order(
                        parent.pivot,
                        *rotation,
                        RotationOrder::ZYX,
                    ) {
                        chain.push(transform);
                    }
                }
                current_parent = parent.parent.as_deref();
            } else {
                break;
            }
        }

        chains.push(chain);
    }

    chains
}

fn convert_bedrock_cube(
    cube: &BedrockCube,
    bone_chain: &[RotationTransform],
    tex_width: f32,
    tex_height: f32,
) -> Vec<Triangle> {
    let mut triangles = Vec::with_capacity(12);
    let scale = BLOCK_SCALE;

    // Bedrock cubes: origin is min corner, size is dimensions
    let from = scale_vec3(cube.origin, scale);
    let to = scale_vec3(
        [
            cube.origin[0] + cube.size[0],
            cube.origin[1] + cube.size[1],
            cube.origin[2] + cube.size[2],
        ],
        scale,
    );

    let vertices = compute_cube_vertices(from, to);

    // Apply cube's own rotation (if any)
    let mut vertices = if let (Some(rotation), Some(pivot)) = (&cube.rotation, &cube.pivot) {
        let angles = *rotation;
        if angles[0].abs() > 0.001 || angles[1].abs() > 0.001 || angles[2].abs() > 0.001 {
            let transform = RotationTransform::with_order(
                scale_vec3(*pivot, scale),
                angles,
                RotationOrder::ZYX,
            );
            rotate_vertices(&vertices, &transform)
        } else {
            vertices
        }
    } else if let Some(rotation) = &cube.rotation {
        // Rotation without explicit pivot — use cube center
        let angles = *rotation;
        if angles[0].abs() > 0.001 || angles[1].abs() > 0.001 || angles[2].abs() > 0.001 {
            let center = [
                (from[0] + to[0]) / 2.0,
                (from[1] + to[1]) / 2.0,
                (from[2] + to[2]) / 2.0,
            ];
            let transform = RotationTransform::with_order(center, angles, RotationOrder::ZYX);
            rotate_vertices(&vertices, &transform)
        } else {
            vertices
        }
    } else {
        vertices
    };

    // Apply bone rotation chain (bone, parent, grandparent, ..., root)
    for transform in bone_chain {
        let scaled = RotationTransform::with_order(
            scale_vec3(transform.origin, scale),
            transform.angles,
            transform.order,
        );
        vertices = rotate_vertices(&vertices, &scaled);
    }

    // Parse per-face UVs
    let per_face = parse_per_face_uv(&cube.uv);

    let default_color = [0.85, 0.85, 0.85];

    for (indices, face_name) in BEDROCK_FACE_DEFS {
        let face_uv = match face_name {
            FaceName::North => per_face.as_ref().and_then(|f| f.north.as_ref()),
            FaceName::South => per_face.as_ref().and_then(|f| f.south.as_ref()),
            FaceName::East => per_face.as_ref().and_then(|f| f.east.as_ref()),
            FaceName::West => per_face.as_ref().and_then(|f| f.west.as_ref()),
            FaceName::Up => per_face.as_ref().and_then(|f| f.up.as_ref()),
            FaceName::Down => per_face.as_ref().and_then(|f| f.down.as_ref()),
        };

        // Skip faces without UV data
        let uvs = if let Some(fuv) = face_uv {
            let u1 = fuv.uv[0] / tex_width;
            let v1 = fuv.uv[1] / tex_height;
            let u2 = (fuv.uv[0] + fuv.uv_size[0]) / tex_width;
            let v2 = (fuv.uv[1] + fuv.uv_size[1]) / tex_height;

            let corners = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];
            apply_uv_rotation(corners, fuv.uv_rotation.unwrap_or(0.0))
        } else if per_face.is_some() {
            // Per-face UV mode but this face has no UV — skip it
            continue;
        } else {
            DEFAULT_UVS
        };

        let tris = quad_to_triangles(&vertices, indices, uvs, default_color, None);
        triangles.extend(tris);
    }

    triangles
}

fn parse_per_face_uv(uv_value: &serde_json::Value) -> Option<BedrockPerFaceUv> {
    if uv_value.is_object() {
        serde_json::from_value(uv_value.clone()).ok()
    } else {
        None
    }
}

/// Rotates all triangles 180 degrees around the Y axis through their collective center.
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

    for tri in triangles.iter_mut() {
        for v in &mut tri.verts {
            v[0] = 2.0 * center.x - v[0];
            v[2] = 2.0 * center.z - v[2];
        }
    }
}
