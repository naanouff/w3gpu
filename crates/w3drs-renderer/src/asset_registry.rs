use std::collections::HashMap;

use bytemuck::cast_slice;
use w3drs_assets::{AlphaMode, Material, Mesh, Vertex};
use w3drs_math::BoundingSphere;
use wgpu::util::DeviceExt;

use crate::material_uniforms::MaterialUniforms;

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub bounding_sphere: BoundingSphere,
    /// Local-space AABB (used by the GPU occlusion cull pass).
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
}

pub struct GpuTexture {
    pub view: wgpu::TextureView,
}

pub struct GpuMaterial {
    pub bind_group: wgpu::BindGroup,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}

/// Optional texture handles to associate with a material on upload.
/// `None` fields fall back to the registry's 1×1 default textures.
#[derive(Default)]
pub struct MaterialTextures {
    pub albedo: Option<u32>,
    pub normal: Option<u32>,
    pub metallic_roughness: Option<u32>,
    pub emissive: Option<u32>,
    /// `KHR_materials_anisotropy` map (linear); defaults to +X / strength 1 texel when `None`.
    pub anisotropy: Option<u32>,
    /// `KHR_materials_clearcoat` intensity (R); defaults to white (×1) when `None`.
    pub clearcoat: Option<u32>,
    /// `KHR_materials_clearcoat` roughness (G); defaults to G=1 when `None`.
    pub clearcoat_roughness: Option<u32>,
    /// KHR `transmissionTexture` (R).
    pub transmission: Option<u32>,
    /// KHR `specularTexture` (A).
    pub specular: Option<u32>,
    /// KHR `specularColorTexture` sRGB.
    pub specular_color: Option<u32>,
    /// KHR `thicknessTexture` (G).
    pub thickness: Option<u32>,
    /// glTF `occlusionTexture` (R, linear).
    pub occlusion: Option<u32>,
}

pub struct AssetRegistry {
    meshes: HashMap<u32, GpuMesh>,
    materials: HashMap<u32, GpuMaterial>,
    textures: HashMap<u32, GpuTexture>,
    next_mesh_id: u32,
    next_material_id: u32,
    next_texture_id: u32,
    // Shared sampler for all material textures
    pub default_sampler: wgpu::Sampler,
    // 1×1 fallback texture views
    pub white_view: wgpu::TextureView,       // albedo fallback
    pub flat_normal_view: wgpu::TextureView, // normal fallback [128,128,255,255]
    pub default_mr_view: wgpu::TextureView, // metallic-roughness fallback [0,255,255,255] (G=R=1→no scale)
    pub black_view: wgpu::TextureView,      // emissive fallback
    /// Default anisotropy texel per Khronos spec: direction (1,0), strength 1 → RGB linear (1,0.5,1).
    pub default_aniso_view: wgpu::TextureView,
    /// Clearcoat roughness multiplier when no texture: G = 1.0 (`Rgba8Unorm` (0,255,0,255)).
    pub default_clearcoat_rough_view: wgpu::TextureView,
    /// Transmission R=1, transmission factor × texture.
    pub default_transmission_view: wgpu::TextureView,
    /// Specular A=1.
    pub default_specular_view: wgpu::TextureView,
    /// Specular color blanc sRGB.
    pub default_specular_color_view: wgpu::TextureView,
    /// Thickness G=1.
    pub default_thickness_view: wgpu::TextureView,
    /// Occlusion R=1 (linéaire, « pas d’occlusion »).
    pub default_occl_view: wgpu::TextureView,
}

impl AssetRegistry {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let white_view = upload_1x1(device, queue, [255, 255, 255, 255], false);
        let flat_normal_view = upload_1x1(device, queue, [128, 128, 255, 255], false);
        // G=255 (roughness factor ×1), B=255 (metallic factor ×1) per glTF spec §5.22.5
        let default_mr_view = upload_1x1(device, queue, [0, 255, 255, 255], false);
        let black_view = upload_1x1(device, queue, [0, 0, 0, 255], false);
        let default_aniso_view = upload_1x1(device, queue, [255, 128, 255, 255], false);
        let default_clearcoat_rough_view = upload_1x1(device, queue, [0, 255, 0, 255], false);
        // R=0: transmission factor ×0 = no transmission when no texture provided
        let default_transmission_view = upload_1x1(device, queue, [0, 0, 0, 255], false);
        let default_specular_view = upload_1x1(device, queue, [0, 0, 0, 255], false);
        let default_specular_color_view = upload_1x1(device, queue, [255, 255, 255, 255], true);
        let default_thickness_view = upload_1x1(device, queue, [0, 255, 0, 255], false);
        let default_occl_view = upload_1x1(device, queue, [255, 255, 255, 255], false);

