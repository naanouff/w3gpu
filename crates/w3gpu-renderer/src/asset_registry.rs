use std::collections::HashMap;

use bytemuck::cast_slice;
use w3gpu_assets::{Mesh, Vertex};
use w3gpu_math::BoundingSphere;
use wgpu::util::DeviceExt;

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub bounding_sphere: BoundingSphere,
}

#[derive(Default)]
pub struct AssetRegistry {
    meshes: HashMap<u32, GpuMesh>,
    next_id: u32,
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

        let id = self.next_id;
        self.next_id += 1;
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

    pub fn get_mesh(&self, id: u32) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }
}
