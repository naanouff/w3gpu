use std::f32::consts::PI;
use w3drs_assets::HdrImage;

/// Diffuse IBL (convolution cos-weighted hemisphere). 32×32 donnait un filtrage trop grossier
/// (banding / fuites de couleur) ; 128×128 reste raisonnable en VRAM vs qualité perçue.
const IRRADIANCE_SIZE: u32 = 128;
const IRRADIANCE_SAMPLES: u32 = 1024;
/// Face size of the specular prefiltered env cubemap (mip0). Higher = sharper mirror IBL
/// at the cost of CPU bake time and VRAM (~6 × size² × rgba16 × ~1.33 mips).
const PREFILTERED_SIZE: u32 = 512;
const PREFILTERED_SAMPLES: u32 = 384;
const BRDF_LUT_SIZE: u32 = 256;
/// Aligné sur `w3dts/.../ibl/brdf.frag.wgsl` (`SAMPLE_COUNT = 1024`).
const BRDF_LUT_SAMPLES: u32 = 1024;
/// Full mip chain for `PREFILTERED_SIZE` (power of two).
const PREFILTERED_MIPS: u32 = PREFILTERED_SIZE.trailing_zeros() + 1;

/// Monte Carlo sample count for a prefilter mip: fewer on small mips (faster bake, sufficient variance).
#[inline]
fn prefiltered_samples_for_mip(mip_size: u32) -> u32 {
    let base = PREFILTERED_SAMPLES;
    let rel = mip_size as f32 / PREFILTERED_SIZE as f32;
    let scaled = (base as f32 * rel * rel).max(24.0).round() as u32;
    scaled.clamp(24, base)
}

