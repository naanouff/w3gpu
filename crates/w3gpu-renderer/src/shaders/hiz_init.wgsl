// Copies the z-prepass Depth32Float texture into mip-0 of the R32Float Hi-Z chain.
@group(0) @binding(0) var depth_in: texture_depth_2d;
@group(0) @binding(1) var hiz_out:  texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(hiz_out);
    if id.x >= size.x || id.y >= size.y { return; }
    let d = textureLoad(depth_in, vec2<i32>(id.xy), 0);
    textureStore(hiz_out, vec2<i32>(id.xy), vec4<f32>(d, 0.0, 0.0, 0.0));
}
