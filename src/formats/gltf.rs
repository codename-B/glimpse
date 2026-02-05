//! Provides a glTF/GLB format loader.
//!
//! Supports both binary GLB and JSON glTF files with embedded or external resources.
//!
//! # Examples
//! ```
//! use glimpse::formats::{self, FormatLoader};
//!
//! let loader = formats::gltf::GltfLoader;
//! assert!(loader.extensions().contains(&"gltf"));
//! ```

use std::path::Path;
use std::sync::Arc;

use super::{
    FormatLoader, LoadError, LoadResult, Mat4, ModelData, TextureData, Triangle, Vec2, Vec3,
};

/// The glTF format loader.
///
/// # Examples
/// ```
/// use glimpse::formats::{self, FormatLoader};
///
/// let loader = formats::gltf::GltfLoader;
/// assert_eq!(loader.name(), "glTF");
/// ```
pub struct GltfLoader;

impl FormatLoader for GltfLoader {
    fn name(&self) -> &'static str {
        "glTF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gltf", "glb"]
    }

    fn can_load(&self, data: &[u8], extension: Option<&str>) -> bool {
        // Check extension first
        if let Some(ext) = extension {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "gltf" || ext_lower == "glb" {
                return true;
            }
        }

        // Check GLB magic bytes
        if data.len() >= 4 && &data[0..4] == b"glTF" {
            return true;
        }

        // Check for JSON glTF structure
        if data.len() > 10 {
            // Quick check for glTF JSON structure
            let start = String::from_utf8_lossy(&data[..data.len().min(1000)]);
            if start.contains("\"asset\"")
                && (start.contains("\"scene\"") || start.contains("\"scenes\""))
            {
                return true;
            }
        }

        false
    }

    fn load_from_bytes(&self, data: &[u8]) -> LoadResult {
        // Try the standard import first (works for GLB and fully-embedded glTF)
        if let Ok((document, buffers, images)) = gltf::import_slice(data) {
            return load_from_gltf(document, buffers, images);
        }

        // Fall back to lenient parsing for JSON glTF with external references
        let gltf_data = gltf::Gltf::from_slice(data)
            .map_err(|e| LoadError::InvalidData(format!("Failed to parse glTF: {}", e)))?;
        let document = gltf_data.document;

        // Try to load embedded buffers (data URIs)
        let mut buffers: Vec<gltf::buffer::Data> = Vec::new();
        for buffer in document.buffers() {
            match buffer.source() {
                gltf::buffer::Source::Bin => {
                    if let Some(blob) = gltf_data.blob.as_ref() {
                        buffers.push(gltf::buffer::Data(blob.clone()));
                    }
                }
                gltf::buffer::Source::Uri(uri) => {
                    if let Some(data) = decode_data_uri(uri) {
                        buffers.push(gltf::buffer::Data(data));
                    } else {
                        buffers.push(gltf::buffer::Data(Vec::new()));
                    }
                }
            }
        }

        // Try to load embedded images (data URIs)
        let mut images: Vec<gltf::image::Data> = Vec::new();
        for image in document.images() {
            match image.source() {
                gltf::image::Source::View { view, mime_type: _ } => {
                    let buffer_index = view.buffer().index();
                    if buffer_index < buffers.len() && !buffers[buffer_index].0.is_empty() {
                        let start = view.offset();
                        let end = start + view.length();
                        if end <= buffers[buffer_index].0.len() {
                            let img_data = &buffers[buffer_index].0[start..end];
                            if let Some(img) = decode_image_data(img_data) {
                                images.push(img);
                                continue;
                            }
                        }
                    }
                }
                gltf::image::Source::Uri { uri, mime_type: _ } => {
                    if let Some(data) = decode_data_uri(uri) {
                        if let Some(img) = decode_image_data(&data) {
                            images.push(img);
                            continue;
                        }
                    }
                }
            }
        }

        load_from_gltf(document, buffers, images)
    }

    fn load_from_path(&self, path: &Path) -> LoadResult {
        let (document, buffers, images) = gltf::import(path)
            .map_err(|e| LoadError::InvalidData(format!("Failed to import glTF: {}", e)))?;
        load_from_gltf(document, buffers, images)
    }
}

const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Loads triangles from a parsed glTF document.
fn load_from_gltf(
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
) -> LoadResult {
    // Load textures
    let textures: Vec<Option<Arc<TextureData>>> = document
        .textures()
        .map(|tex| {
            let source = tex.source();
            let img_index = source.index();
            if img_index < images.len() {
                let img = &images[img_index];
                let rgba_pixels = convert_to_rgba(&img.pixels, img.format);
                Some(Arc::new(TextureData {
                    width: img.width,
                    height: img.height,
                    data: rgba_pixels,
                }))
            } else {
                None
            }
        })
        .collect();

    // Extract triangles from scene hierarchy
    let mut triangles = Vec::new();

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or(LoadError::NoGeometry)?;

    for node in scene.nodes() {
        extract_node_triangles(&node, &buffers, &textures, &mut triangles, IDENTITY);
    }

    if triangles.is_empty() {
        return Err(LoadError::NoGeometry);
    }

    Ok(ModelData { triangles })
}