/// IBL textures + sampler. The bind group (group 3) is built externally by the
/// engine, combined with the shadow map, to stay within `max_bind_groups = 4`.
pub struct IblContext {
    pub irradiance_texture: wgpu::Texture,
    pub irradiance_view: wgpu::TextureView,
    pub prefiltered_texture: wgpu::Texture,
    pub prefiltered_view: wgpu::TextureView,
    pub brdf_lut_texture: wgpu::Texture,
    pub brdf_lut_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl IblContext {
    /// Flat ambient fallback (no HDR): constant grey irradiance + pre-computed BRDF LUT.
    pub fn new_default(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // 1×1 grey irradiance/prefiltered cubemap (matches frame.ambient_intensity = 0.12)
        const AMB: f32 = 0.12;
        let grey_f16 = [
            f32_to_f16(AMB),
            f32_to_f16(AMB),
            f32_to_f16(AMB),
            f32_to_f16(1.0),
        ];
        let grey_bytes: Vec<u8> = grey_f16.iter().flat_map(|&h| h.to_le_bytes()).collect();

        let irr_tex = create_cubemap(device, 1, 1, 1);
        let pre_tex = create_cubemap(device, 1, 1, 1);
        for face in 0..6u32 {
            upload_cubemap_layer(queue, &irr_tex, face, 0, 1, 1, &grey_bytes);
            upload_cubemap_layer(queue, &pre_tex, face, 0, 1, 1, &grey_bytes);
        }

        let brdf_data = compute_brdf_lut(BRDF_LUT_SIZE, BRDF_LUT_SAMPLES);
        let brdf_tex = create_brdf_lut_texture(device, BRDF_LUT_SIZE);
        upload_brdf_lut(queue, &brdf_tex, &brdf_data, BRDF_LUT_SIZE);

        build_context(device, irr_tex, pre_tex, brdf_tex, 1)
    }

    /// Full IBL from an equirectangular HDR image.
    pub fn from_hdr(hdr: &HdrImage, device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        log::info!(
            "IBL: computing irradiance map ({}×{}×6)…",
            IRRADIANCE_SIZE,
            IRRADIANCE_SIZE
        );
        let irr_tex = create_cubemap(device, IRRADIANCE_SIZE, IRRADIANCE_SIZE, 1);
        #[cfg(not(target_arch = "wasm32"))]
        {
            use rayon::prelude::*;
            let faces: Vec<Vec<u8>> = (0..6usize)
                .into_par_iter()
                .map(|face| compute_irradiance_face(hdr, face, IRRADIANCE_SIZE, IRRADIANCE_SAMPLES))
                .collect();
            for face in 0..6usize {
                upload_cubemap_layer(
                    queue,
                    &irr_tex,
                    face as u32,
                    0,
                    IRRADIANCE_SIZE,
                    IRRADIANCE_SIZE,
                    &faces[face],
                );
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            for face in 0..6usize {
                let face_data =
                    compute_irradiance_face(hdr, face, IRRADIANCE_SIZE, IRRADIANCE_SAMPLES);
                upload_cubemap_layer(
                    queue,
                    &irr_tex,
                    face as u32,
                    0,
                    IRRADIANCE_SIZE,
                    IRRADIANCE_SIZE,
                    &face_data,
                );
            }
        }

        log::info!(
            "IBL: computing prefiltered env map ({}×{}×6×{} mips)…",
            PREFILTERED_SIZE,
            PREFILTERED_SIZE,
            PREFILTERED_MIPS
        );
        let pre_tex = create_cubemap(device, PREFILTERED_SIZE, PREFILTERED_SIZE, PREFILTERED_MIPS);
        for mip in 0..PREFILTERED_MIPS {
            let roughness = mip as f32 / (PREFILTERED_MIPS - 1).max(1) as f32;
            let mip_size = (PREFILTERED_SIZE >> mip).max(1);
            let mip_samples = prefiltered_samples_for_mip(mip_size);
            #[cfg(not(target_arch = "wasm32"))]
            {
                use rayon::prelude::*;
                let faces: Vec<Vec<u8>> = (0..6usize)
                    .into_par_iter()
                    .map(|face| {
                        compute_prefiltered_face(hdr, face, mip_size, roughness, mip_samples)
                    })
                    .collect();
                for face in 0..6usize {
                    upload_cubemap_layer(
                        queue,
                        &pre_tex,
                        face as u32,
                        mip,
                        mip_size,
                        mip_size,
                        &faces[face],
                    );
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                for face in 0..6usize {
                    let face_data =
                        compute_prefiltered_face(hdr, face, mip_size, roughness, mip_samples);
                    upload_cubemap_layer(
                        queue,
                        &pre_tex,
                        face as u32,
                        mip,
                        mip_size,
                        mip_size,
                        &face_data,
                    );
                }
            }
        }

        log::info!(
            "IBL: computing BRDF LUT ({}×{})…",
            BRDF_LUT_SIZE,
            BRDF_LUT_SIZE
        );
        let brdf_data = compute_brdf_lut(BRDF_LUT_SIZE, BRDF_LUT_SAMPLES);
        let brdf_tex = create_brdf_lut_texture(device, BRDF_LUT_SIZE);
        upload_brdf_lut(queue, &brdf_tex, &brdf_data, BRDF_LUT_SIZE);

        log::info!("IBL: done.");
        build_context(device, irr_tex, pre_tex, brdf_tex, IRRADIANCE_SIZE)
    }
}

// ── GPU resource helpers ───────────────────────────────────────────────────────

fn create_cubemap(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    mip_levels: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cubemap"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 6,
        },
        mip_level_count: mip_levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_brdf_lut_texture(device: &wgpu::Device, size: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("brdf_lut"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn upload_cubemap_layer(
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    face: u32,
    mip: u32,
    width: u32,
    height: u32,
    data: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: mip,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: face,
            },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 8), // rgba16float = 8 bytes/texel
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn upload_brdf_lut(queue: &wgpu::Queue, tex: &wgpu::Texture, data: &[u8], size: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size * 4), // rg16float = 4 bytes/texel
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
}

fn build_context(
    device: &wgpu::Device,
    irr_tex: wgpu::Texture,
    pre_tex: wgpu::Texture,
    brdf_tex: wgpu::Texture,
    irr_size: u32,
) -> IblContext {
    let irradiance_view = irr_tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        array_layer_count: Some(6),
        ..Default::default()
    });

    let prefiltered_view = pre_tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        array_layer_count: Some(6),
        mip_level_count: Some(pre_tex.mip_level_count()),
        ..Default::default()
    });

    let brdf_lut_view = brdf_tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("ibl sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: if irr_size > 1 {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        },
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    IblContext {
        irradiance_texture: irr_tex,
        irradiance_view,
        prefiltered_texture: pre_tex,
        prefiltered_view,
        brdf_lut_texture: brdf_tex,
        brdf_lut_view,
        sampler,
    }
}

