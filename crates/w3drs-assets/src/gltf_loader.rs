use glam::{Mat3, Mat4, Vec3};
use thiserror::Error;

use crate::{
    material::{
        AlphaMode, Material, ShadingModel, TextureUvTransform, TEX_UV_ALBEDO, TEX_UV_ANISOTROPY,
        TEX_UV_CLEARCOAT, TEX_UV_CLEARCOAT_ROUGHNESS, TEX_UV_EMISSIVE, TEX_UV_METALLIC_ROUGHNESS,
        TEX_UV_NORMAL, TEX_UV_SPECULAR, TEX_UV_SPECULAR_COLOR, TEX_UV_THICKNESS,
        TEX_UV_TRANSMISSION,
    },
    mesh::Mesh,
    vertex::Vertex,
};

#[derive(Debug, Error)]
pub enum GltfError {
    #[error("gltf parse error: {0}")]
    Parse(#[from] gltf::Error),
    #[error("primitive missing POSITION attribute")]
    MissingPositions,
}

/// Decoded RGBA8 image ready for GPU upload.
pub struct RgbaImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// One glTF primitive — mesh geometry + material params + optional texture data.
pub struct GltfPrimitive {
    pub mesh: Mesh,
    pub material: Material,
    /// Base color / albedo (sRGB)
    pub albedo_image: Option<RgbaImage>,
    /// Tangent-space normal map (linear)
    pub normal_image: Option<RgbaImage>,
    /// Metallic (B) + roughness (G) per glTF spec (linear)
    pub metallic_roughness_image: Option<RgbaImage>,
    /// Emissive color (sRGB)
    pub emissive_image: Option<RgbaImage>,
    /// `KHR_materials_anisotropy` texture (linear RGB); default mapping when absent uses factor-only path.
    pub anisotropy_image: Option<RgbaImage>,
    /// `KHR_materials_clearcoat` — intensité (canal **R**).
    pub clearcoat_image: Option<RgbaImage>,
    /// `KHR_materials_clearcoat` — rugosité (canal **G**).
    pub clearcoat_roughness_image: Option<RgbaImage>,
    /// `KHR_materials_transmission` (canal **R**).
    pub transmission_image: Option<RgbaImage>,
    /// `KHR_materials_specular` (canal **A**).
    pub specular_image: Option<RgbaImage>,
    /// `KHR_materials_specular` teinte F0 sRGB.
    pub specular_color_image: Option<RgbaImage>,
    /// `KHR_materials_volume` épaisseur (canal **G**).
    pub thickness_image: Option<RgbaImage>,
}

/// Load all mesh primitives from a GLB/glTF byte slice.
///
/// Uses [`gltf::Gltf::from_slice_without_validation`] then charge buffers / images : certains assets
/// listent `KHR_materials_clearcoat` (ou d’autres extensions) dans `extensionsRequired`, alors que la
/// crate **gltf** 1.4.x ne les déclare pas dans son tableau interne `ENABLED_EXTENSIONS` — `import_slice`
/// échouerait à tort. La validation stricte des index / tailles reste en partie implicite à l’usage.
///
/// Parcourt la **scène par défaut** (ou la première scène) et fusionne la **matrice monde** de chaque
/// nœud dans les sommets (positions, normales, tangentes). Sans cela, les modèles dont l’orientation
/// repose sur une hiérarchie de nœuds (ex. DamagedHelmet) apparaissent « sur le côté ».
pub fn load_from_bytes(bytes: &[u8]) -> Result<Vec<GltfPrimitive>, GltfError> {
    let gltf = gltf::Gltf::from_slice_without_validation(bytes)?;
    let buffers = gltf::import_buffers(&gltf, None, gltf.blob.clone())?;
    let images = gltf::import_images(&gltf, None, &buffers)?;
    let mut result = Vec::new();

    let scene_opt = gltf.default_scene().or_else(|| gltf.scenes().next());
    if let Some(scene) = scene_opt {
        for root in scene.nodes() {
            visit_scene_node(&gltf, &buffers, &images, root, Mat4::IDENTITY, &mut result)?;
        }
    } else {
        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                push_primitive(
                    &gltf,
                    &buffers,
                    &images,
                    &primitive,
                    Mat4::IDENTITY,
                    &mut result,
                )?;
            }
        }
    }

    Ok(result)
}

