// Fixture Phase B — referenced by render_graph.json (compute pass).
// Not yet wired to wgpu; must compile as valid WGSL.

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    _ = gid;
}