/// Recursively walks the glTF scene graph and collects world-space triangles.
fn extract_node_triangles(
    node: &gltf::Node,
    buffers: &[gltf::buffer::Data],
    textures: &[Option<Arc<TextureData>>],
    triangles: &mut Vec<Triangle>,
    parent_transform: Mat4,
) {
    let local: Mat4 = node.transform().matrix();
    let world = mat4_mul(parent_transform, local);

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }

            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| &*d.0));

            let positions: Vec<Vec3> = match reader.read_positions() {
                Some(iter) => iter.collect(),
                None => continue,
            };

            // Read UV coordinates (TEXCOORD_0)
            let uvs: Vec<Vec2> = reader
                .read_tex_coords(0)
                .map(|iter| iter.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            // Get material properties
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            let base_factor = pbr.base_color_factor();
            let material_color = [base_factor[0], base_factor[1], base_factor[2]];

            // Get base color texture if present
            let texture = pbr.base_color_texture().and_then(|info| {
                let tex_index = info.texture().index();
                textures.get(tex_index).and_then(|t| t.clone())
            });

            // Read vertex colors if available
            let vertex_colors: Option<Vec<[f32; 4]>> = reader
                .read_colors(0)
                .map(|iter| iter.into_rgba_f32().collect());

            // Index buffer
            let indices: Vec<u32> = match reader.read_indices() {
                Some(iter) => iter.into_u32().collect(),
                None => (0..positions.len() as u32).collect(),
            };

            for tri_indices in indices.chunks_exact(3) {
                let i0 = tri_indices[0] as usize;
                let i1 = tri_indices[1] as usize;
                let i2 = tri_indices[2] as usize;

                if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                    continue;
                }

                let v0 = transform_point(world, positions[i0]);
                let v1 = transform_point(world, positions[i1]);
                let v2 = transform_point(world, positions[i2]);

                let uv0 = if i0 < uvs.len() { uvs[i0] } else { [0.0, 0.0] };
                let uv1 = if i1 < uvs.len() { uvs[i1] } else { [0.0, 0.0] };
                let uv2 = if i2 < uvs.len() { uvs[i2] } else { [0.0, 0.0] };

                // Combine vertex colors with material color
                let color = if let Some(ref vc) = vertex_colors {
                    let c0 = if i0 < vc.len() {
                        vc[i0]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };
                    let c1 = if i1 < vc.len() {
                        vc[i1]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };
                    let c2 = if i2 < vc.len() {
                        vc[i2]
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };
                    [
                        ((c0[0] + c1[0] + c2[0]) / 3.0) * material_color[0],
                        ((c0[1] + c1[1] + c2[1]) / 3.0) * material_color[1],
                        ((c0[2] + c1[2] + c2[2]) / 3.0) * material_color[2],
                    ]
                } else {
                    material_color
                };

                triangles.push(Triangle {
                    verts: [v0, v1, v2],
                    uvs: [uv0, uv1, uv2],
                    color,
                    texture: texture.clone(),
                });
            }
        }
    }

    for child in node.children() {
        extract_node_triangles(&child, buffers, textures, triangles, world);
    }
}

/// Decodes a data: URI to raw bytes.
fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    if !uri.starts_with("data:") {
        return None;
    }

    let comma_pos = uri.find(',')?;
    let encoded = &uri[(comma_pos + 1)..];

    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()
}

/// Decodes image data (PNG, JPEG, etc.) to RGBA pixels.
fn decode_image_data(data: &[u8]) -> Option<gltf::image::Data> {
    use image::GenericImageView;

    let img = image::load_from_memory(data).ok()?;
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();

    Some(gltf::image::Data {
        width,
        height,
        format: gltf::image::Format::R8G8B8A8,
        pixels: rgba.into_raw(),
    })
}

/// Converts pixel data to RGBA format if needed.
fn convert_to_rgba(pixels: &[u8], format: gltf::image::Format) -> Vec<u8> {
    use gltf::image::Format;
    match format {
        Format::R8G8B8A8 => pixels.to_vec(),
        Format::R8G8B8 => {
            let mut rgba = Vec::with_capacity(pixels.len() / 3 * 4);
            for chunk in pixels.chunks_exact(3) {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(chunk[2]);
                rgba.push(255);
            }
            rgba
        }
        Format::R8 => {
            let mut rgba = Vec::with_capacity(pixels.len() * 4);
            for &gray in pixels {
                rgba.push(gray);
                rgba.push(gray);
                rgba.push(gray);
                rgba.push(255);
            }
            rgba
        }
        Format::R8G8 => {
            let mut rgba = Vec::with_capacity(pixels.len() * 2);
            for chunk in pixels.chunks_exact(2) {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(0);
                rgba.push(255);
            }
            rgba
        }
        Format::R16 | Format::R16G16 | Format::R16G16B16 | Format::R16G16B16A16 => {
            vec![255u8; (pixels.len() / 2) * 4]
        }
        Format::R32G32B32FLOAT | Format::R32G32B32A32FLOAT => {
            vec![
                255u8;
                (pixels.len()
                    / if format == Format::R32G32B32FLOAT {
                        12
                    } else {
                        16
                    })
                    * 4
            ]
        }
    }
}

// Linear algebra helpers

fn transform_point(m: Mat4, p: Vec3) -> Vec3 {
    let x = m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0];
    let y = m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1];
    let z = m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2];
    let w = m[0][3] * p[0] + m[1][3] * p[1] + m[2][3] * p[2] + m[3][3];
    if w.abs() < 1e-10 {
        [x, y, z]
    } else {
        [x / w, y / w, z / w]
    }
}

fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut r = [[0.0_f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            r[col][row] = a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3];
        }
    }
    r
}