// ── CPU IBL precomputation ─────────────────────────────────────────────────────

fn compute_irradiance_texel(
    hdr: &HdrImage,
    face: usize,
    size: u32,
    px: u32,
    py: u32,
    samples: u32,
) -> [f32; 3] {
    let n = face_direction(face, px, py, size);
    let (tangent, bitangent) = tangent_frame(n);
    let mut acc = [0.0f32; 3];
    for i in 0..samples {
        let xi = hammersley(i, samples);
        let phi = 2.0 * PI * xi[0];
        let cos_t = xi[1].sqrt();
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        let h_local = [sin_t * phi.cos(), sin_t * phi.sin(), cos_t];
        let dir = tbn_to_world(h_local, tangent, bitangent, n);
        let s = sample_equirect(hdr, dir);
        acc[0] += s[0];
        acc[1] += s[1];
        acc[2] += s[2];
    }
    let inv = 1.0 / samples as f32;
    [acc[0] * inv, acc[1] * inv, acc[2] * inv]
}

fn compute_irradiance_face(hdr: &HdrImage, face: usize, size: u32, samples: u32) -> Vec<u8> {
    let count = (size * size) as usize;
    let mut data = Vec::with_capacity(count * 8);
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        let rgb: Vec<[f32; 3]> = (0..size * size)
            .into_par_iter()
            .map(|i| {
                let px = i % size;
                let py = i / size;
                compute_irradiance_texel(hdr, face, size, px, py, samples)
            })
            .collect();
        for c in rgb {
            push_rgba16f(&mut data, c[0], c[1], c[2], 1.0);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        for py in 0..size {
            for px in 0..size {
                let c = compute_irradiance_texel(hdr, face, size, px, py, samples);
                push_rgba16f(&mut data, c[0], c[1], c[2], 1.0);
            }
        }
    }
    data
}

