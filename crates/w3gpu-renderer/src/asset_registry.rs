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
}

pub struct GpuMaterial {
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

#[derive(Default)]
pub struct AssetRegistry {
    meshes: HashMap<u32, GpuMesh>,
    materials: HashMap<u32, GpuMaterial>,
    next_mesh_id: u32,
    next_material_id: u32,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self::default()
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
        self.meshes.insert(
            id,
            GpuMesh {
                vertex_buffer,
                index_buffer,
                index_count: mesh.indices.len() as u32,
                bounding_sphere: mesh.bounding_sphere,
            },
        );
        id
    }

    pub fn upload_material(
        &mut self,
        material: &Material,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> u32 {
        let uniforms = MaterialUniforms::from(material);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material bind group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let id = self.next_material_id;
        self.next_material_id += 1;
        self.materials.insert(id, GpuMaterial { uniform_buffer, bind_group });
        id
    }

    pub fn get_mesh(&self, id: u32) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }

    pub fn get_material(&self, id: u32) -> Option<&GpuMaterial> {
        self.materials.get(&id)
    }
}
