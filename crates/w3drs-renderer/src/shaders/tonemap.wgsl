// Tonemap + FXAA composite pass.
// Inputs: HDR scene colour (RGBA16Float) + bloom texture (half-res RGBA16Float).
// Output: LDR linear to sRGB swapchain (hardware applies gamma via Bgra8UnormSrgb).
//
// Entry points:
//   vs_fullscreen — shared fullscreen triangle (no VBO)
//   fs_tonemap    — ACES Narkowicz + additive bloom + FXAA Quality 12

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

// NOTE: linear_to_srgb is intentionally absent.
// The swapchain surface is Bgra8UnormSrgb / Rgba8UnormSrgb — the GPU applies the
// sRGB transfer function automatically when we write linear values to the attachment.
// Applying an additional software gamma here would cause double gamma correction.

// ── FXAA Quality 12 — adapted from FXAA 3.11 (NVIDIA, Timothy Lottes) ─────────
// Operates on LDR tone-mapped signal. 9 base samples + up to 16 edge-search samples.
//
// Improvements over the previous 3×3 blur:
//  • Detects edge direction (horizontal vs vertical) with a Sobel-like filter.
//  • Searches along the edge in both directions (8 steps, increasing stride).
//  • Computes a proper sub-pixel blend factor for single-pixel aliasing.
//  • Only samples the geometry of the alias, not a uniform blur.

const FXAA_EDGE_THRESHOLD:     f32 = 0.166;   // min local contrast to activate
const FXAA_EDGE_THRESHOLD_MIN: f32 = 0.0625;  // skip very dark/flat regions
const FXAA_SUBPIX:             f32 = 0.75;    // sub-pixel correction strength

fn fxaa_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// Sample hdr_tex at given UV (explicit LOD=0 — allowed in non-uniform control flow).
fn fxaa_tap(uv: vec2<f32>, exp: f32) -> vec3<f32> {
    return aces(textureSampleLevel(hdr_tex, s_linear, uv, 0.0).rgb * exp);
}

