//! Provides a software rasterizer for 3D model thumbnails.
//!
//! This module handles format-agnostic rasterization: taking triangles
//! and converting them to pixels using a simple perspective camera,
//! flat shading, and z-buffer.
//!
//! No GPU is required; it runs entirely on the CPU.
//!
//! # Examples
//! ```no_run
//! use glimpse::renderer;
//!
//! let pixels = renderer::render_thumbnail(b"", None, 64, 64);
//! assert!(pixels.is_none());
//! ```

use std::path::Path;

use glam::{Mat4, Vec3, Vec4};

use crate::formats::{self, ModelData, Triangle};

/// Renders a model from raw bytes into an RGBA pixel buffer.
/// Auto-detects the format based on content and extension.
///
/// # Examples
/// ```
/// use glimpse::renderer::render_thumbnail;
///
/// let pixels = render_thumbnail(b"not a model", None, 64, 64);
/// assert!(pixels.is_none());
/// ```
pub fn render_thumbnail(
    data: &[u8],
    extension: Option<&str>,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let model = formats::load_model(data, extension).ok()?;
    render_model_data(model, width, height)
}

/// Renders a model from a file path into an RGBA pixel buffer.
/// Auto-detects the format based on content and extension.
///
/// # Examples
/// ```
/// use std::path::Path;
///
/// use glimpse::renderer::render_thumbnail_from_path;
///
/// let pixels = render_thumbnail_from_path(Path::new("does_not_exist.gltf"), 64, 64);
/// assert!(pixels.is_none());
/// ```
pub fn render_thumbnail_from_path(path: &Path, width: u32, height: u32) -> Option<Vec<u8>> {
    let model = formats::load_model_from_path(path).ok()?;
    render_model_data(model, width, height)
}

/// Renders a glTF/GLB model from raw bytes into an RGBA pixel buffer.
/// This is a convenience wrapper that maintains backwards compatibility.
///
/// # Examples
/// ```
/// use glimpse::renderer::render_gltf_thumbnail;
///
/// let pixels = render_gltf_thumbnail(b"invalid", 64, 64);
/// assert!(pixels.is_none());
/// ```
pub fn render_gltf_thumbnail(data: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    render_thumbnail(data, Some("glb"), width, height)
}

/// Renders a glTF/GLB model from a file path into an RGBA pixel buffer.
/// This is a convenience wrapper that maintains backwards compatibility.
///
/// # Examples
/// ```
/// use std::path::Path;
///
/// use glimpse::renderer::render_gltf_thumbnail_from_path;
///
/// let pixels = render_gltf_thumbnail_from_path(Path::new("does_not_exist.glb"), 64, 64);
/// assert!(pixels.is_none());
/// ```
pub fn render_gltf_thumbnail_from_path(path: &Path, width: u32, height: u32) -> Option<Vec<u8>> {
    render_thumbnail_from_path(path, width, height)
}