fn mat4_from_gltf_transform(t: gltf::scene::Transform) -> Mat4 {
    Mat4::from_cols_array_2d(&t.matrix())
}

/// Applique la matrice monde du nœud : positions (point), normales (inverse-transpose), tangentes
/// (partie linéaire), puis recalcule la bitangente pour conserver l’orthogonalité et le signe w.
fn transform_vertices(vertices: &mut [Vertex], world: Mat4) {
    let linear = Mat3::from_mat4(world);
    let normal_mat = world.inverse().transpose();
    for v in vertices.iter_mut() {
        let p = Vec3::from_array(v.position);
        v.position = world.transform_point3(p).to_array();

        let n = Vec3::from_array(v.normal);
        let t = Vec3::from_array(v.tangent);
        let b = Vec3::from_array(v.bitangent);
        let handedness = if n.cross(t).dot(b) >= 0.0 {
            1.0f32
        } else {
            -1.0f32
        };

        let n_w = normal_mat.transform_vector3(n);
        let n_w = n_w.try_normalize().unwrap_or(Vec3::Y);

        let t_w = linear * t;
        let t_w = t_w.try_normalize().unwrap_or(Vec3::X);
        let t_orth = (t_w - n_w * t_w.dot(n_w)).try_normalize().unwrap_or(t_w);
        let b_w = n_w.cross(t_orth) * handedness;

        v.normal = n_w.to_array();
        v.tangent = t_orth.to_array();
        v.bitangent = b_w.to_array();
    }
}

fn visit_scene_node(
    document: &gltf::Gltf,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    node: gltf::Node<'_>,
    parent_world: Mat4,
    out: &mut Vec<GltfPrimitive>,
) -> Result<(), GltfError> {
    let local = mat4_from_gltf_transform(node.transform());
    let world = parent_world * local;

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            push_primitive(document, buffers, images, &primitive, world, out)?;
        }
    }

    for child in node.children() {
        visit_scene_node(document, buffers, images, child, world, out)?;
    }
    Ok(())
}

