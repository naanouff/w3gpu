// Fixture Phase B — compute: group 0 (`storage_buffers`, `storage_buffers_read`, `storage_writes`, `texture_reads`) ; group 1 : `storage_buffers_group1` (rw) puis `storage_buffers_read_group1` (ro).
// group 0: rw buffers @0.., read-only storage @next, storage textures, then sampled textures.

@group(0) @binding(0)
var<storage, read_write> indirect_args: array<u32>;

@group(0) @binding(1)
var<storage, read> ro_pad: array<u32>;

@group(0) @binding(2)
var ping: texture_storage_2d<rgba16float, write>;

@group(0) @binding(3)
var hdr_pre_raster: texture_2d<f32>;

@group(1) @binding(0)
var<storage, read_write> g1_side: array<u32>;

@group(1) @binding(1)
var<storage, read> g1_ro_pad: array<u32>;

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x == 0u && gid.y == 0u && gid.z == 0u) {
        let _ro = ro_pad[0];
        let _g1ro = g1_ro_pad[0];
        // `hdr_color` is not cleared yet; only exercise the binding (stable output).
        let _pre = textureLoad(hdr_pre_raster, vec2<i32>(0, 0), 0);
        indirect_args[0] = indirect_args[0] + 1u;
        g1_side[0] = indirect_args[0];
        textureStore(ping, vec2<i32>(0, 0), vec4<f32>(0.01, 0.0, 0.0, 1.0));
    }
}
