//! Provides a Wavefront OBJ format loader.
//!
//! OBJ is a widely supported 3D model format. This loader handles geometry
//! (vertices and faces) with automatic polygon triangulation. When loaded
//! from a file path, companion .mtl materials are resolved for diffuse
//! colors and textures.

use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::sync::Arc;

use obj::raw::material::{parse_mtl, MtlColor};
use obj::raw::object::Polygon;
use obj::raw::parse_obj;

use super::shared::texture::load_texture_from_file;
use super::{FormatLoader, LoadError, LoadResult, ModelData, TextureData, Triangle};

pub struct ObjLoader;

impl FormatLoader for ObjLoader {
    fn name(&self) -> &'static str {
        "Wavefront OBJ"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["obj"]
    }

    fn can_load(&self, data: &[u8], extension: Option<&str>) -> bool {
        if let Some(ext) = extension {
            if ext.to_lowercase() == "obj" {
                return true;
            }
        }

        // Content detection: look for OBJ vertex/face lines
        if let Ok(text) = std::str::from_utf8(data) {
            let sample = &text[..text.len().min(4000)];
            let mut has_vertex = false;
            let mut has_face = false;
            for line in sample.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("v ") {
                    has_vertex = true;
                }
                if trimmed.starts_with("f ") {
                    has_face = true;
                }
                if has_vertex && has_face {
                    return true;
                }
            }
        }

        false
    }

    fn load_from_bytes(&self, data: &[u8]) -> LoadResult {
        let reader = BufReader::new(Cursor::new(data));
        let raw = parse_obj(reader)
            .map_err(|e| LoadError::InvalidData(format!("Failed to parse OBJ: {}", e)))?;

        convert_raw_obj_to_triangles(&raw, &HashMap::new())
    }

    fn load_from_path(&self, path: &Path) -> LoadResult {
        let data = std::fs::read(path)?;
        let reader = BufReader::new(Cursor::new(&data[..]));
        let raw = parse_obj(reader)
            .map_err(|e| LoadError::InvalidData(format!("Failed to parse OBJ: {}", e)))?;

        // Load companion .mtl files
        let obj_dir = path.parent().unwrap_or(Path::new("."));
        let materials = load_mtl_materials(&raw.material_libraries, obj_dir);

        convert_raw_obj_to_triangles(&raw, &materials)
    }
}

/// Loaded material data from .mtl file.
struct ObjMaterial {
    color: [f32; 3],
    texture: Option<Arc<TextureData>>,
}

