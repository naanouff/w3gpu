use std::collections::HashMap;

use bytemuck::cast_slice;
use w3gpu_assets::{Material, Mesh, Vertex};
use w3gpu_math::BoundingSphere;
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
}

/// Optional texture handles to associate with a material on upload.
/// `None` fields fall back to the registry's 1×1 default textures.
#[derive(Default)]
pub struct MaterialTextures {
    pub albedo: Option<u32>,
    pub normal: Option<u32>,
    pub metallic_roughness: Option<u32>,
    pub emissive: Option<u32>,
}

pub struct AssetRegistry {
    meshes:    HashMap<u32, GpuMesh>,
    materials: HashMap<u32, GpuMaterial>,
    textures:  HashMap<u32, GpuTexture>,
    next_mesh_id:     u32,
    next_material_id: u32,
    next_texture_id:  u32,
    // Shared sampler for all material textures
    pub default_sampler: wgpu::Sampler,
    // 1×1 fallback texture views
    pub white_view:   wgpu::TextureView, // albedo fallback
    pub flat_normal_view: wgpu::TextureView, // normal fallback [128,128,255,255]
    pub default_mr_view:  wgpu::TextureView, // metallic-roughness fallback [0,128,0,255]
    pub black_view:   wgpu::TextureView, // emissive fallback
}

impl AssetRegistry {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let white_view       = upload_1x1(device, queue, [255, 255, 255, 255], false);
        let flat_normal_view = upload_1x1(device, queue, [128, 128, 255, 255], false);
        let default_mr_view  = upload_1x1(device, queue, [0, 128, 0, 255], false);
        let black_view       = upload_1x1(device, queue, [0, 0, 0, 255], false);

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
        }
    }

    pub fn upload_mesh(
        &mut self,
        mesh: &Mesh,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> u32 {
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
        self.meshes.insert(id, GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            bounding_sphere: mesh.bounding_sphere,
            aabb_min: mesh.aabb.min.to_array(),
            aabb_max: mesh.aabb.max.to_array(),
        });
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
        let mr_view     = self.tex_view(textures.metallic_roughness, &self.default_mr_view as *const _);
        let emit_view   = self.tex_view(textures.emissive, &self.black_view as *const _);

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
                    resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                },
            ],
        });

        let id = self.next_material_id;
        self.next_material_id += 1;
        self.materials.insert(id, GpuMaterial { bind_group });
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
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