fn compute_prefiltered_texel(
    hdr: &HdrImage,
    face: usize,
    size: u32,
    px: u32,
    py: u32,
    roughness: f32,
    samples: u32,
) -> [f32; 3] {
    let n = face_direction(face, px, py, size);
    let v = n;
    let mut total_color = [0.0f32; 3];
    let mut total_weight = 0.0f32;
    for i in 0..samples {
        let xi = hammersley(i, samples);
        let h = importance_sample_ggx(xi, n, roughness);
        let vdoth = dot(v, h).max(0.0);
        let l = normalize([
            2.0 * vdoth * h[0] - v[0],
            2.0 * vdoth * h[1] - v[1],
            2.0 * vdoth * h[2] - v[2],
        ]);
        let ndotl = dot(n, l).max(0.0);
        if ndotl > 0.0 {
            let s = sample_equirect(hdr, l);
            total_color[0] += s[0] * ndotl;
            total_color[1] += s[1] * ndotl;
            total_color[2] += s[2] * ndotl;
            total_weight += ndotl;
        }
    }
    if total_weight > 0.0 {
        [
            total_color[0] / total_weight,
            total_color[1] / total_weight,
            total_color[2] / total_weight,
        ]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn compute_prefiltered_face(
    hdr: &HdrImage,
    face: usize,
    size: u32,
    roughness: f32,
    samples: u32,
) -> Vec<u8> {
    let count = (size * size) as usize;
    let mut data = Vec::with_capacity(count * 8);
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        let rgb: Vec<[f32; 3]> = (0..size * size)
            .into_par_iter()
            .map(|i| {
                let px = i % size;
                let py = i / size;
                compute_prefiltered_texel(hdr, face, size, px, py, roughness, samples)
            })
            .collect();
        for c in rgb {
            push_rgba16f(&mut data, c[0], c[1], c[2], 1.0);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        for py in 0..size {
            for px in 0..size {
                let c = compute_prefiltered_texel(hdr, face, size, px, py, roughness, samples);
                push_rgba16f(&mut data, c[0], c[1], c[2], 1.0);
            }
        }
    }
    data
}

/// Returns RGBA16F encoded as bytes but with only RG used (scale, bias for split-sum).
fn compute_brdf_lut(size: u32, samples: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize); // rg16float = 4 bytes
    for py in 0..size {
        let roughness = (py as f32 + 0.5) / size as f32;
        for px in 0..size {
            let ndotv = ((px as f32 + 0.5) / size as f32).max(1e-4);
            let v = [(1.0 - ndotv * ndotv).max(0.0).sqrt(), 0.0, ndotv];
            let n = [0.0f32, 0.0, 1.0];
            let mut a = 0.0f32;
            let mut b = 0.0f32;
            for i in 0..samples {
                let xi = hammersley(i, samples);
                let h = importance_sample_ggx(xi, n, roughness);
                let vdoth = dot(v, h).max(0.0);
                let l = normalize([
                    2.0 * vdoth * h[0] - v[0],
                    2.0 * vdoth * h[1] - v[1],
                    2.0 * vdoth * h[2] - v[2],
                ]);
                let ndotl = dot(n, l).max(0.0);
                let ndoth = dot(n, h).max(0.0);
                if ndotl > 0.0 {
                    // Same visibility + Jacobian as Filament / w3dts `ibl/brdf.frag.wgsl`
                    // (height-correlated Smith GGX), so split-sum matches direct `pbr.wgsl`.
                    let vis = v_smith_ggx_correlated(ndotv, ndotl, roughness);
                    let fc = (1.0 - vdoth).powi(5);
                    let ndotl_vis_pdf = ndotl * vis * (4.0 * vdoth / ndoth.max(1e-4));
                    a += (1.0 - fc) * ndotl_vis_pdf;
                    b += fc * ndotl_vis_pdf;
                }
            }
            let inv = 1.0 / samples as f32;
            push_rg16f(
                &mut data,
                (a * inv).clamp(0.0, 1.0),
                (b * inv).clamp(0.0, 1.0),
            );
        }
    }
    data
}

// ── math helpers ──────────────────────────────────────────────────────────────

#[inline]
fn hammersley(i: u32, n: u32) -> [f32; 2] {
    let bits = i.reverse_bits();
    let radical_inverse = bits as f32 * 2.328_306_4e-10; // 1/2^32
    [i as f32 / n as f32, radical_inverse]
}

fn importance_sample_ggx(xi: [f32; 2], n: [f32; 3], roughness: f32) -> [f32; 3] {
    let a = roughness * roughness;
    let a2 = a * a;
    let phi = 2.0 * PI * xi[0];
    let cos_theta = ((1.0 - xi[1]) / (1.0 + (a2 - 1.0) * xi[1])).max(0.0).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let h_local = [sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta];
    let (tangent, bitangent) = tangent_frame(n);
    normalize(tbn_to_world(h_local, tangent, bitangent, n))
}

fn tangent_frame(n: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let up = if n[1].abs() < 0.999 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    (t, b)
}

#[inline]
fn tbn_to_world(v: [f32; 3], t: [f32; 3], b: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    [
        t[0] * v[0] + b[0] * v[1] + n[0] * v[2],
        t[1] * v[0] + b[1] * v[1] + n[1] * v[2],
        t[2] * v[0] + b[2] * v[1] + n[2] * v[2],
    ]
}

fn face_direction(face: usize, px: u32, py: u32, size: u32) -> [f32; 3] {
    let u = (2.0 * (px as f32 + 0.5) / size as f32) - 1.0;
    let v = (2.0 * (py as f32 + 0.5) / size as f32) - 1.0;
    let dir = match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        5 => [-u, -v, -1.0],
        _ => unreachable!(),
    };
    normalize(dir)
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
    ]
}

