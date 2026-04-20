// 2×2 max-filter downsample step for Hi-Z pyramid construction.
// `src` is a single-mip view of level N; `dst` writes to level N+1.
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dst_size = textureDimensions(dst);
    if id.x >= dst_size.x || id.y >= dst_size.y { return; }
    let s        = vec2<i32>(id.xy) * 2;
    let src_max  = vec2<i32>(textureDimensions(src)) - vec2<i32>(1);
    let d0 = textureLoad(src, clamp(s + vec2<i32>(0, 0), vec2<i32>(0), src_max), 0).r;
    let d1 = textureLoad(src, clamp(s + vec2<i32>(1, 0), vec2<i32>(0), src_max), 0).r;
    let d2 = textureLoad(src, clamp(s + vec2<i32>(0, 1), vec2<i32>(0), src_max), 0).r;
    let d3 = textureLoad(src, clamp(s + vec2<i32>(1, 1), vec2<i32>(0), src_max), 0).r;
    textureStore(dst, vec2<i32>(id.xy), vec4<f32>(max(max(d0, d1), max(d2, d3)), 0.0, 0.0, 0.0));
}