/// Loads materials from .mtl files referenced by the OBJ.
fn load_mtl_materials(mtl_libs: &[String], obj_dir: &Path) -> HashMap<String, ObjMaterial> {
    let mut materials = HashMap::new();

    for mtl_name in mtl_libs {
        let mtl_path = obj_dir.join(mtl_name);
        let mtl_data = match std::fs::read(&mtl_path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let reader = BufReader::new(Cursor::new(&mtl_data[..]));
        let raw_mtl = match parse_mtl(reader) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let mtl_dir = mtl_path.parent().unwrap_or(obj_dir);

        for (name, mat) in &raw_mtl.materials {
            let color = mat
                .diffuse
                .as_ref()
                .map(mtl_color_to_rgb)
                .unwrap_or([0.85, 0.85, 0.85]);

            let texture = mat.diffuse_map.as_ref().and_then(|map| {
                let tex_path = mtl_dir.join(&map.file);
                load_texture_from_file(&tex_path)
            });

            materials.insert(name.clone(), ObjMaterial { color, texture });
        }
    }

    materials
}

fn mtl_color_to_rgb(color: &MtlColor) -> [f32; 3] {
    match color {
        MtlColor::Rgb(r, g, b) => [*r, *g, *b],
        MtlColor::Xyz(x, y, z) => [*x, *y, *z],
        MtlColor::Spectral(_, _) => [0.85, 0.85, 0.85],
    }
}

/// Extracts position index at a given slot from any polygon variant.
fn polygon_pos_at(polygon: &Polygon, i: usize) -> Option<usize> {
    match polygon {
        Polygon::P(indices) => indices.get(i).copied(),
        Polygon::PT(pairs) => pairs.get(i).map(|&(p, _)| p),
        Polygon::PN(pairs) => pairs.get(i).map(|&(p, _)| p),
        Polygon::PTN(triples) => triples.get(i).map(|&(p, _, _)| p),
    }
}

/// Returns the number of vertices in a polygon.
fn polygon_len(polygon: &Polygon) -> usize {
    match polygon {
        Polygon::P(indices) => indices.len(),
        Polygon::PT(pairs) => pairs.len(),
        Polygon::PN(pairs) => pairs.len(),
        Polygon::PTN(triples) => triples.len(),
    }
}

/// Extracts texture coordinate index at a given slot (if available).
fn polygon_tex_at(polygon: &Polygon, i: usize) -> Option<usize> {
    match polygon {
        Polygon::P(_) | Polygon::PN(_) => None,
        Polygon::PT(pairs) => pairs.get(i).map(|&(_, t)| t),
        Polygon::PTN(triples) => triples.get(i).map(|&(_, t, _)| t),
    }
}

fn convert_raw_obj_to_triangles(
    raw: &obj::raw::object::RawObj,
    materials: &HashMap<String, ObjMaterial>,
) -> LoadResult {
    let mut triangles = Vec::new();
    let default_color = [0.85, 0.85, 0.85];
    let default_uv = [0.0, 0.0];

    let positions = &raw.positions;
    let tex_coords = &raw.tex_coords;

    // Build polygon index → material name mapping from meshes
    let mut polygon_material: Vec<Option<&str>> = vec![None; raw.polygons.len()];
    for (mat_name, group) in &raw.meshes {
        for range in &group.polygons {
            for i in range.start..range.end {
                if i < polygon_material.len() {
                    polygon_material[i] = Some(mat_name.as_str());
                }
            }
        }
    }

    for (poly_idx, polygon) in raw.polygons.iter().enumerate() {
        let n = polygon_len(polygon);
        if n < 3 {
            continue;
        }

        // Look up material for this polygon
        let mat = polygon_material
            .get(poly_idx)
            .and_then(|name| name.and_then(|n| materials.get(n)));

        let color = mat.map(|m| m.color).unwrap_or(default_color);
        let texture = mat.and_then(|m| m.texture.clone());

        // Fan triangulation
        let p0 = match polygon_pos_at(polygon, 0) {
            Some(idx) if idx < positions.len() => idx,
            _ => continue,
        };
        let v0 = [positions[p0].0, positions[p0].1, positions[p0].2];
        let uv0 = polygon_tex_at(polygon, 0)
            .filter(|&idx| idx < tex_coords.len())
            .map(|idx| [tex_coords[idx].0, tex_coords[idx].1])
            .unwrap_or(default_uv);

        for i in 1..n - 1 {
            let p1 = match polygon_pos_at(polygon, i) {
                Some(idx) if idx < positions.len() => idx,
                _ => continue,
            };
            let p2 = match polygon_pos_at(polygon, i + 1) {
                Some(idx) if idx < positions.len() => idx,
                _ => continue,
            };

            let v1 = [positions[p1].0, positions[p1].1, positions[p1].2];
            let v2 = [positions[p2].0, positions[p2].1, positions[p2].2];

            let uv1 = polygon_tex_at(polygon, i)
                .filter(|&idx| idx < tex_coords.len())
                .map(|idx| [tex_coords[idx].0, tex_coords[idx].1])
                .unwrap_or(default_uv);
            let uv2 = polygon_tex_at(polygon, i + 1)
                .filter(|&idx| idx < tex_coords.len())
                .map(|idx| [tex_coords[idx].0, tex_coords[idx].1])
                .unwrap_or(default_uv);

            triangles.push(Triangle {
                verts: [v0, v1, v2],
                uvs: [uv0, uv1, uv2],
                color,
                texture: texture.clone(),
            });
        }
    }

    if triangles.is_empty() {
        return Err(LoadError::NoGeometry);
    }

    Ok(ModelData { triangles })
}