fn fxaa_quality(uv: vec2<f32>, rcpDim: vec2<f32>, exp: f32, colorM: vec3<f32>) -> vec3<f32> {
    let lM = fxaa_luma(colorM);

    // 4 cardinal neighbours — used for early exit and sub-pixel factor
    let lN = fxaa_luma(fxaa_tap(uv + vec2( 0.0,       -rcpDim.y), exp));
    let lS = fxaa_luma(fxaa_tap(uv + vec2( 0.0,        rcpDim.y), exp));
    let lE = fxaa_luma(fxaa_tap(uv + vec2( rcpDim.x,  0.0      ), exp));
    let lW = fxaa_luma(fxaa_tap(uv + vec2(-rcpDim.x,  0.0      ), exp));

    let lMin = min(lM, min(min(lN, lS), min(lE, lW)));
    let lMax = max(lM, max(max(lN, lS), max(lE, lW)));
    let range = lMax - lMin;

    // Early exit: flat or dark region — no aliasing to fix
    if range < max(FXAA_EDGE_THRESHOLD_MIN, lMax * FXAA_EDGE_THRESHOLD) {
        return colorM;
    }

    // 4 diagonal neighbours for edge direction detection
    let lNW = fxaa_luma(fxaa_tap(uv + vec2(-rcpDim.x, -rcpDim.y), exp));
    let lNE = fxaa_luma(fxaa_tap(uv + vec2( rcpDim.x, -rcpDim.y), exp));
    let lSW = fxaa_luma(fxaa_tap(uv + vec2(-rcpDim.x,  rcpDim.y), exp));
    let lSE = fxaa_luma(fxaa_tap(uv + vec2( rcpDim.x,  rcpDim.y), exp));

    // Sub-pixel aliasing: single-pixel-wide features (e.g. thin lines, text)
    let lAvg4   = (lN + lS + lE + lW) * 0.25;
    let subBlend = clamp((abs(lAvg4 - lM) / range) * 2.0 - 0.5, 0.0, 1.0);
    let subBlend2 = subBlend * subBlend * FXAA_SUBPIX;

    // Sobel-like edge direction:
    //   edgeH large → brightness varies top-to-bottom → horizontal edge
    //   edgeV large → brightness varies left-to-right → vertical edge
    let edgeH = abs(-2.0*lW + lNW + lSW) + abs(-2.0*lM + lN + lS)*2.0 + abs(-2.0*lE + lNE + lSE);
    let edgeV = abs(-2.0*lN + lNW + lNE) + abs(-2.0*lM + lW + lE)*2.0 + abs(-2.0*lS + lSW + lSE);
    // horzEdge=true: horizontal edge → search along X, apply offset in Y
    let horzEdge = edgeH >= edgeV;

    // Perpendicular gradient: which side of center is the stronger step?
    let lPerpP = select(lS, lE, horzEdge);  // south if H-edge, east if V-edge
    let lPerpN = select(lN, lW, horzEdge);  // north if H-edge, west if V-edge
    let gradP = abs(lPerpP - lM);
    let gradN = abs(lPerpN - lM);
    let perpIsPos = gradP >= gradN;          // dominant gradient toward +Y (S) or +X (E)?

    // Step sizes in UV space
    let perpStep   = select(rcpDim.y, rcpDim.x, horzEdge);  // Y-step if H, X-step if V
    let perpSign   = select(-1.0, 1.0, perpIsPos);
    // Search direction: along the edge
    let searchDir  = select(vec2(0.0, rcpDim.y), vec2(rcpDim.x, 0.0), horzEdge);

    // Start at edge midpoint (half pixel toward dominant perpendicular side)
    let uvEdge     = uv + select(
        vec2(0.0, perpSign * perpStep * 0.5),   // H-edge: offset in Y
        vec2(perpSign * perpStep * 0.5, 0.0),   // V-edge: offset in X
        horzEdge,
    );
    let lumaEdge   = select((lM + lPerpN) * 0.5, (lM + lPerpP) * 0.5, perpIsPos);
    let scaledGrad = max(gradP, gradN) * 0.25;

    // Edge endpoint search — 8 steps each direction with increasing stride
    // Steps: 1,1,1,1,1.5,2,4,8 pixels (quality 12 preset)
    var uvP = uvEdge + searchDir;
    var uvN = uvEdge - searchDir;
    var ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge;
    var ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge;
    var doneP = abs(ldP) >= scaledGrad;
    var doneN = abs(ldN) >= scaledGrad;

    if !doneP { uvP += searchDir;       ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; doneP = abs(ldP) >= scaledGrad; }
    if !doneN { uvN -= searchDir;       ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; doneN = abs(ldN) >= scaledGrad; }
    if !doneP { uvP += searchDir;       ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; doneP = abs(ldP) >= scaledGrad; }
    if !doneN { uvN -= searchDir;       ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; doneN = abs(ldN) >= scaledGrad; }
    if !doneP { uvP += searchDir;       ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; doneP = abs(ldP) >= scaledGrad; }
    if !doneN { uvN -= searchDir;       ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; doneN = abs(ldN) >= scaledGrad; }
    if !doneP { uvP += searchDir * 1.5; ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; doneP = abs(ldP) >= scaledGrad; }
    if !doneN { uvN -= searchDir * 1.5; ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; doneN = abs(ldN) >= scaledGrad; }
    if !doneP { uvP += searchDir * 2.0; ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; doneP = abs(ldP) >= scaledGrad; }
    if !doneN { uvN -= searchDir * 2.0; ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; doneN = abs(ldN) >= scaledGrad; }
    if !doneP { uvP += searchDir * 4.0; ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; doneP = abs(ldP) >= scaledGrad; }
    if !doneN { uvN -= searchDir * 4.0; ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; doneN = abs(ldN) >= scaledGrad; }
    if !doneP { uvP += searchDir * 8.0; ldP = fxaa_luma(fxaa_tap(uvP, exp)) - lumaEdge; }
    if !doneN { uvN -= searchDir * 8.0; ldN = fxaa_luma(fxaa_tap(uvN, exp)) - lumaEdge; }

    // Distance from uvEdge to each search endpoint (along the search axis)
    let distP   = select(abs(uvP.y - uvEdge.y), abs(uvP.x - uvEdge.x), horzEdge);
    let distN   = select(abs(uvN.y - uvEdge.y), abs(uvN.x - uvEdge.x), horzEdge);
    let spanLen = distP + distN;
    let distMin = min(distP, distN);

    // Is the center pixel on the wrong side of the edge luma at the nearest endpoint?
    // If yes, blending moves us toward the correct side.
    let lML      = lM - lumaEdge;
    let endPGood = (ldP < 0.0) != (lML < 0.0);
    let endNGood = (ldN < 0.0) != (lML < 0.0);
    let nearestGood = select(endNGood, endPGood, distP < distN);

    let edgeBlend  = select(0.0, 0.5 - distMin / max(spanLen, 0.0001), nearestGood);
    let finalBlend = max(subBlend2, edgeBlend);

    // Apply perpendicular offset: sample toward the edge by `finalBlend` pixels
    let pixOff  = finalBlend * perpStep * perpSign;
    let uvFinal = uv + select(
        vec2(0.0, pixOff),  // H-edge: offset in Y
        vec2(pixOff, 0.0),  // V-edge: offset in X
        horzEdge,
    );

    return fxaa_tap(uvFinal, exp);
}

// ── Tonemap + FXAA fragment ────────────────────────────────────────────────────
@fragment
fn fs_tonemap(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let hdr_dim = vec2<f32>(textureDimensions(hdr_tex, 0));
    let rcpDim  = 1.0 / hdr_dim;
    let uv      = pos.xy * rcpDim;

    // Additive bloom (low-frequency, sampled with bilinear)
    let bloom  = textureSample(bloom_tex, s_linear, uv).rgb;
    let hdr    = textureSample(hdr_tex,   s_linear, uv).rgb;
    let colorM = aces((hdr + bloom * params.bloom_strength) * params.exposure);

    if ((params.flags & FLAG_SKIP_FXAA) != 0u) {
        return vec4<f32>(colorM, 1.0);
    }

    let result = fxaa_quality(uv, rcpDim, params.exposure, colorM);
    return vec4<f32>(result, 1.0);
}