/// Equirect HDR lookup with **bilinear** filtering (nearest-neighbour caused a coarse grid on IBL).
fn sample_equirect(hdr: &HdrImage, dir: [f32; 3]) -> [f32; 3] {
    let n = normalize(dir);
    let u = 0.5 + f32::atan2(n[2], n[0]) / (2.0 * PI);
    let v = 0.5 - f32::asin(n[1].clamp(-1.0, 1.0)) / PI;
    let w = hdr.width as f32;
    let h = hdr.height as f32;
    if hdr.width < 2 || hdr.height < 2 {
        let px = ((u * w).floor() as u32).min(hdr.width.saturating_sub(1));
        let py = ((v * h).floor() as u32).min(hdr.height.saturating_sub(1));
        return hdr.pixels[(py * hdr.width + px) as usize];
    }
    let u_c = u.clamp(1e-6, 1.0 - 1e-6);
    let v_c = v.clamp(1e-6, 1.0 - 1e-6);
    let fu = u_c * (w - 1.0);
    let fv = v_c * (h - 1.0);
    let x0 = fu.floor() as u32;
    let y0 = fv.floor() as u32;
    let x1 = (x0 + 1).min(hdr.width - 1);
    let y1 = (y0 + 1).min(hdr.height - 1);
    let tx = fu - x0 as f32;
    let ty = fv - y0 as f32;
    let idx = |x: u32, y: u32| (y * hdr.width + x) as usize;
    let c00 = hdr.pixels[idx(x0, y0)];
    let c10 = hdr.pixels[idx(x1, y0)];
    let c01 = hdr.pixels[idx(x0, y1)];
    let c11 = hdr.pixels[idx(x1, y1)];
    let c0 = lerp_rgb(c00, c10, tx);
    let c1 = lerp_rgb(c01, c11, tx);
    lerp_rgb(c0, c1, ty)
}

/// Height-correlated Smith GGX (Filament), matching `pbr.wgsl` `v_smith_ggx_correlated`.
#[inline]
fn v_smith_ggx_correlated(ndot_v: f32, ndot_l: f32, perceptual_roughness: f32) -> f32 {
    let alpha = perceptual_roughness * perceptual_roughness;
    let lambda_v =
        ndot_l * ((ndot_v - alpha * ndot_v) * ndot_v + alpha).max(0.0).sqrt();
    let lambda_l =
        ndot_v * ((ndot_l - alpha * ndot_l) * ndot_l + alpha).max(0.0).sqrt();
    0.5 / (lambda_v + lambda_l + 1e-7)
}

#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt();
    if len < 1e-7 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

// ── f16 helpers ───────────────────────────────────────────────────────────────

#[inline]
fn f32_to_f16(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = bits & 0x7fffff;
    if exp <= 0 {
        sign
    } else if exp >= 31 {
        sign | 0x7c00
    } else {
        sign | ((exp as u16) << 10) | (mant >> 13) as u16
    }
}

#[inline]
fn push_rgba16f(buf: &mut Vec<u8>, r: f32, g: f32, b: f32, a: f32) {
    for h in [f32_to_f16(r), f32_to_f16(g), f32_to_f16(b), f32_to_f16(a)] {
        buf.extend_from_slice(&h.to_le_bytes());
    }
}

