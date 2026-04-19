use bytemuck::{Pod, Zeroable};

/// A single renderable object queried from ECS each frame.
pub struct RenderCommand {
    pub mesh_id: u32,
    pub material_id: u32,
    pub world_matrix: [[f32; 4]; 4],
    pub cast_shadow: bool,
}

/// One GPU draw call covering `instance_count` consecutive entries in the
/// instance storage buffer, all sharing the same mesh and material.
pub struct DrawBatch {
    pub mesh_id: u32,
    pub material_id: u32,
    pub first_instance: u32,
    pub instance_count: u32,
    pub cast_shadow: bool,
}

/// Arguments for `draw_indexed_indirect` — matches the WebGPU / wgpu layout.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct DrawIndexedIndirectArgs {
    pub index_count:    u32,
    pub instance_count: u32,
    pub first_index:    u32,
    pub base_vertex:    i32,
    pub first_instance: u32,
}

/// Sort commands by (mesh_id, material_id) and group consecutive identical
/// entries into batches. Returns the flattened world-matrix array (to upload
/// to the instance storage buffer) and the corresponding batch list.
pub fn build_batches(mut cmds: Vec<RenderCommand>) -> (Vec<[[f32; 4]; 4]>, Vec<DrawBatch>) {
    cmds.sort_unstable_by_key(|c| (c.mesh_id, c.material_id));

    let mut matrices: Vec<[[f32; 4]; 4]> = Vec::with_capacity(cmds.len());
    let mut batches: Vec<DrawBatch> = Vec::new();

    let mut i = 0;
    while i < cmds.len() {
        let mesh_id    = cmds[i].mesh_id;
        let mat_id     = cmds[i].material_id;
        let first      = matrices.len() as u32;
        let mut shadow = false;
        let start      = i;

        while i < cmds.len()
            && cmds[i].mesh_id     == mesh_id
            && cmds[i].material_id == mat_id
        {
            if cmds[i].cast_shadow { shadow = true; }
            matrices.push(cmds[i].world_matrix);
            i += 1;
        }

        batches.push(DrawBatch {
            mesh_id,
            material_id:    mat_id,
            first_instance: first,
            instance_count: (i - start) as u32,
            cast_shadow:    shadow,
        });
    }

    (matrices, batches)
}