fn push_primitive(
    document: &gltf::Gltf,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    primitive: &gltf::Primitive<'_>,
    node_world: Mat4,
    out: &mut Vec<GltfPrimitive>,
) -> Result<(), GltfError> {
    let reader = primitive.reader(|buf| Some(&buffers[buf.index()]));

    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or(GltfError::MissingPositions)?
        .collect();

    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|it| it.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

    let uv0s: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|it| it.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let uv1s: Vec<[f32; 2]> = reader
        .read_tex_coords(1)
        .map(|it| it.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let tangents_raw: Vec<[f32; 4]> = reader
        .read_tangents()
        .map(|it| it.collect())
        .unwrap_or_default();

    let colors: Vec<[f32; 4]> = reader
        .read_colors(0)
        .map(|it| it.into_rgba_f32().collect())
        .unwrap_or_else(|| vec![[1.0, 1.0, 1.0, 1.0]; positions.len()]);

    let indices: Vec<u32> = reader
        .read_indices()
        .map(|it| it.into_u32().collect())
        .unwrap_or_else(|| (0..positions.len() as u32).collect());

    let mut vertices: Vec<Vertex> = positions
        .iter()
        .enumerate()
        .map(|(i, pos)| {
            let n = normals[i];
            let (tangent, bitangent) = if i < tangents_raw.len() {
                let t = tangents_raw[i];
                let tan = [t[0], t[1], t[2]];
                let bitan = cross_scaled(n, tan, t[3]);
                (tan, bitan)
            } else {
                orthonormal_tangent_frame(n)
            };
            Vertex {
                position: *pos,
                uv0: uv0s[i],
                uv1: uv1s[i],
                normal: n,
                tangent,
                bitangent,
                color: colors[i],
            }
        })
        .collect();

    transform_vertices(&mut vertices, node_world);

    let mat = primitive.material();
    let pbr = mat.pbr_metallic_roughness();

    let albedo_image = image_for_idx(
        pbr.base_color_texture()
            .map(|t| t.texture().source().index()),
        images,
    );
    let normal_image = image_for_idx(
        mat.normal_texture().map(|t| t.texture().source().index()),
        images,
    );
    let metallic_roughness_image = image_for_idx(
        pbr.metallic_roughness_texture()
            .map(|t| t.texture().source().index()),
        images,
    );
    let emissive_image = image_for_idx(
        mat.emissive_texture().map(|t| t.texture().source().index()),
        images,
    );
    let aniso = parse_anisotropy(&mat);
    let aniso_img_idx = aniso
        .texture_index
        .and_then(|ti| image_index_for_gltf_texture_index(document, ti));
    let anisotropy_image = image_for_idx(aniso_img_idx, images);
    let clearcoat = parse_clearcoat(&mat);
    let cc_img_idx = clearcoat
        .clearcoat_texture_index
        .and_then(|ti| image_index_for_gltf_texture_index(document, ti));
    let cr_img_idx = clearcoat
        .rough_texture_index
        .and_then(|ti| image_index_for_gltf_texture_index(document, ti));
    let clearcoat_image = image_for_idx(cc_img_idx, images);
    let clearcoat_roughness_image = image_for_idx(cr_img_idx, images);

    let trans_idx = mat
        .transmission()
        .and_then(|tr| tr.transmission_texture())
        .and_then(|info| image_index_for_gltf_texture_index(document, info.texture().index()));
    let transmission_image = image_for_idx(trans_idx, images);
    let specular_idx = mat
        .specular()
        .and_then(|sp| sp.specular_texture())
        .and_then(|info| image_index_for_gltf_texture_index(document, info.texture().index()));
    let specular_image = image_for_idx(specular_idx, images);
    let specular_color_idx = mat
        .specular()
        .and_then(|sp| sp.specular_color_texture())
        .and_then(|info| image_index_for_gltf_texture_index(document, info.texture().index()));
    let specular_color_image = image_for_idx(specular_color_idx, images);
    let thick_idx = mat
        .volume()
        .and_then(|v| v.thickness_texture())
        .and_then(|info| image_index_for_gltf_texture_index(document, info.texture().index()));
    let thickness_image = image_for_idx(thick_idx, images);

    out.push(GltfPrimitive {
        mesh: Mesh::new(vertices, indices),
        material: convert_material(&mat, &aniso, &clearcoat),
        albedo_image,
        normal_image,
        metallic_roughness_image,
        emissive_image,
        anisotropy_image,
        clearcoat_image,
        clearcoat_roughness_image,
        transmission_image,
        specular_image,
        specular_color_image,
        thickness_image,
    });
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn image_for_idx(idx: Option<usize>, images: &[gltf::image::Data]) -> Option<RgbaImage> {
    let img = images.get(idx?)?;
    Some(RgbaImage {
        data: to_rgba8(img),
        width: img.width,
        height: img.height,
    })
}

/// `KHR_materials_*` JSON `texture.index` points at glTF **`textures[]`**, not `images[]`.
fn image_index_for_gltf_texture_index(
    document: &gltf::Document,
    texture_index: usize,
) -> Option<usize> {
    document
        .textures()
        .nth(texture_index)
        .map(|tex| tex.source().index())
}

fn to_rgba8(img: &gltf::image::Data) -> Vec<u8> {
    use gltf::image::Format;
    match img.format {
        Format::R8G8B8A8 => img.pixels.clone(),
        Format::R8G8B8 => img
            .pixels
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        Format::R8G8 => img
            .pixels
            .chunks(2)
            .flat_map(|c| [c[0], c[1], 0, 255])
            .collect(),
        Format::R8 => img.pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        Format::R16G16B16A16 => img
            .pixels
            .chunks(8)
            .flat_map(|c| [c[1], c[3], c[5], c[7]])
            .collect(),
        Format::R16G16B16 => img
            .pixels
            .chunks(6)
            .flat_map(|c| [c[1], c[3], c[5], 255])
            .collect(),
        Format::R16G16 => img
            .pixels
            .chunks(4)
            .flat_map(|c| [c[1], c[3], 0, 255])
            .collect(),
        Format::R16 => img
            .pixels
            .chunks(2)
            .flat_map(|c| [c[1], c[1], c[1], 255])
            .collect(),
        // HDR float: clamp and convert
        Format::R32G32B32FLOAT => img
            .pixels
            .chunks(12)
            .flat_map(|c| {
                let r = f32::from_le_bytes([c[0], c[1], c[2], c[3]]).clamp(0.0, 1.0);
                let g = f32::from_le_bytes([c[4], c[5], c[6], c[7]]).clamp(0.0, 1.0);
                let b = f32::from_le_bytes([c[8], c[9], c[10], c[11]]).clamp(0.0, 1.0);
                [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255]
            })
            .collect(),
        Format::R32G32B32A32FLOAT => img
            .pixels
            .chunks(16)
            .flat_map(|c| {
                let r = f32::from_le_bytes([c[0], c[1], c[2], c[3]]).clamp(0.0, 1.0);
                let g = f32::from_le_bytes([c[4], c[5], c[6], c[7]]).clamp(0.0, 1.0);
                let b = f32::from_le_bytes([c[8], c[9], c[10], c[11]]).clamp(0.0, 1.0);
                let a = f32::from_le_bytes([c[12], c[13], c[14], c[15]]).clamp(0.0, 1.0);
                [
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    (a * 255.0) as u8,
                ]
            })
            .collect(),
    }
}

fn cross_scaled(n: [f32; 3], t: [f32; 3], handedness: f32) -> [f32; 3] {
    let b = [
        n[1] * t[2] - n[2] * t[1],
        n[2] * t[0] - n[0] * t[2],
        n[0] * t[1] - n[1] * t[0],
    ];
    [b[0] * handedness, b[1] * handedness, b[2] * handedness]
}

fn orthonormal_tangent_frame(n: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let up = if n[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let t = normalize(cross_vecs(up, n));
    let b = cross_vecs(n, t);
    (t, b)
}

fn cross_vecs(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-6 {
        [1.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize ────────────────────────────────────────────────────────────

    #[test]
    fn normalize_unit_vector_unchanged() {
        let v = normalize([1.0, 0.0, 0.0]);
        assert!((v[0] - 1.0).abs() < 1e-6);
        assert!(v[1].abs() < 1e-6);
        assert!(v[2].abs() < 1e-6);
    }

    #[test]
    fn normalize_scaled_vector() {
        let v = normalize([3.0, 0.0, 0.0]);
        assert!((v[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_near_zero_returns_fallback() {
        let v = normalize([0.0, 0.0, 0.0]);
        assert_eq!(v, [1.0, 0.0, 0.0]);
    }

    // ── cross_vecs ───────────────────────────────────────────────────────────

    #[test]
    fn cross_x_y_gives_z() {
        let c = cross_vecs([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((c[0]).abs() < 1e-6);
        assert!((c[1]).abs() < 1e-6);
        assert!((c[2] - 1.0).abs() < 1e-6);
    }

    // ── cross_scaled ─────────────────────────────────────────────────────────

    #[test]
    fn cross_scaled_positive_handedness() {
        let n = [0.0f32, 0.0, 1.0];
        let t = [1.0f32, 0.0, 0.0];
        let b = cross_scaled(n, t, 1.0);
        // n×t = (0,0,1)×(1,0,0) = (0*0-1*0, 1*1-0*0, 0*0-0*1) = (0,1,0)
        assert!((b[0]).abs() < 1e-6);
        assert!((b[1] - 1.0).abs() < 1e-6);
        assert!((b[2]).abs() < 1e-6);
    }

    #[test]
    fn cross_scaled_negative_handedness_flips() {
        let n = [0.0f32, 0.0, 1.0];
        let t = [1.0f32, 0.0, 0.0];
        let b = cross_scaled(n, t, -1.0);
        assert!((b[1] + 1.0).abs() < 1e-6);
    }

    // ── orthonormal_tangent_frame ─────────────────────────────────────────────

    #[test]
    fn tangent_frame_orthonormal_for_z_normal() {
        let (t, b) = orthonormal_tangent_frame([0.0, 0.0, 1.0]);
        // t and b should be unit length
        let tl = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        let bl = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
        assert!((tl - 1.0).abs() < 1e-5);
        assert!((bl - 1.0).abs() < 1e-5);
        // t·n = 0, b·n = 0
        let n = [0.0f32, 0.0, 1.0];
        let tdotn = t[0] * n[0] + t[1] * n[1] + t[2] * n[2];
        let bdotn = b[0] * n[0] + b[1] * n[1] + b[2] * n[2];
        assert!(tdotn.abs() < 1e-5);
        assert!(bdotn.abs() < 1e-5);
    }

    #[test]
    fn tangent_frame_orthonormal_for_y_normal() {
        let (t, _b) = orthonormal_tangent_frame([0.0, 1.0, 0.0]);
        let tl = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        assert!((tl - 1.0).abs() < 1e-5);
        let n = [0.0f32, 1.0, 0.0];
        let tdotn = t[0] * n[0] + t[1] * n[1] + t[2] * n[2];
        assert!(tdotn.abs() < 1e-5);
    }

    // ── to_rgba8 ─────────────────────────────────────────────────────────────

    fn make_img(format: gltf::image::Format, pixels: Vec<u8>) -> gltf::image::Data {
        gltf::image::Data {
            format,
            width: 1,
            height: 1,
            pixels,
        }
    }

    #[test]
    fn to_rgba8_r8g8b8a8_passthrough() {
        let img = make_img(gltf::image::Format::R8G8B8A8, vec![10, 20, 30, 200]);
        assert_eq!(to_rgba8(&img), vec![10, 20, 30, 200]);
    }

    #[test]
    fn to_rgba8_r8g8b8_adds_alpha() {
        let img = make_img(gltf::image::Format::R8G8B8, vec![1, 2, 3]);
        assert_eq!(to_rgba8(&img), vec![1, 2, 3, 255]);
    }

    #[test]
    fn to_rgba8_r8g8_pads_blue_alpha() {
        let img = make_img(gltf::image::Format::R8G8, vec![50, 100]);
        assert_eq!(to_rgba8(&img), vec![50, 100, 0, 255]);
    }

    #[test]
    fn to_rgba8_r8_grayscale_expands() {
        let img = make_img(gltf::image::Format::R8, vec![128]);
        assert_eq!(to_rgba8(&img), vec![128, 128, 128, 255]);
    }

    #[test]
    fn to_rgba8_r16_takes_high_byte() {
        // R16 big-endian: high byte first
        let img = make_img(gltf::image::Format::R16, vec![0xAB, 0xCD]);
        let out = to_rgba8(&img);
        // c[1] = 0xCD (high byte of little-endian? No: pixels[0]=low, pixels[1]=high)
        assert_eq!(out, vec![0xCD, 0xCD, 0xCD, 255]);
    }

    #[test]
    fn to_rgba8_r32g32b32_float_clamps_and_converts() {
        let r = 0.5f32;
        let g = 1.0f32;
        let b = 2.0f32; // will be clamped to 1.0
        let mut pixels = Vec::new();
        pixels.extend_from_slice(&r.to_le_bytes());
        pixels.extend_from_slice(&g.to_le_bytes());
        pixels.extend_from_slice(&b.to_le_bytes());
        let img = make_img(gltf::image::Format::R32G32B32FLOAT, pixels);
        let out = to_rgba8(&img);
        assert_eq!(out[0], (0.5 * 255.0) as u8);
        assert_eq!(out[1], 255u8);
        assert_eq!(out[2], 255u8);
        assert_eq!(out[3], 255u8);
    }

    #[test]
    fn to_rgba8_r32g32b32a32_float_converts() {
        let mut pixels = Vec::new();
        for v in [0.25f32, 0.5, 0.75, 1.0] {
            pixels.extend_from_slice(&v.to_le_bytes());
        }
        let img = make_img(gltf::image::Format::R32G32B32A32FLOAT, pixels);
        let out = to_rgba8(&img);
        assert_eq!(out[0], (0.25 * 255.0) as u8);
        assert_eq!(out[1], (0.5 * 255.0) as u8);
        assert_eq!(out[2], (0.75 * 255.0) as u8);
        assert_eq!(out[3], 255u8);
    }

    #[test]
    fn clearcoat_json_texture_info_parsed() {
        let o = serde_json::json!({
            "clearcoatFactor": 0.9,
            "clearcoatRoughnessFactor": 0.25,
            "clearcoatTexture": {"index": 2, "texCoord": 1},
            "clearcoatRoughnessTexture": {"index": 5, "texCoord": 0},
        });
        let m = o.as_object().unwrap();
        let (ci, ctc) = super::extension_texture_index_coord(m, "clearcoatTexture");
        assert_eq!(ci, Some(2));
        assert_eq!(ctc, 1);
        let (ri, rtc) = super::extension_texture_index_coord(m, "clearcoatRoughnessTexture");
        assert_eq!(ri, Some(5));
        assert_eq!(rtc, 0);
    }

    #[test]
    fn khr_texture_transform_json_parsed() {
        let tex = serde_json::json!({
            "index": 0,
            "texCoord": 1,
            "extensions": {
                "KHR_texture_transform": {
                    "offset": [0.1, 0.2],
                    "rotation": 0.5,
                    "scale": [2.0, 3.0]
                }
            }
        });
        let m = tex.as_object().unwrap();
        let uv = super::texture_uv_transform_from_json_tex_info(m);
        assert!((uv.offset[0] - 0.1).abs() < 1e-5);
        assert!((uv.offset[1] - 0.2).abs() < 1e-5);
        assert!((uv.scale[0] - 2.0).abs() < 1e-5);
        assert!((uv.scale[1] - 3.0).abs() < 1e-5);
        assert!((uv.rotation - 0.5).abs() < 1e-5);
        assert_eq!(uv.tex_coord, 1);
    }
}

#[derive(Clone, Debug, Default)]
struct ParsedAnisotropy {
    strength: f32,
    rotation: f32,
    texture_index: Option<usize>,
    uv: TextureUvTransform,
}

#[derive(Clone, Debug, Default)]
struct ParsedClearcoat {
    factor: f32,
    roughness: f32,
    clearcoat_texture_index: Option<usize>,
    rough_texture_index: Option<usize>,
    clearcoat_uv: TextureUvTransform,
    rough_uv: TextureUvTransform,
}

fn json_vec2_f32(a: Option<&serde_json::Value>, default: [f32; 2]) -> [f32; 2] {
    let Some(serde_json::Value::Array(arr)) = a else {
        return default;
    };
    let x = arr
        .first()
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default[0] as f64) as f32;
    let y = arr
        .get(1)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default[1] as f64) as f32;
    [x, y]
}

fn texture_uv_merge_khr_json(
    base_tex_coord: u32,
    tt: &serde_json::Map<String, serde_json::Value>,
) -> TextureUvTransform {
    let offset = json_vec2_f32(tt.get("offset"), [0.0, 0.0]);
    let scale = json_vec2_f32(tt.get("scale"), [1.0, 1.0]);
    let rotation = tt
        .get("rotation")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let tex_override = tt
        .get("texCoord")
        .and_then(serde_json::Value::as_u64)
        .map(|u| u.min(1) as u32);
    TextureUvTransform {
        offset,
        scale,
        rotation,
        tex_coord: tex_override.unwrap_or(base_tex_coord),
    }
}

/// Lit `KHR_texture_transform` sur une `textureInfo` JSON (extensions imbriquées).
fn texture_uv_transform_from_json_tex_info(
    tex: &serde_json::Map<String, serde_json::Value>,
) -> TextureUvTransform {
    let base_tc = tex
        .get("texCoord")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .min(1) as u32;
    let Some(ext) = tex.get("extensions").and_then(|e| e.as_object()) else {
        return TextureUvTransform {
            tex_coord: base_tc,
            ..Default::default()
        };
    };
    let Some(tt) = ext.get("KHR_texture_transform").and_then(|t| t.as_object()) else {
        return TextureUvTransform {
            tex_coord: base_tc,
            ..Default::default()
        };
    };
    texture_uv_merge_khr_json(base_tc, tt)
}

fn uv_transform_from_gltf_info(info: &gltf::texture::Info<'_>) -> TextureUvTransform {
    let base_tc = info.tex_coord().min(1);
    if let Some(tt) = info.texture_transform() {
        let tc = tt.tex_coord().map(|u| u.min(1)).unwrap_or(base_tc);
        TextureUvTransform {
            offset: tt.offset(),
            scale: tt.scale(),
            rotation: tt.rotation(),
            tex_coord: tc,
        }
    } else {
        TextureUvTransform {
            tex_coord: base_tc,
            ..Default::default()
        }
    }
}

fn uv_transform_from_normal_texture(nt: &gltf::material::NormalTexture<'_>) -> TextureUvTransform {
    let base_tc = nt.tex_coord().min(1);
    nt.extension_value("KHR_texture_transform")
        .and_then(|v| v.as_object())
        .map(|o| texture_uv_merge_khr_json(base_tc, o))
        .unwrap_or(TextureUvTransform {
            tex_coord: base_tc,
            ..Default::default()
        })
}

fn parse_anisotropy(mat: &gltf::Material<'_>) -> ParsedAnisotropy {
    let Some(v) = mat.extension_value("KHR_materials_anisotropy") else {
        return ParsedAnisotropy::default();
    };
    let strength = v
        .get("anisotropyStrength")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let rotation = v
        .get("anisotropyRotation")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let (texture_index, uv) = v
        .get("anisotropyTexture")
        .and_then(|t| t.as_object())
        .map(|tex| {
            let idx = tex
                .get("index")
                .and_then(serde_json::Value::as_u64)
                .map(|u| u as usize);
            let uv = texture_uv_transform_from_json_tex_info(tex);
            (idx, uv)
        })
        .unwrap_or((None, TextureUvTransform::default()));
    ParsedAnisotropy {
        strength,
        rotation,
        texture_index,
        uv,
    }
}

fn parse_clearcoat(mat: &gltf::Material<'_>) -> ParsedClearcoat {
    let Some(v) = mat.extension_value("KHR_materials_clearcoat") else {
        return ParsedClearcoat::default();
    };
    let Some(o) = v.as_object() else {
        return ParsedClearcoat::default();
    };
    let factor = o
        .get("clearcoatFactor")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let rough = o
        .get("clearcoatRoughnessFactor")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let (clearcoat_texture_index, clearcoat_uv) = o
        .get("clearcoatTexture")
        .and_then(|t| t.as_object())
        .map(|tex| {
            let idx = tex
                .get("index")
                .and_then(serde_json::Value::as_u64)
                .map(|u| u as usize);
            (idx, texture_uv_transform_from_json_tex_info(tex))
        })
        .unwrap_or((None, TextureUvTransform::default()));
    let (rough_texture_index, rough_uv) = o
        .get("clearcoatRoughnessTexture")
        .and_then(|t| t.as_object())
        .map(|tex| {
            let idx = tex
                .get("index")
                .and_then(serde_json::Value::as_u64)
                .map(|u| u as usize);
            (idx, texture_uv_transform_from_json_tex_info(tex))
        })
        .unwrap_or((None, TextureUvTransform::default()));
    ParsedClearcoat {
        factor: factor.clamp(0.0, 1.0),
        roughness: rough.clamp(0.0, 1.0),
        clearcoat_texture_index,
        rough_texture_index,
        clearcoat_uv,
        rough_uv,
    }
}

#[cfg(test)]
fn extension_texture_index_coord(
    o: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> (Option<usize>, u32) {
    o.get(key)
        .and_then(|t| t.as_object())
        .map(|tex| {
            let idx = tex
                .get("index")
                .and_then(serde_json::Value::as_u64)
                .map(|u| u as usize);
            let tc = tex
                .get("texCoord")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            (idx, tc.min(1))
        })
        .unwrap_or((None, 0))
}

fn convert_material(
    mat: &gltf::Material<'_>,
    aniso: &ParsedAnisotropy,
    clearcoat: &ParsedClearcoat,
) -> Material {
    let pbr = mat.pbr_metallic_roughness();
    let base = pbr.base_color_factor();
    let emissive = mat.emissive_factor();
    let emissive_strength = mat.emissive_strength().unwrap_or(1.0);
    let alpha_mode = match mat.alpha_mode() {
        gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
        gltf::material::AlphaMode::Mask => AlphaMode::Mask,
        gltf::material::AlphaMode::Blend => AlphaMode::Blend,
    };
    let ior = mat.ior().unwrap_or(1.5).clamp(1.0001, 256.0);

    let mut texture_transforms = [TextureUvTransform::default(); 11];
    texture_transforms[TEX_UV_ALBEDO] = pbr
        .base_color_texture()
        .map(|t| uv_transform_from_gltf_info(&t))
        .unwrap_or_default();
    texture_transforms[TEX_UV_NORMAL] = mat
        .normal_texture()
        .map(|t| uv_transform_from_normal_texture(&t))
        .unwrap_or_default();
    texture_transforms[TEX_UV_METALLIC_ROUGHNESS] = pbr
        .metallic_roughness_texture()
        .map(|t| uv_transform_from_gltf_info(&t))
        .unwrap_or_default();
    texture_transforms[TEX_UV_EMISSIVE] = mat
        .emissive_texture()
        .map(|t| uv_transform_from_gltf_info(&t))
        .unwrap_or_default();
    texture_transforms[TEX_UV_ANISOTROPY] = aniso.uv;
    texture_transforms[TEX_UV_CLEARCOAT] = clearcoat.clearcoat_uv;
    texture_transforms[TEX_UV_CLEARCOAT_ROUGHNESS] = clearcoat.rough_uv;

    let mut khr_flags: u32 = 0;
    let mut transmission_factor = 0.0f32;
    let mut specular_factor = 1.0f32;
    let mut specular_color_factor = [1.0f32, 1.0, 1.0];
    let mut thickness_factor = 0.0f32;
    let mut attenuation_distance = 1.0e10f32;
    let mut attenuation_color = [1.0f32, 1.0, 1.0];

    if let Some(tr) = mat.transmission() {
        khr_flags |= 2;
        transmission_factor = tr.transmission_factor().clamp(0.0, 1.0);
        if let Some(info) = tr.transmission_texture() {
            texture_transforms[TEX_UV_TRANSMISSION] = uv_transform_from_gltf_info(&info);
        }
    }

    if let Some(sp) = mat.specular() {
        khr_flags |= 1;
        specular_factor = sp.specular_factor();
        specular_color_factor = sp.specular_color_factor();
        if let Some(info) = sp.specular_texture() {
            texture_transforms[TEX_UV_SPECULAR] = uv_transform_from_gltf_info(&info);
        }
        if let Some(info) = sp.specular_color_texture() {
            texture_transforms[TEX_UV_SPECULAR_COLOR] = uv_transform_from_gltf_info(&info);
        }
    }

    if let Some(vol) = mat.volume() {
        khr_flags |= 4;
        thickness_factor = vol.thickness_factor().max(0.0);
        attenuation_distance = vol.attenuation_distance();
        if attenuation_distance < 1e-6 {
            attenuation_distance = 1.0e10;
        }
        attenuation_color = vol.attenuation_color();
        if let Some(info) = vol.thickness_texture() {
            texture_transforms[TEX_UV_THICKNESS] = uv_transform_from_gltf_info(&info);
        }
    }

    Material {
        name: mat.name().unwrap_or("").to_string(),
        shading_model: ShadingModel::Pbr,
        albedo: base,
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        emissive,
        alpha_mode,
        alpha_cutoff: mat.alpha_cutoff().unwrap_or(0.5),
        double_sided: mat.double_sided(),
        anisotropy_strength: aniso.strength,
        anisotropy_rotation: aniso.rotation,
        ior,
        clearcoat_factor: clearcoat.factor,
        clearcoat_roughness: clearcoat.roughness,
        emissive_strength: emissive_strength.max(0.0),
        transmission_factor,
        specular_factor,
        specular_color_factor,
        thickness_factor,
        attenuation_distance,
        attenuation_color,
        khr_flags,
        texture_transforms,
    }
}
