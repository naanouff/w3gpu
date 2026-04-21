// Tonemap + FXAA composite pass.
// Inputs: HDR scene colour (RGBA16Float) + bloom texture (half-res RGBA16Float).
// Output: LDR sRGB to the swapchain.
//
// Entry points:
//   vs_fullscreen — shared fullscreen triangle (no VBO)
//   fs_tonemap    — ACES Narkowicz + additive bloom + FXAA 3x3

// ── Vertex shader ─────────────────────────────────────────────────────────────
@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32(vi & 1u) * 4.0 - 1.0;
    let y = f32((vi >> 1u) & 1u) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// ── Bindings ───────────────────────────────────────────────────────────────────
@group(0) @binding(0) var hdr_tex:   texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var s_linear:  sampler;
@group(0) @binding(3) var<uniform>   params: TonemapParams;

struct TonemapParams {
    exposure:       f32,
    bloom_strength: f32,
    flags:          u32,
    _pad1:          f32,
}

const FLAG_SKIP_FXAA: u32 = 1u;

// ── ACES Narkowicz approximation ───────────────────────────────────────────────
fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

// ── Linear → sRGB gamma ────────────────────────────────────────────────────────
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// ── Luma for FXAA ─────────────────────────────────────────────────────────────
fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// ── Tonemap + FXAA fragment ────────────────────────────────────────────────────
@fragment
fn fs_tonemap(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let hdr_dim   = vec2<f32>(textureDimensions(hdr_tex, 0));
    let uv        = pos.xy / hdr_dim;
    let texel     = 1.0 / hdr_dim;

    // Additive bloom
    let bloom = textureSample(bloom_tex, s_linear, uv).rgb;
    let hdr   = textureSample(hdr_tex,  s_linear, uv).rgb;
    var color = hdr + bloom * params.bloom_strength;

    // Exposure
    color *= params.exposure;

    // ACES tone mapping
    color = aces(color);

    if ((params.flags & FLAG_SKIP_FXAA) != 0u) {
        return vec4<f32>(linear_to_srgb(color), 1.0);
    }

    // FXAA — 3×3 luma neighbourhood, edge-blend in LDR space.
    // Tonemapped neighbours (without bloom to avoid double-sampling artefacts).
    let n  = aces(textureSample(hdr_tex, s_linear, uv + vec2<f32>( 0.0, -texel.y)).rgb * params.exposure);
    let s2 = aces(textureSample(hdr_tex, s_linear, uv + vec2<f32>( 0.0,  texel.y)).rgb * params.exposure);
    let w  = aces(textureSample(hdr_tex, s_linear, uv + vec2<f32>(-texel.x,  0.0)).rgb * params.exposure);
    let e  = aces(textureSample(hdr_tex, s_linear, uv + vec2<f32>( texel.x,  0.0)).rgb * params.exposure);

    let lM = luma(color);
    let lN = luma(n);
    let lS = luma(s2);
    let lW = luma(w);
    let lE = luma(e);

    let lMin = min(lM, min(min(lN, lS), min(lW, lE)));
    let lMax = max(lM, max(max(lN, lS), max(lW, lE)));
    let contrast = lMax - lMin;

    // Only blend where contrast is meaningful.
    var blended = color;
    if contrast > max(0.0312, lMax * 0.125) {
        let blend = clamp(contrast * 4.0, 0.0, 1.0) * 0.5;
        blended = mix(color, (n + s2 + w + e) * 0.25, blend);
    }

    return vec4<f32>(linear_to_srgb(blended), 1.0);
}