        Self {
            meshes: HashMap::new(),
            materials: HashMap::new(),
            textures: HashMap::new(),
            next_mesh_id: 0,
            next_material_id: 0,
            next_texture_id: 0,
            default_sampler,
            white_view,
            flat_normal_view,
            default_mr_view,
            black_view,
            default_aniso_view,
            default_clearcoat_rough_view,
            default_transmission_view,
            default_specular_view,
            default_specular_color_view,
            default_thickness_view,
            default_occl_view,
        }
    }

    pub fn upload_mesh(&mut self, mesh: &Mesh, device: &wgpu::Device, _queue: &wgpu::Queue) -> u32 {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: cast_slice::<Vertex, u8>(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index buffer"),
            contents: cast_slice::<u32, u8>(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let id = self.next_mesh_id;
        self.next_mesh_id += 1;
        self.meshes.insert(
            id,
            GpuMesh {
                vertex_buffer,
                index_buffer,
                index_count: mesh.indices.len() as u32,
                bounding_sphere: mesh.bounding_sphere,
                aabb_min: mesh.aabb.min.to_array(),
                aabb_max: mesh.aabb.max.to_array(),
            },
        );
        id
    }

    /// Upload an RGBA8 texture. `srgb = true` for albedo/emissive, `false` for normal/mr.
    pub fn upload_texture_rgba8(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        let view = upload_rgba8(device, queue, data, width, height, srgb);
        let id = self.next_texture_id;
        self.next_texture_id += 1;
        self.textures.insert(id, GpuTexture { view });
        id
    }

    pub fn upload_material(
        &mut self,
        material: &Material,
        textures: MaterialTextures,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> u32 {
        let uniforms = MaterialUniforms::from(material);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let albedo_view = self.tex_view(textures.albedo, &self.white_view as *const _);
        let normal_view = self.tex_view(textures.normal, &self.flat_normal_view as *const _);
        let mr_view = self.tex_view(
            textures.metallic_roughness,
            &self.default_mr_view as *const _,
        );
        let emit_view = self.tex_view(textures.emissive, &self.black_view as *const _);
        let aniso_view = self.tex_view(textures.anisotropy, &self.default_aniso_view as *const _);
        let clearcoat_view = self.tex_view(textures.clearcoat, &self.white_view as *const _);
        let clearcoat_rough_view = self.tex_view(
            textures.clearcoat_roughness,
            &self.default_clearcoat_rough_view as *const _,
        );
        let trans_view = self.tex_view(
            textures.transmission,
            &self.default_transmission_view as *const _,
        );
        let spec_view = self.tex_view(textures.specular, &self.default_specular_view as *const _);
        let sc_view = self.tex_view(
            textures.specular_color,
            &self.default_specular_color_view as *const _,
        );
        let thick_view =
            self.tex_view(textures.thickness, &self.default_thickness_view as *const _);
        let occ_view = self.tex_view(textures.occlusion, &self.default_occl_view as *const _);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(mr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(emit_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(aniso_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(clearcoat_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(clearcoat_rough_view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(trans_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(spec_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(sc_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(thick_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(occ_view),
                },
            ],
        });

        let id = self.next_material_id;
        self.next_material_id += 1;
        self.materials.insert(
            id,
            GpuMaterial {
                bind_group,
                alpha_mode: material.alpha_mode.clone(),
                alpha_cutoff: material.alpha_cutoff,
                double_sided: material.double_sided,
            },
        );
        id
    }

    pub fn get_mesh(&self, id: u32) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }

    pub fn get_material(&self, id: u32) -> Option<&GpuMaterial> {
        self.materials.get(&id)
    }

    pub fn get_texture(&self, id: u32) -> Option<&GpuTexture> {
        self.textures.get(&id)
    }

    /// Vide tous les maillages / matériaux / textures utilisateur et remet les compteurs d’`id` à zéro,
    /// avec les mêmes fallbacks 1×1 que [`Self::new`].
    pub fn reset_in_place(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        *self = Self::new(device, queue);
    }

    // Returns the texture view for the given optional id, or the fallback via raw pointer
    // (avoids borrow conflicts since self.textures and self.fallbacks are separate fields).
    fn tex_view(&self, id: Option<u32>, fallback: *const wgpu::TextureView) -> &wgpu::TextureView {
        if let Some(tex_id) = id {
            if let Some(tex) = self.textures.get(&tex_id) {
                return &tex.view;
            }
        }
        // SAFETY: fallback pointer points to a field of self which lives at least as long as self
        unsafe { &*fallback }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn upload_1x1(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pixel: [u8; 4],
    srgb: bool,
) -> wgpu::TextureView {
    upload_rgba8(device, queue, &pixel, 1, 1, srgb)
}

fn upload_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u8],
    width: u32,
    height: u32,
    srgb: bool,
) -> wgpu::TextureView {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let alpha_aware = srgb && data.chunks_exact(4).any(|px| px[3] < 255);
    let mips = rgba8_mip_chain(data, width, height, alpha_aware);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, mip) in mips.iter().enumerate() {
        let level = level as u32;
        let mip_width = (width >> level).max(1);
        let mip_height = (height >> level).max(1);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * mip_width),
                rows_per_image: Some(mip_height),
            },
            wgpu::Extent3d {
                width: mip_width,
                height: mip_height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn rgba8_mip_chain(data: &[u8], width: u32, height: u32, alpha_aware_rgb: bool) -> Vec<Vec<u8>> {
    let mut levels = Vec::new();
    let base = if alpha_aware_rgb {
        bleed_transparent_rgba8(data, width, height)
    } else {
        data.to_vec()
    };
    levels.push(base);
    let mut w = width;
    let mut h = height;
    while w > 1 || h > 1 {
        let prev = levels.last().expect("base mip exists");
        let nw = (w / 2).max(1);
        let nh = (h / 2).max(1);
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0u32; 4];
                let mut alpha_weighted_rgb = [0u32; 3];
                let mut alpha_weight = 0u32;
                let mut n = 0u32;
                for oy in 0..2 {
                    for ox in 0..2 {
                        let sx = (x * 2 + ox).min(w - 1);
                        let sy = (y * 2 + oy).min(h - 1);
                        let i = ((sy * w + sx) * 4) as usize;
                        for c in 0..4 {
                            acc[c] += prev[i + c] as u32;
                        }
                        if alpha_aware_rgb {
                            let a = prev[i + 3] as u32;
                            for c in 0..3 {
                                alpha_weighted_rgb[c] += prev[i + c] as u32 * a;
                            }
                            alpha_weight += a;
                        }
                        n += 1;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                if alpha_aware_rgb && alpha_weight > 0 {
                    for c in 0..3 {
                        next[o + c] = (alpha_weighted_rgb[c] / alpha_weight) as u8;
                    }
                    next[o + 3] = (acc[3] / n) as u8;
                } else {
                    for c in 0..4 {
                        next[o + c] = (acc[c] / n) as u8;
                    }
                }
            }
        }
        levels.push(next);
        w = nw;
        h = nh;
    }
    levels
}

fn bleed_transparent_rgba8(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    const ALPHA_BLEED_THRESHOLD: u8 = 8;
    let mut out = data.to_vec();
    if !out.chunks_exact(4).any(|px| px[3] <= ALPHA_BLEED_THRESHOLD) {
        return out;
    }

    let mut unresolved = true;
    let max_iters = (width.max(height).ilog2() + 2).max(4);
    for _ in 0..max_iters {
        if !unresolved {
            break;
        }
        unresolved = false;
        let prev = out.clone();
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                if prev[idx + 3] > ALPHA_BLEED_THRESHOLD {
                    continue;
                }
                let mut rgb = [0u32; 3];
                let mut count = 0u32;
                for oy in -1i32..=1 {
                    for ox in -1i32..=1 {
                        if ox == 0 && oy == 0 {
                            continue;
                        }
                        let sx = x as i32 + ox;
                        let sy = y as i32 + oy;
                        if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                            continue;
                        }
                        let ni = (((sy as u32) * width + sx as u32) * 4) as usize;
                        if prev[ni + 3] <= ALPHA_BLEED_THRESHOLD {
                            continue;
                        }
                        for c in 0..3 {
                            rgb[c] += prev[ni + c] as u32;
                        }
                        count += 1;
                    }
                }
                if count > 0 {
                    for c in 0..3 {
                        out[idx + c] = (rgb[c] / count) as u8;
                    }
                } else {
                    unresolved = true;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{bleed_transparent_rgba8, rgba8_mip_chain};

    #[test]
    fn rgba8_mip_chain_reaches_one_by_one() {
        let data = vec![255u8; 4 * 4 * 4];
        let mips = rgba8_mip_chain(&data, 4, 4, false);
        assert_eq!(mips.len(), 3);
        assert_eq!(mips[0].len(), 4 * 4 * 4);
        assert_eq!(mips[1].len(), 2 * 2 * 4);
        assert_eq!(mips[2].len(), 4);
    }

    #[test]
    fn rgba8_mip_chain_averages_pixels() {
        let data = vec![0, 0, 0, 255, 100, 0, 0, 255, 200, 0, 0, 255, 255, 0, 0, 255];
        let mips = rgba8_mip_chain(&data, 2, 2, false);
        assert_eq!(mips[1], vec![138, 0, 0, 255]);
    }

    #[test]
    fn rgba8_mip_chain_alpha_weights_rgb() {
        let data = vec![
            255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255,
        ];
        let mips = rgba8_mip_chain(&data, 2, 2, true);
        assert_eq!(mips[1], vec![255, 255, 255, 127]);
    }

    #[test]
    fn bleed_transparent_rgba8_fills_zero_alpha_rgb_from_neighbors() {
        let data = vec![255, 0, 0, 0, 10, 20, 30, 255];
        let out = bleed_transparent_rgba8(&data, 2, 1);
        assert_eq!(&out[0..4], &[10, 20, 30, 0]);
        assert_eq!(&out[4..8], &[10, 20, 30, 255]);
    }
}