/// Renders loaded model data to pixels.
fn render_model_data(model: ModelData, width: u32, height: u32) -> Option<Vec<u8>> {
    let triangles = model.triangles;

    if triangles.is_empty() {
        return None;
    }

    // ---- Compute bounding sphere ----
    let (bb_min, bb_max) = compute_bounds(&triangles);
    let center = bb_min.lerp(bb_max, 0.5);
    let extent = bb_max - bb_min;
    let radius = extent.length() * 0.5;

    if radius < 1e-6 {
        return None;
    }

    // ---- Camera ----
    // Azimuth rotated 180° so models face the camera instead of away
    let azimuth: f32 = (35.0 + 180.0_f32).to_radians();
    let elevation: f32 = 25.0_f32.to_radians();
    let dist = radius * 2.8;

    let eye = Vec3::new(
        center.x + dist * elevation.cos() * azimuth.sin(),
        center.y + dist * elevation.sin(),
        center.z + dist * elevation.cos() * azimuth.cos(),
    );

    let view = Mat4::look_at_rh(eye, center, Vec3::Y);
    let aspect = width as f32 / height as f32;
    let near = radius * 0.01;
    let far = radius * 100.0;
    let proj = Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect, near, far);
    let view_proj = proj * view;

    // ---- Framebuffer ----
    let w = width as usize;
    let h = height as usize;
    let mut color_buf = vec![[0.0_f32; 4]; w * h];
    let mut depth_buf = vec![f32::INFINITY; w * h];

    // ---- Lighting ----
    let light_dir = Vec3::new(0.5, 0.8, 0.3).normalize();
    let light2_dir = Vec3::new(-0.3, 0.2, -0.5).normalize();

    // ---- Rasterize each triangle ----
    for tri in &triangles {
        let mut clip = [Vec4::ZERO; 3];
        let mut screen = [Vec3::ZERO; 3];
        let mut visible = true;

        for i in 0..3 {
            let v = Vec3::from_array(tri.verts[i]);
            clip[i] = view_proj * v.extend(1.0);

            if clip[i].w <= 0.0 {
                visible = false;
                break;
            }

            let inv_w = 1.0 / clip[i].w;
            screen[i] = Vec3::new(
                (clip[i].x * inv_w * 0.5 + 0.5) * width as f32,
                (0.5 - clip[i].y * inv_w * 0.5) * height as f32,
                clip[i].z * inv_w,
            );
        }

        if !visible {
            continue;
        }

        // Face normal in world space (flat shading)
        let v0 = Vec3::from_array(tri.verts[0]);
        let v1 = Vec3::from_array(tri.verts[1]);
        let v2 = Vec3::from_array(tri.verts[2]);
        let e1 = v1 - v0;
        let e2 = v2 - v0;
        let normal = e1.cross(e2).normalize();

        let ndl_main = normal.dot(light_dir).abs();
        let ndl_fill = normal.dot(light2_dir).abs();

        let ambient = 0.15;
        let diffuse = ndl_main * 0.60 + ndl_fill * 0.15;
        let specular = ndl_main.powf(32.0) * 0.10;
        let shade = (ambient + diffuse + specular).min(1.0);

        // Screen-space bounding box
        let min_x = screen[0].x.min(screen[1].x).min(screen[2].x).max(0.0) as usize;
        let max_x = (screen[0].x.max(screen[1].x).max(screen[2].x).ceil() as usize).min(w);
        let min_y = screen[0].y.min(screen[1].y).min(screen[2].y).max(0.0) as usize;
        let max_y = (screen[0].y.max(screen[1].y).max(screen[2].y).ceil() as usize).min(h);

        // Rasterize
        for y in min_y..max_y {
            for x in min_x..max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let (u_bary, v_bary, w_bary) = barycentric(screen, px, py);

                if u_bary >= 0.0 && v_bary >= 0.0 && w_bary >= 0.0 {
                    let z = u_bary * screen[0].z + v_bary * screen[1].z + w_bary * screen[2].z;
                    let idx = y * w + x;

                    if z < depth_buf[idx] {
                        depth_buf[idx] = z;

                        // Interpolate UVs using barycentric coordinates
                        let tex_u = u_bary * tri.uvs[0][0]
                            + v_bary * tri.uvs[1][0]
                            + w_bary * tri.uvs[2][0];
                        let tex_v = u_bary * tri.uvs[0][1]
                            + v_bary * tri.uvs[1][1]
                            + w_bary * tri.uvs[2][1];

                        // Sample texture if available, otherwise use base color
                        let (base, alpha) = if let Some(ref tex) = tri.texture {
                            let sampled = tex.sample(tex_u, tex_v);
                            (
                                [
                                    sampled[0] * tri.color[0],
                                    sampled[1] * tri.color[1],
                                    sampled[2] * tri.color[2],
                                ],
                                sampled[3],
                            )
                        } else {
                            (tri.color, 1.0)
                        };

                        // Alpha cutoff - skip fully transparent pixels
                        if alpha < 0.5 {
                            continue;
                        }

                        let shaded = [
                            (base[0] * shade).min(1.0),
                            (base[1] * shade).min(1.0),
                            (base[2] * shade).min(1.0),
                        ];

                        color_buf[idx] = [shaded[0], shaded[1], shaded[2], 1.0];
                    }
                }
            }
        }
    }

    // ---- Convert f32 → u8 RGBA ----
    let mut pixels = vec![0u8; w * h * 4];
    for i in 0..w * h {
        pixels[i * 4] = (color_buf[i][0].clamp(0.0, 1.0) * 255.0) as u8;
        pixels[i * 4 + 1] = (color_buf[i][1].clamp(0.0, 1.0) * 255.0) as u8;
        pixels[i * 4 + 2] = (color_buf[i][2].clamp(0.0, 1.0) * 255.0) as u8;
        pixels[i * 4 + 3] = (color_buf[i][3].clamp(0.0, 1.0) * 255.0) as u8;
    }

    Some(pixels)
}

/// Computes the axis-aligned bounding box of all triangle vertices.
fn compute_bounds(triangles: &[Triangle]) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for tri in triangles {
        for v in &tri.verts {
            let p = Vec3::from_array(*v);
            min = min.min(p);
            max = max.max(p);
        }
    }
    (min, max)
}

// ===========================================================================
// Rasterization helpers
// ===========================================================================

fn barycentric(tri: [Vec3; 3], px: f32, py: f32) -> (f32, f32, f32) {
    let v0x = tri[1].x - tri[0].x;
    let v0y = tri[1].y - tri[0].y;
    let v1x = tri[2].x - tri[0].x;
    let v1y = tri[2].y - tri[0].y;
    let v2x = px - tri[0].x;
    let v2y = py - tri[0].y;

    let d00 = v0x * v0x + v0y * v0y;
    let d01 = v0x * v1x + v0y * v1y;
    let d11 = v1x * v1x + v1y * v1y;
    let d20 = v2x * v0x + v2y * v0y;
    let d21 = v2x * v1x + v2y * v1y;

    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-10 {
        return (-1.0, -1.0, -1.0);
    }

    let inv = 1.0 / denom;
    let v = (d11 * d20 - d01 * d21) * inv;
    let w = (d00 * d21 - d01 * d20) * inv;
    let u = 1.0 - v - w;

    (u, v, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty_data() {
        let empty = vec![];
        assert!(render_gltf_thumbnail(&empty, 256, 256).is_none());
    }

    #[test]
    fn test_render_invalid_data() {
        let invalid = b"not gltf data";
        assert!(render_gltf_thumbnail(invalid, 256, 256).is_none());
    }

    #[test]
    fn test_render_output_dimensions() {
        let width = 128;
        let height = 128;
        let result = render_gltf_thumbnail(b"invalid", width, height);
        if let Some(pixels) = result {
            assert_eq!(pixels.len(), (width * height * 4) as usize);
        }
    }
}
