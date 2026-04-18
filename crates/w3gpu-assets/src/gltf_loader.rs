use thiserror::Error;

use crate::{material::{AlphaMode, Material}, mesh::Mesh, vertex::Vertex};

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
}

/// Load all mesh primitives from a GLB/glTF byte slice.
pub fn load_from_bytes(bytes: &[u8]) -> Result<Vec<GltfPrimitive>, GltfError> {
    let (document, buffers, images) = gltf::import_slice(bytes)?;
    let mut result = Vec::new();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
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

            let vertices: Vec<Vertex> = positions
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

            let mat = primitive.material();
            let pbr = mat.pbr_metallic_roughness();

            let albedo_image             = image_for_idx(pbr.base_color_texture().map(|t| t.texture().source().index()), &images);
            let normal_image             = image_for_idx(mat.normal_texture().map(|t| t.texture().source().index()), &images);
            let metallic_roughness_image = image_for_idx(pbr.metallic_roughness_texture().map(|t| t.texture().source().index()), &images);
            let emissive_image           = image_for_idx(mat.emissive_texture().map(|t| t.texture().source().index()), &images);

            result.push(GltfPrimitive {
                mesh: Mesh::new(vertices, indices),
                material: convert_material(&mat),
                albedo_image,
                normal_image,
                metallic_roughness_image,
                emissive_image,
            });
        }
    }

    Ok(result)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn image_for_idx(idx: Option<usize>, images: &[gltf::image::Data]) -> Option<RgbaImage> {
    let img = images.get(idx?)?;
    Some(RgbaImage { data: to_rgba8(img), width: img.width, height: img.height })
}

fn to_rgba8(img: &gltf::image::Data) -> Vec<u8> {
    use gltf::image::Format;
    match img.format {
        Format::R8G8B8A8 => img.pixels.clone(),
        Format::R8G8B8   => img.pixels.chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        Format::R8G8     => img.pixels.chunks(2)
            .flat_map(|c| [c[0], c[1], 0, 255])
            .collect(),
        Format::R8       => img.pixels.iter()
            .flat_map(|&v| [v, v, v, 255])
            .collect(),
        Format::R16G16B16A16 => img.pixels.chunks(8)
            .flat_map(|c| [c[1], c[3], c[5], c[7]])
            .collect(),
        Format::R16G16B16    => img.pixels.chunks(6)
            .flat_map(|c| [c[1], c[3], c[5], 255])
            .collect(),
        Format::R16G16       => img.pixels.chunks(4)
            .flat_map(|c| [c[1], c[3], 0, 255])
            .collect(),
        Format::R16          => img.pixels.chunks(2)
            .flat_map(|c| [c[1], c[1], c[1], 255])
            .collect(),
        // HDR float: clamp and convert
        Format::R32G32B32FLOAT => img.pixels.chunks(12)
            .flat_map(|c| {
                let r = f32::from_le_bytes([c[0],c[1],c[2],c[3]]).clamp(0.0,1.0);
                let g = f32::from_le_bytes([c[4],c[5],c[6],c[7]]).clamp(0.0,1.0);
                let b = f32::from_le_bytes([c[8],c[9],c[10],c[11]]).clamp(0.0,1.0);
                [(r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8, 255]
            })
            .collect(),
        Format::R32G32B32A32FLOAT => img.pixels.chunks(16)
            .flat_map(|c| {
                let r = f32::from_le_bytes([c[0],c[1],c[2],c[3]]).clamp(0.0,1.0);
                let g = f32::from_le_bytes([c[4],c[5],c[6],c[7]]).clamp(0.0,1.0);
                let b = f32::from_le_bytes([c[8],c[9],c[10],c[11]]).clamp(0.0,1.0);
                let a = f32::from_le_bytes([c[12],c[13],c[14],c[15]]).clamp(0.0,1.0);
                [(r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8, (a*255.0) as u8]
            })
            .collect(),
    }
}

fn cross_scaled(n: [f32; 3], t: [f32; 3], handedness: f32) -> [f32; 3] {
    let b = [
        n[1]*t[2] - n[2]*t[1],
        n[2]*t[0] - n[0]*t[2],
        n[0]*t[1] - n[1]*t[0],
    ];
    [b[0]*handedness, b[1]*handedness, b[2]*handedness]
}

fn orthonormal_tangent_frame(n: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let up = if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let t = normalize(cross_vecs(up, n));
    let b = cross_vecs(n, t);
    (t, b)
}

fn cross_vecs(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
    if len < 1e-6 { [1.0, 0.0, 0.0] } else { [v[0]/len, v[1]/len, v[2]/len] }
}

fn convert_material(mat: &gltf::Material<'_>) -> Material {
    let pbr = mat.pbr_metallic_roughness();
    let base = pbr.base_color_factor();
    let emissive = mat.emissive_factor();
    let alpha_mode = match mat.alpha_mode() {
        gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
        gltf::material::AlphaMode::Mask   => AlphaMode::Mask,
        gltf::material::AlphaMode::Blend  => AlphaMode::Blend,
    };
    Material {
        name: mat.name().unwrap_or("").to_string(),
        albedo: base,
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        emissive,
        alpha_mode,
        alpha_cutoff: mat.alpha_cutoff().unwrap_or(0.5),
        double_sided: mat.double_sided(),
        ..Default::default()
    }
}