#[inline]
fn push_rg16f(buf: &mut Vec<u8>, r: f32, g: f32) {
    for h in [f32_to_f16(r), f32_to_f16(g)] {
        buf.extend_from_slice(&h.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── f32_to_f16 ────────────────────────────────────────────────────────────

    #[test]
    fn f16_zero() {
        assert_eq!(f32_to_f16(0.0), 0x0000);
    }

    #[test]
    fn f16_one() {
        assert_eq!(f32_to_f16(1.0), 0x3C00);
    }

    #[test]
    fn f16_two() {
        assert_eq!(f32_to_f16(2.0), 0x4000);
    }

    #[test]
    fn f16_large_clamps_to_inf() {
        assert_eq!(f32_to_f16(1e10), 0x7C00);
    }

    #[test]
    fn f16_negative_one() {
        assert_eq!(f32_to_f16(-1.0), 0xBC00);
    }

    // ── hammersley ────────────────────────────────────────────────────────────

    #[test]
    fn hammersley_first_sample_zero_radical() {
        let h = hammersley(0, 8);
        assert_eq!(h[0], 0.0);
        assert!(h[1].abs() < 1e-7);
    }

    #[test]
    fn hammersley_second_sample() {
        let h = hammersley(1, 8);
        assert!((h[0] - 0.125).abs() < 1e-6);
        assert!((h[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn hammersley_values_in_0_1() {
        for i in 0..16 {
            let h = hammersley(i, 16);
            assert!(h[0] >= 0.0 && h[0] < 1.0);
            assert!(h[1] >= 0.0 && h[1] <= 1.0);
        }
    }

    // ── vector math ───────────────────────────────────────────────────────────

    #[test]
    fn dot_orthogonal_is_zero() {
        assert!((dot([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])).abs() < 1e-7);
    }

    #[test]
    fn dot_parallel_is_one() {
        assert!((dot([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]) - 1.0).abs() < 1e-7);
    }

    #[test]
    fn cross_x_y_gives_z() {
        let c = cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(c[0].abs() < 1e-7);
        assert!(c[1].abs() < 1e-7);
        assert!((c[2] - 1.0).abs() < 1e-7);
    }

    #[test]
    fn normalize_produces_unit_vector() {
        let v = normalize([3.0, 4.0, 0.0]);
        let len = dot(v, v).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_near_zero_is_safe() {
        let v = normalize([0.0, 0.0, 0.0]);
        let len = dot(v, v).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }

    // ── face_direction ────────────────────────────────────────────────────────

    #[test]
    fn face0_center_points_plus_x() {
        let d = face_direction(0, 0, 0, 1);
        assert!((d[0] - 1.0).abs() < 1e-5, "d={:?}", d);
        assert!(d[1].abs() < 1e-5);
        assert!(d[2].abs() < 1e-5);
    }

    #[test]
    fn face1_center_points_minus_x() {
        let d = face_direction(1, 0, 0, 1);
        assert!((d[0] + 1.0).abs() < 1e-5, "d={:?}", d);
    }

    #[test]
    fn face_directions_are_unit_length() {
        for face in 0..6 {
            let d = face_direction(face, 3, 3, 8);
            let len = dot(d, d).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "face={} len={}", face, len);
        }
    }

    // ── v_smith_ggx_correlated (BRDF LUT / direct) ─────────────────────────────

    #[test]
    fn v_smith_ggx_correlated_grazing_is_finite() {
        let v = v_smith_ggx_correlated(0.05, 0.05, 0.25);
        assert!(v.is_finite() && v > 0.0 && v < 1e6);
    }

    #[test]
    fn v_smith_ggx_correlated_rough_attenuates() {
        let v_smooth = v_smith_ggx_correlated(0.8, 0.8, 0.05);
        let v_rough = v_smith_ggx_correlated(0.8, 0.8, 0.95);
        assert!(v_rough < v_smooth, "rougher surface should shadow more");
    }

    // ── push helpers ──────────────────────────────────────────────────────────

    #[test]
    fn push_rgba16f_produces_8_bytes() {
        let mut buf = Vec::new();
        push_rgba16f(&mut buf, 1.0, 0.0, 0.0, 1.0);
        assert_eq!(buf.len(), 8);
        // first 2 bytes = f16(1.0) = 0x3C00 little-endian
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 0x3C00);
    }

    #[test]
    fn push_rg16f_produces_4_bytes() {
        let mut buf = Vec::new();
        push_rg16f(&mut buf, 1.0, 0.0);
        assert_eq!(buf.len(), 4);
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 0x3C00);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 0x0000);
    }
}
