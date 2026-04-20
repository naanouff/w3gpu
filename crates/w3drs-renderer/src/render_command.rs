use bytemuck::{Pod, Zeroable};

// ── Phase 4.2 types ───────────────────────────────────────────────────────────

/// Per-entity data uploaded to the GPU occlusion-cull compute shader (48 bytes).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct EntityCullData {
    pub aabb_min:    [f32; 3],
    pub first_index: u32,
    pub aabb_max:    [f32; 3],
    pub index_count: u32,
    pub base_vertex: i32,
    pub _pad:        [u32; 3],
}

/// One renderable entity collected from ECS, with world-space AABB and mesh
/// info pre-filled so the GPU cull pass can write draw args directly.
pub struct DrawEntity {
    pub mesh_id:      u32,
    pub material_id:  u32,
    pub world_matrix: [[f32; 4]; 4],
    pub cast_shadow:  bool,
    /// World-space AABB (pre-transformed on CPU).
    pub aabb_min:     [f32; 3],
    pub aabb_max:     [f32; 3],
    pub first_index:  u32,
    pub index_count:  u32,
    pub base_vertex:  i32,
}

/// Shadow draw batch derived from the sorted entity list (grouped by mesh_id).
pub struct ShadowBatch {
    pub mesh_id:        u32,
    pub first_instance: u32,
    pub instance_count: u32,
}

/// Sort entities by (mesh_id, material_id) and return the flat arrays needed
/// for GPU upload plus the sorted list for the CPU render loop.
pub fn build_entity_list(
    mut entities: Vec<DrawEntity>,
) -> (Vec<[[f32; 4]; 4]>, Vec<EntityCullData>, Vec<DrawEntity>) {
    entities.sort_unstable_by_key(|e| (e.mesh_id, e.material_id));
    let matrices: Vec<[[f32; 4]; 4]> = entities.iter().map(|e| e.world_matrix).collect();
    let cull_data: Vec<EntityCullData> = entities.iter().map(|e| EntityCullData {
        aabb_min:    e.aabb_min,
        first_index: e.first_index,
        aabb_max:    e.aabb_max,
        index_count: e.index_count,
        base_vertex: e.base_vertex,
        _pad:        [0; 3],
    }).collect();
    (matrices, cull_data, entities)
}

/// Group the pre-sorted entity list into shadow batches by mesh_id.
/// Because entities are sorted by (mesh_id, mat_id), same-mesh entities are
/// always contiguous → first_instance correctly indexes into the instance buffer.
pub fn derive_shadow_batches(entities: &[DrawEntity]) -> Vec<ShadowBatch> {
    let mut batches = Vec::new();
    let mut i = 0;
    while i < entities.len() {
        let mesh_id = entities[i].mesh_id;
        let first   = i as u32;
        while i < entities.len() && entities[i].mesh_id == mesh_id { i += 1; }
        batches.push(ShadowBatch {
            mesh_id,
            first_instance: first,
            instance_count: i as u32 - first,
        });
    }
    batches
}

// ── Phase 4.1 types (kept for compat) ────────────────────────────────────────

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
