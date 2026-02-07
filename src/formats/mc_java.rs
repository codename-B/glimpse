//! Provides a Minecraft Java Edition block/item model format loader.
//!
//! Java Edition models use elements with from/to cube bounds and single-axis
//! rotation. They are identified by the `"parent"` field (e.g. `"block/block"`).
//! Textures are external files, so models render with solid color.

use std::path::Path;

use serde::Deserialize;

use super::shared::cube::{
    apply_uv_rotation, compute_cube_vertices, quad_to_triangles, scale_vec3, BLOCK_SCALE,
    DEFAULT_UVS,
};
use super::shared::rotation::{rotate_vertices, RotationOrder, RotationTransform};
use super::{FormatLoader, LoadError, LoadResult, ModelData, Triangle};

pub struct McJavaLoader;

impl FormatLoader for McJavaLoader {
    fn name(&self) -> &'static str {
        "Minecraft Java"
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

            // MC Java models are identified by "parent" field or "texture_size" (underscore)
            let has_mc_marker =
                sample.contains("\"parent\"") || sample.contains("\"texture_size\"");
            let has_elements = sample.contains("\"elements\"");

            return has_mc_marker && has_elements;
        }

        false
    }

    fn load_from_bytes(&self, data: &[u8]) -> LoadResult {
        let text = std::str::from_utf8(data).map_err(|_| {
            LoadError::InvalidData("Invalid UTF-8 in MC Java model file".to_string())
        })?;

        let model: JavaModel = serde_json::from_str(text)
            .map_err(|e| LoadError::InvalidData(format!("Failed to parse MC Java model: {}", e)))?;

        convert_java_model_to_triangles(model)
    }

    fn load_from_path(&self, path: &Path) -> LoadResult {
        let data = std::fs::read(path)?;
        self.load_from_bytes(&data)
    }
}

// ---- MC Java JSON structure ----

#[derive(Deserialize)]
struct JavaModel {
    #[serde(default)]
    texture_size: Option<[f32; 2]>,
    #[serde(default)]
    elements: Vec<JavaElement>,
}

#[derive(Deserialize)]
struct JavaElement {
    #[serde(default)]
    from: [f32; 3],
    #[serde(default)]
    to: [f32; 3],
    #[serde(default)]
    rotation: Option<JavaRotation>,
    #[serde(default)]
    faces: JavaFaces,
}

#[derive(Deserialize)]
struct JavaRotation {
    #[serde(default)]
    angle: f32,
    #[serde(default)]
    axis: String,
    #[serde(default)]
    origin: Option<[f32; 3]>,
}

#[derive(Deserialize, Default)]
struct JavaFaces {
    north: Option<JavaFace>,
    south: Option<JavaFace>,
    east: Option<JavaFace>,
    west: Option<JavaFace>,
    up: Option<JavaFace>,
    down: Option<JavaFace>,
}

#[derive(Deserialize)]
struct JavaFace {
    #[serde(default)]
    uv: Option<[f32; 4]>,
    #[serde(default)]
    texture: Option<String>,
    #[serde(default)]
    rotation: Option<f32>,
}

/// MC Java face vertex indices (same winding as Blockbench java_block).
const JAVA_FACE_INDICES: [([usize; 4], FaceSlot); 6] = [
    ([2, 3, 0, 1], FaceSlot::North),
    ([7, 6, 5, 4], FaceSlot::South),
    ([6, 2, 1, 5], FaceSlot::East),
    ([3, 7, 4, 0], FaceSlot::West),
    ([3, 2, 6, 7], FaceSlot::Up),
    ([4, 5, 1, 0], FaceSlot::Down),
];

#[derive(Clone, Copy)]
enum FaceSlot {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

fn convert_java_model_to_triangles(model: JavaModel) -> LoadResult {
    let mut triangles = Vec::new();

    // UV space size from texture_size or default 16x16
    let (tex_width, tex_height) = model
        .texture_size
        .map(|ts| (ts[0], ts[1]))
        .unwrap_or((16.0, 16.0));

    for element in &model.elements {
        let cube_tris = convert_java_cube(element, tex_width, tex_height);
        triangles.extend(cube_tris);
    }

    if triangles.is_empty() {
        return Err(LoadError::NoGeometry);
    }

    rotate_triangles_y_180(&mut triangles);

    Ok(ModelData { triangles })
}

fn convert_java_cube(element: &JavaElement, tex_width: f32, tex_height: f32) -> Vec<Triangle> {
    let mut triangles = Vec::with_capacity(12);
    let scale = BLOCK_SCALE;

    let from = scale_vec3(element.from, scale);
    let to = scale_vec3(element.to, scale);

    let vertices = compute_cube_vertices(from, to);

    // Apply element rotation (single-axis in MC Java)
    let vertices = if let Some(ref rot) = element.rotation {
        let angles = match rot.axis.as_str() {
            "x" => [rot.angle, 0.0, 0.0],
            "y" => [0.0, rot.angle, 0.0],
            "z" => [0.0, 0.0, rot.angle],
            _ => [0.0, rot.angle, 0.0],
        };

        if angles[0].abs() > 0.001 || angles[1].abs() > 0.001 || angles[2].abs() > 0.001 {
            let origin = rot.origin.map(|o| scale_vec3(o, scale)).unwrap_or([0.0; 3]);
            let transform = RotationTransform::with_order(origin, angles, RotationOrder::XYZ);
            rotate_vertices(&vertices, &transform)
        } else {
            vertices
        }
    } else {
        vertices
    };

    let default_color = [0.85, 0.85, 0.85];

    for (indices, face_slot) in JAVA_FACE_INDICES {
        let face = match face_slot {
            FaceSlot::North => element.faces.north.as_ref(),
            FaceSlot::South => element.faces.south.as_ref(),
            FaceSlot::East => element.faces.east.as_ref(),
            FaceSlot::West => element.faces.west.as_ref(),
            FaceSlot::Up => element.faces.up.as_ref(),
            FaceSlot::Down => element.faces.down.as_ref(),
        };

        let face = match face {
            Some(f) => f,
            None => continue,
        };

        // Skip faces without a texture reference
        if face.texture.is_none() {
            continue;
        }

        let uvs = if let Some(uv) = &face.uv {
            let u1 = uv[0] / tex_width;
            let v1 = uv[1] / tex_height;
            let u2 = uv[2] / tex_width;
            let v2 = uv[3] / tex_height;

            let corners = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];
            apply_uv_rotation(corners, face.rotation.unwrap_or(0.0))
        } else {
            DEFAULT_UVS
        };

        let tris = quad_to_triangles(&vertices, indices, uvs, default_color, None);
        triangles.extend(tris);
    }

    triangles
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
