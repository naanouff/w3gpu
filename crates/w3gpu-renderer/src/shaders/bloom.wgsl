// Bloom post-process shaders — prefilter + separable gaussian blur.
//
// All passes share the same fullscreen-triangle vertex shader.
// Fragment entry points:
//   fs_prefilter — luminance threshold + half-res downsample (HDR → bloom_a)
//   fs_blur_h    — horizontal 9-tap gaussian (bloom_a → bloom_b)
//   fs_blur_v    — vertical   9-tap gaussian (bloom_b → bloom_a)

// ── Shared vertex shader ───────────────────────────────────────────────────────
// Generates a single fullscreen triangle from vertex index (no VBO needed).
@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32(vi & 1u) * 4.0 - 1.0;
    let y = f32((vi >> 1u) & 1u) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// ── Prefilter bindings ─────────────────────────────────────────────────────────
@group(0) @binding(0) var pf_tex:  texture_2d<f32>;
@group(0) @binding(1) var pf_samp: sampler;
@group(0) @binding(2) var<uniform> pf_params: BloomParams;

struct BloomParams {
    threshold: f32,   // luminance below which no bloom contribution
    knee:      f32,   // soft-knee width around the threshold
    _pad0:     f32,
    _pad1:     f32,
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Karis luminance weight — prevents bright spots (fireflies) from dominating.
fn karis(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 + luma(c));
}

// Soft-knee threshold curve.
fn apply_threshold(c: vec3<f32>) -> vec3<f32> {
    let l = luma(c);
    let rq = clamp(l - pf_params.threshold + pf_params.knee, 0.0, 2.0 * pf_params.knee);
    let w = (rq * rq) / (4.0 * pf_params.knee + 1e-5);
    return c * (max(w, l - pf_params.threshold) / max(l, 1e-5));
}

// Threshold extract + 2× downsample using a Karis-weighted 4-tap average.
// Renders into the bloom texture (half the HDR resolution).
@fragment
fn fs_prefilter(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    // pos.xy is in bloom-space (half-res). Map to HDR UV.
    let hdr_dim   = vec2<f32>(textureDimensions(pf_tex, 0));
    let texel     = 1.0 / hdr_dim;
    let uv        = pos.xy * 2.0 / hdr_dim;  // pos.xy / (hdr_dim * 0.5)

    let d = texel * 0.5;
    let c0 = karis(textureSample(pf_tex, pf_samp, uv + vec2<f32>(-d.x, -d.y)).rgb);
    let c1 = karis(textureSample(pf_tex, pf_samp, uv + vec2<f32>( d.x, -d.y)).rgb);
    let c2 = karis(textureSample(pf_tex, pf_samp, uv + vec2<f32>(-d.x,  d.y)).rgb);
    let c3 = karis(textureSample(pf_tex, pf_samp, uv + vec2<f32>( d.x,  d.y)).rgb);

    // Inverse Karis weight after averaging to recover linear-light values.
    var avg = (c0 + c1 + c2 + c3) * 0.25;
    avg = avg / (1.0 - luma(avg) + 1e-5);

    return vec4<f32>(apply_threshold(avg), 1.0);
}

// ── Blur bindings ──────────────────────────────────────────────────────────────
@group(0) @binding(0) var bl_tex:  texture_2d<f32>;
@group(0) @binding(1) var bl_samp: sampler;

// 9-tap gaussian kernel, sigma ≈ 3 texels.
// Weights computed as exp(-d² / (2·9)), normalised.
const W0: f32 = 0.0630;
const W1: f32 = 0.0929;
const W2: f32 = 0.1227;
const W3: f32 = 0.1449;
const W4: f32 = 0.1532;

fn gaussian9(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>, step: vec2<f32>) -> vec3<f32> {
    var acc = textureSample(tex, samp, uv - step * 4.0).rgb * W0;
    acc    += textureSample(tex, samp, uv - step * 3.0).rgb * W1;
    acc    += textureSample(tex, samp, uv - step * 2.0).rgb * W2;
    acc    += textureSample(tex, samp, uv - step      ).rgb * W3;
    acc    += textureSample(tex, samp, uv             ).rgb * W4;
    acc    += textureSample(tex, samp, uv + step      ).rgb * W3;
    acc    += textureSample(tex, samp, uv + step * 2.0).rgb * W2;
    acc    += textureSample(tex, samp, uv + step * 3.0).rgb * W1;
    acc    += textureSample(tex, samp, uv + step * 4.0).rgb * W0;
    return acc;
}

@fragment
fn fs_blur_h(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dim  = vec2<f32>(textureDimensions(bl_tex, 0));
    let uv   = pos.xy / dim;
    let step = vec2<f32>(1.0 / dim.x, 0.0);
    return vec4<f32>(gaussian9(bl_tex, bl_samp, uv, step), 1.0);
}

@fragment
fn fs_blur_v(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dim  = vec2<f32>(textureDimensions(bl_tex, 0));
    let uv   = pos.xy / dim;
    let step = vec2<f32>(0.0, 1.0 / dim.y);
    return vec4<f32>(gaussian9(bl_tex, bl_samp, uv, step), 1.0);
}
