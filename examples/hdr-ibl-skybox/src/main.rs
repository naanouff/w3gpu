//! Visualise la **même** HDR deux façons : fond équirectangulaire (skybox) + IBL sur une sphère
//! métallique très réfléchissante — utile pour comparer couleurs / orientation / fireflies entre
//! l’échantillonnage direct du fichier et la cubemap préfiltrée CPU (`IblContext::from_hdr_with_spec`).
//!
//! **Args** (comme `hdr-ibl-bench`) : `[--tier=max|high|medium|low|min] [chemin.hdr]` — HDR par défaut
//! `www/public/studio_small_03_2k.hdr` (racine workspace).
//!
//! **Debug** : **← / →** changent la vue plein écran (HDR caméra, faces préfiltre, faces
//! irradiance) ; **`[` / `]`** ajustent le mip affiché pour les vues préfiltre (1–6).
//! **`I`** active/désactive la **diffuse IBL** (cubemap d’irradiance) sur la sphère PBR.

use glam::{Mat4, Vec3, Vec4};
use pollster::FutureExt;
use std::mem::size_of;
use std::path::PathBuf;
use w3drs_assets::{load_hdr_from_bytes, primitives, HdrImage, Material};
use w3drs_ecs::{
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
    Scheduler, World,
};
use w3drs_renderer::{
    build_entity_list, camera_system, derive_shadow_batches, transform_system, AssetRegistry,
    BloomParams, CullPass, CullUniforms, DrawEntity, DrawIndexedIndirectArgs, FrameUniforms,
    GpuContext, HdrTarget, HizPass, IblContext, IblGenerationSpec, LightUniforms, MaterialTextures,
    PostProcessPass, RenderState, ShadowPass, TonemapParams, DEPTH_FORMAT, HDR_FORMAT,
    IBL_FLAG_DISABLE_IRRADIANCE_DIFFUSE,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const ENV_DEBUG_WGSL: &str = include_str!("../shaders/env_debug.wgsl");

/// Modes : 0 HDR sky, 1–6 préfiltre (faces), 7–12 irradiance (faces).
const DEBUG_VIEW_COUNT: u32 = 13;
const FACE_ABBREV: [&str; 6] = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];

fn debug_view_line(mode: u32, lod: f32) -> String {
    match mode {
        0 => "HDR équirect (caméra)".into(),
        1..=6 => format!(
            "Préfiltre face {} — mip {:.1}",
            FACE_ABBREV[(mode - 1) as usize],
            lod
        ),
        7..=12 => format!("Irradiance face {}", FACE_ABBREV[(mode - 7) as usize]),
        _ => "?".into(),
    }
}

fn main() {
    env_logger::init();
    let (ibl_spec, ibl_tier_display, hdr_path) = parse_skybox_cli();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        state: None,
        ibl_spec,
        ibl_tier_display,
        hdr_path,
    };
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    state: Option<State>,
    ibl_spec: IblGenerationSpec,
    ibl_tier_display: String,
    hdr_path: PathBuf,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let title = format!(
            "hdr-ibl-skybox — HDR sky + chrome sphere [ibl_tier={}]",
            self.ibl_tier_display
        );
        let window = event_loop
            .create_window(Window::default_attributes().with_title(title))
            .unwrap();
        self.state = Some(
            State::new(window, self.ibl_spec, self.hdr_path.clone()).block_on(),
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                match code {
                    KeyCode::ArrowLeft => {
                        state.debug_view =
                            (state.debug_view + DEBUG_VIEW_COUNT - 1) % DEBUG_VIEW_COUNT;
                    }
                    KeyCode::ArrowRight => {
                        state.debug_view = (state.debug_view + 1) % DEBUG_VIEW_COUNT;
                    }
                    KeyCode::BracketLeft => {
                        state.debug_prefilter_lod = (state.debug_prefilter_lod - 0.5).max(0.0);
                    }
                    KeyCode::BracketRight => {
                        state.debug_prefilter_lod = (state.debug_prefilter_lod + 0.5).min(24.0);
                    }
                    KeyCode::KeyI => {
                        state.disable_irradiance_diffuse_ibl =
                            !state.disable_irradiance_diffuse_ibl;
                    }
                    _ => {}
                }
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.context.resize(size.width, size.height);
                state
                    .hiz_pass
                    .resize(&state.context.device, size.width, size.height);
                state
                    .cull_pass
                    .rebuild_hiz_bg(&state.context.device, &state.hiz_pass.hiz_full_view);
                state
                    .hdr_target
                    .resize(&state.context.device, size.width, size.height);
                state.post_process.resize(
                    &state.context.device,
                    &state.hdr_target.view,
                    size.width,
                    size.height,
                );
                for e in state.world.query_entities::<CameraComponent>() {
                    if let Some(cam) = state.world.get_component_mut::<CameraComponent>(e) {
                        cam.aspect = size.width as f32 / size.height as f32;
                    }
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: btn,
                ..
            } => {
                state.mouse_pressed = btn == ElementState::Pressed;
                if !state.mouse_pressed {
                    state.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = (position.x as f32, position.y as f32);
                if state.mouse_pressed {
                    if let Some(last) = state.last_cursor {
                        state.orbit.drag(pos.0 - last.0, pos.1 - last.1);
                    }
                }
                state.last_cursor = Some(pos);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 0.75,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.025,
                };
                state.orbit.zoom(scroll);
            }
            WindowEvent::RedrawRequested => {
                state.tick();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Même convention que `hdr-ibl-bench` : `-t` / `--tier` / `--tier=name` puis chemin `.hdr` optionnel.
fn parse_skybox_cli() -> (IblGenerationSpec, String, PathBuf) {
    let default_hdr = workspace_root().join("www/public/studio_small_03_2k.hdr");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut tier_raw = "max".to_string();
    let mut path_opt: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        if a == "-t" || a == "--tier" {
            i += 1;
            if i < args.len() {
                tier_raw = args[i].clone();
            }
            i += 1;
            continue;
        }
        if let Some(t) = a.strip_prefix("--tier=") {
            tier_raw = t.to_string();
            i += 1;
            continue;
        }
        if path_opt.is_none() && !a.starts_with('-') {
            path_opt = Some(PathBuf::from(a));
        }
        i += 1;
    }
    let path = path_opt.unwrap_or(default_hdr);
    let tier_trim = tier_raw.trim();
    let spec = IblGenerationSpec::from_tier_name(tier_trim);
    let display = tier_trim.to_ascii_lowercase();
    (spec, display, path)
}

// ── Orbit camera ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
}

impl OrbitCamera {
    fn new(distance: f32, pitch: f32, yaw: f32, target: Vec3) -> Self {
        Self {
            yaw,
            pitch,
            distance,
            target,
        }
    }

    fn eye(&self) -> Vec3 {
        let y = self.distance * self.pitch.sin();
        let xz = self.distance * self.pitch.cos();
        self.target + Vec3::new(xz * self.yaw.sin(), y, xz * self.yaw.cos())
    }

    fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.005;
        self.pitch = (self.pitch + dy * 0.005).clamp(-1.5, 1.5);
    }

    fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(0.35, 120.0);
    }
}

// ── HDR equirect texture (RGBA16F) — même grille que le bake IBL côté CPU ─────

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

fn pack_hdr_rgba16f(hdr: &HdrImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(hdr.pixels.len() * 8);
    for [r, g, b] in &hdr.pixels {
        for h in [
            f32_to_f16(*r),
            f32_to_f16(*g),
            f32_to_f16(*b),
            f32_to_f16(1.0),
        ] {
            out.extend_from_slice(&h.to_le_bytes());
        }
    }
    out
}

fn upload_hdr_equirect_rgba16f(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    hdr: &HdrImage,
) -> (wgpu::Texture, wgpu::TextureView) {
    let data = pack_hdr_rgba16f(hdr);
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr equirect sky"),
        size: wgpu::Extent3d {
            width: hdr.width,
            height: hdr.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(hdr.width * 8),
            rows_per_image: Some(hdr.height),
        },
        wgpu::Extent3d {
            width: hdr.width,
            height: hdr.height,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&Default::default());
    (tex, view)
}

// ── Sky pass (fullscreen equirect) ───────────────────────────────────────────

#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyUniforms {
    inv_view_projection: [[f32; 4]; 4],
    camera_position: [f32; 3],
    _pad0: f32,
    view_mode: u32,
    _pad1: u32,
    prefilter_lod: f32,
    _pad2: f32,
}

struct SkyPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    textures_bind_group: wgpu::BindGroup,
}

impl SkyPass {
    fn new(device: &wgpu::Device, hdr_view: &wgpu::TextureView, ibl: &IblContext) -> Self {
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky frame layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(size_of::<SkyUniforms>() as u64).unwrap(),
                    ),
                },
                count: None,
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("env debug textures"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("env debug pipeline layout"),
            bind_group_layouts: &[&frame_layout, &tex_layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("env_debug"),
            source: wgpu::ShaderSource::Wgsl(ENV_DEBUG_WGSL.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky uniforms"),
            size: size_of::<SkyUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky frame bg"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });
        let textures_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("env debug textures bg"),
            layout: &tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&ibl.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&ibl.prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&ibl.irradiance_view),
                },
            ],
        });
        Self {
            pipeline,
            uniform_buf,
            frame_bind_group,
            textures_bind_group,
        }
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        inv_vp: Mat4,
        cam_pos: Vec3,
        view_mode: u32,
        prefilter_lod: f32,
    ) {
        let u = SkyUniforms {
            inv_view_projection: inv_vp.to_cols_array_2d(),
            camera_position: cam_pos.to_array(),
            _pad0: 0.0,
            view_mode,
            _pad1: 0,
            prefilter_lod,
            _pad2: 0.0,
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));
    }
}

// ── State ──────────────────────────────────────────────────────────────────────

struct State {
    window: Window,
    context: GpuContext,
    render_state: RenderState,
    asset_registry: AssetRegistry,
    /// Retenir les textures IBL (le bind group d’environnement ne possède que des vues).
    #[allow(dead_code)]
    ibl_context: IblContext,
    /// Texture 2D RGBA16F (même fichier que l’IBL) — le sky n’a que la vue.
    #[allow(dead_code)]
    hdr_equirect_tex: wgpu::Texture,
    sky_pass: SkyPass,
    shadow_pass: ShadowPass,
    env_bind_group: wgpu::BindGroup,
    hiz_pass: HizPass,
    cull_pass: CullPass,
    hdr_target: HdrTarget,
    post_process: PostProcessPass,
    camera_entity: u32,
    orbit: OrbitCamera,
    mouse_pressed: bool,
    last_cursor: Option<(f32, f32)>,
    total_time: f32,
    last_instant: std::time::Instant,
    world: World,
    scheduler: Scheduler,

    /// 0 = HDR sky ; 1–6 / 7–12 = faces préfiltre / irradiance (voir `DEBUG_VIEW_COUNT`).
    debug_view: u32,
    /// LOD affiché pour les modes préfiltre 1–6 (`[` / `]`).
    debug_prefilter_lod: f32,
    /// Si vrai : pas de `textureSample(irradiance_map)` dans le diffuse IBL (touche **I**).
    disable_irradiance_diffuse_ibl: bool,
}

impl State {
    async fn new(window: Window, ibl_spec: IblGenerationSpec, hdr_path: PathBuf) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(&window).unwrap();
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };
        let context = GpuContext::new(&instance, surface, size.width, size.height)
            .await
            .expect("GPU context creation failed");

        let render_state = RenderState::new(&context.device, context.surface_format);
        let mut asset_registry = AssetRegistry::new(&context.device, &context.queue);

        let mut world = World::new();
        let mut scheduler = Scheduler::new();
        scheduler
            .add_system(transform_system)
            .add_system(camera_system);

        let camera_entity = world.create_entity();
        world.add_component(
            camera_entity,
            CameraComponent::new(60.0, size.width as f32 / size.height as f32, 0.1, 300.0),
        );
        world.add_component(camera_entity, TransformComponent::default());

        let shadow_pass = ShadowPass::new(&context.device, &render_state.instance_bg_layout);

        let hdr_bytes = std::fs::read(&hdr_path).unwrap_or_else(|e| {
            panic!("Lecture HDR {} : {e}", hdr_path.display());
        });
        let hdr_image = load_hdr_from_bytes(&hdr_bytes)
            .unwrap_or_else(|e| panic!("parse HDR {} : {e}", hdr_path.display()));

        let (hdr_equirect_tex, hdr_equirect_view) =
            upload_hdr_equirect_rgba16f(&context.device, &context.queue, &hdr_image);

        log::info!(
            "IBL bake spec: irr={} pre0={} lut={} (file {})",
            ibl_spec.irradiance_size,
            ibl_spec.prefiltered_size,
            ibl_spec.brdf_lut_size,
            hdr_path.display()
        );
        let ibl_context =
            IblContext::from_hdr_with_spec(&hdr_image, &context.device, &context.queue, &ibl_spec);
        let sky_pass = SkyPass::new(&context.device, &hdr_equirect_view, &ibl_context);
        let env_bind_group = build_env_bind_group(
            &context.device,
            &render_state.ibl_bg_layout,
            &ibl_context,
            &shadow_pass,
        );

        let mut hiz_pass = HizPass::new(
            &context.device,
            &render_state.instance_bg_layout,
            size.width.max(1),
            size.height.max(1),
        );
        if size.width == 0 || size.height == 0 {
            hiz_pass.resize(&context.device, 800, 600);
        }
        let mut cull_pass = CullPass::new(&context.device);
        cull_pass.rebuild_hiz_bg(&context.device, &hiz_pass.hiz_full_view);

        let hdr_target = HdrTarget::new(&context.device, size.width.max(1), size.height.max(1));
        let post_process = PostProcessPass::new(
            &context.device,
            &hdr_target.view,
            context.surface_format,
            size.width.max(1),
            size.height.max(1),
            BloomParams {
                threshold: 1.0,
                knee: 0.5,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            TonemapParams {
                exposure: 1.0,
                bloom_strength: 0.0,
                flags: TonemapParams::FLAG_SKIP_FXAA,
                _pad1: 0.0,
            },
        );

        asset_registry.upload_material(
            &Material::default(),
            MaterialTextures::default(),
            &context.device,
            &render_state.material_bg_layout,
        );

        let sphere_mesh = primitives::uv_sphere(0.42, 56, 112);
        let mesh_id = asset_registry.upload_mesh(&sphere_mesh, &context.device, &context.queue);

        let chrome = Material {
            name: "chrome-debug".into(),
            albedo: [0.95, 0.95, 0.98, 1.0],
            metallic: 1.0,
            roughness: 0.035,
            ..Default::default()
        };
        let mat_id = asset_registry.upload_material(
            &chrome,
            MaterialTextures::default(),
            &context.device,
            &render_state.material_bg_layout,
        );

        let sphere_entity = world.create_entity();
        world.add_component(sphere_entity, RenderableComponent::new(mesh_id, mat_id));
        let mut t = TransformComponent::default();
        t.update_local_matrix();
        world.add_component(sphere_entity, t);

        let orbit = OrbitCamera::new(2.2, 0.18, 0.0, Vec3::ZERO);

        Self {
            window,
            context,
            render_state,
            asset_registry,
            ibl_context,
            hdr_equirect_tex,
            sky_pass,
            shadow_pass,
            env_bind_group,
            hiz_pass,
            cull_pass,
            hdr_target,
            post_process,
            camera_entity,
            orbit,
            mouse_pressed: false,
            last_cursor: None,
            total_time: 0.0,
            last_instant: std::time::Instant::now(),
            world,
            scheduler,
            debug_view: 0,
            debug_prefilter_lod: 0.0,
            disable_irradiance_diffuse_ibl: true,
        }
    }

    fn update_orbit_camera(&mut self) {
        let eye = self.orbit.eye();
        let target = self.orbit.target;
        let (_, rot, _) = Mat4::look_at_rh(eye, target, Vec3::Y)
            .inverse()
            .to_scale_rotation_translation();
        if let Some(t) = self
            .world
            .get_component_mut::<TransformComponent>(self.camera_entity)
        {
            t.position = eye;
            t.rotation = rot;
            t.update_local_matrix();
        }
    }

    fn tick(&mut self) {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;
        self.total_time += dt;

        self.update_orbit_camera();
        self.scheduler.run(&mut self.world, dt, self.total_time);

        let entities = collect_draw_entities(&self.world, &self.asset_registry);
        let (matrices, cull_data, sorted) = build_entity_list(entities);
        let entity_count = sorted.len() as u32;
        let shadow_batches = derive_shadow_batches(&sorted);

        if !matrices.is_empty() {
            self.context.queue.write_buffer(
                &self.render_state.instance_buffer,
                0,
                bytemuck::cast_slice(&matrices),
            );
        }
        if !cull_data.is_empty() {
            self.context.queue.write_buffer(
                &self.cull_pass.entity_cull_buf,
                0,
                bytemuck::cast_slice(&cull_data),
            );
        }

        let (view_proj, _) = camera_view_proj(&self.world);
        self.context.queue.write_buffer(
            &self.cull_pass.cull_uniform_buf,
            0,
            bytemuck::bytes_of(&CullUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                screen_size: [self.hiz_pass.width as f32, self.hiz_pass.height as f32],
                entity_count,
                mip_levels: self.hiz_pass.mip_count,
                cull_enabled: 0,
                _pad: [0u32; 3],
            }),
        );
        self.hiz_pass
            .update_camera(&self.context.queue, view_proj.to_cols_array_2d());

        let (view, projection, cam_pos) = world_active_camera(&self.world);
        let inv_vp = (projection * view).inverse();
        self.sky_pass.write_uniforms(
            &self.context.queue,
            inv_vp,
            cam_pos,
            self.debug_view,
            self.debug_prefilter_lod,
        );

        self.render(entity_count, &sorted, &shadow_batches);

        let irr = if self.disable_irradiance_diffuse_ibl {
            "irradiance IBL off"
        } else {
            "irradiance IBL on"
        };
        self.window.set_title(&format!(
            "hdr-ibl-skybox | {} ({}/{}) | {irr} [I] | [←/→] [ ] ] | LMB orbite",
            debug_view_line(self.debug_view, self.debug_prefilter_lod),
            self.debug_view + 1,
            DEBUG_VIEW_COUNT,
        ));
    }

    fn render(
        &self,
        entity_count: u32,
        sorted: &[DrawEntity],
        shadow_batches: &[w3drs_renderer::ShadowBatch],
    ) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("surface error: {e}");
                return;
            }
        };
        let view = output.texture.create_view(&Default::default());

        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer,
            0,
            bytemuck::bytes_of(&build_frame_uniforms(
                &self.world,
                self.total_time,
                self.disable_irradiance_diffuse_ibl,
            )),
        );
        self.shadow_pass
            .update_light(&self.context.queue, &build_light_uniforms());

        let indirect_stride = size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut enc = self
            .context
            .device
            .create_command_encoder(&Default::default());

        self.cull_pass.encode(&mut enc, entity_count);

        // Shadow map
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_pass.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rp.set_pipeline(&self.shadow_pass.depth_pipeline);
            rp.set_bind_group(0, &self.shadow_pass.shadow_light_bind_group, &[]);
            rp.set_bind_group(1, &self.render_state.instance_bind_group, &[]);
            for batch in shadow_batches {
                let Some(m) = self.asset_registry.get_mesh(batch.mesh_id) else {
                    continue;
                };
                rp.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                rp.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(
                    0..m.index_count,
                    0,
                    batch.first_instance..batch.first_instance + batch.instance_count,
                );
            }
        }

        // HDR main: clear → sky (equirect) → PBR sphere (IBL + direct)
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main hdr"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.03,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.context.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            rp.set_pipeline(&self.sky_pass.pipeline);
            rp.set_bind_group(0, &self.sky_pass.frame_bind_group, &[]);
            rp.set_bind_group(1, &self.sky_pass.textures_bind_group, &[]);
            rp.draw(0..3, 0..1);

            rp.set_pipeline(&self.render_state.pipeline);
            rp.set_bind_group(0, &self.render_state.frame_bind_group, &[]);
            rp.set_bind_group(1, &self.render_state.instance_bind_group, &[]);
            rp.set_bind_group(3, &self.env_bind_group, &[]);
            for (idx, entity) in sorted.iter().enumerate() {
                let mat = self
                    .asset_registry
                    .get_material(entity.material_id)
                    .or_else(|| self.asset_registry.get_material(0));
                let Some(mat) = mat else { continue };
                let Some(mesh) = self.asset_registry.get_mesh(entity.mesh_id) else {
                    continue;
                };
                rp.set_bind_group(2, &mat.bind_group, &[]);
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed_indirect(
                    &self.cull_pass.entity_indirect_buf,
                    idx as u64 * indirect_stride,
                );
            }
        }

        self.post_process.encode_tonemap_only(&mut enc, &view);

        self.context.queue.submit(std::iter::once(enc.finish()));
        output.present();
    }
}

fn collect_draw_entities(world: &World, registry: &AssetRegistry) -> Vec<DrawEntity> {
    let entities = world.query_entities::<RenderableComponent>();
    let mut result = Vec::with_capacity(entities.len());
    for entity in entities {
        if world.has_component::<CulledComponent>(entity) {
            continue;
        }
        let Some(r) = world.get_component::<RenderableComponent>(entity) else {
            continue;
        };
        if !r.visible {
            continue;
        }
        let world_matrix = world
            .get_component::<TransformComponent>(entity)
            .map(|t| t.world_matrix)
            .unwrap_or(Mat4::IDENTITY);
        let Some(mesh) = registry.get_mesh(r.mesh_id) else {
            continue;
        };
        let (aabb_min, aabb_max) = transform_aabb(
            &world_matrix,
            Vec3::from(mesh.aabb_min),
            Vec3::from(mesh.aabb_max),
        );
        result.push(DrawEntity {
            mesh_id: r.mesh_id,
            material_id: r.material_id,
            world_matrix: world_matrix.to_cols_array_2d(),
            cast_shadow: r.cast_shadow,
            aabb_min: aabb_min.to_array(),
            aabb_max: aabb_max.to_array(),
            first_index: 0,
            index_count: mesh.index_count,
            base_vertex: 0,
        });
    }
    result
}

fn transform_aabb(mat: &Mat4, local_min: Vec3, local_max: Vec3) -> (Vec3, Vec3) {
    let corners = [
        Vec3::new(local_min.x, local_min.y, local_min.z),
        Vec3::new(local_max.x, local_min.y, local_min.z),
        Vec3::new(local_min.x, local_max.y, local_min.z),
        Vec3::new(local_max.x, local_max.y, local_min.z),
        Vec3::new(local_min.x, local_min.y, local_max.z),
        Vec3::new(local_max.x, local_min.y, local_max.z),
        Vec3::new(local_min.x, local_max.y, local_max.z),
        Vec3::new(local_max.x, local_max.y, local_max.z),
    ];
    let ws: Vec<Vec3> = corners
        .iter()
        .map(|c| {
            let h = mat.mul_vec4(Vec4::new(c.x, c.y, c.z, 1.0));
            Vec3::new(h.x, h.y, h.z)
        })
        .collect();
    (
        ws.iter().copied().fold(Vec3::splat(f32::MAX), Vec3::min),
        ws.iter().copied().fold(Vec3::splat(f32::MIN), Vec3::max),
    )
}

fn camera_view_proj(world: &World) -> (Mat4, Mat4) {
    world
        .query_entities::<CameraComponent>()
        .into_iter()
        .find_map(|e| {
            let cam = world.get_component::<CameraComponent>(e)?;
            if cam.is_active {
                Some((cam.view_matrix, cam.projection_matrix))
            } else {
                None
            }
        })
        .unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY))
}

fn world_active_camera(world: &World) -> (Mat4, Mat4, Vec3) {
    world
        .query_entities::<CameraComponent>()
        .into_iter()
        .find_map(|e| {
            let cam = world.get_component::<CameraComponent>(e)?;
            if !cam.is_active {
                return None;
            }
            let pos = world
                .get_component::<TransformComponent>(e)
                .map(|t| {
                    let w = t.world_matrix.w_axis;
                    Vec3::new(w.x, w.y, w.z)
                })
                .unwrap_or(Vec3::ZERO);
            Some((cam.view_matrix, cam.projection_matrix, pos))
        })
        .unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO))
}

fn build_light_uniforms() -> LightUniforms {
    let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();
    let light_pos = -light_dir * 30.0;
    let light_view = Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = Mat4::orthographic_rh(-25.0, 25.0, -25.0, 25.0, 0.1, 80.0);
    LightUniforms {
        view_proj: (light_proj * light_view).to_cols_array_2d(),
        shadow_bias: 0.001,
        _pad: [0.0; 3],
    }
}

fn build_frame_uniforms(
    world: &World,
    total_time: f32,
    disable_irradiance_diffuse_ibl: bool,
) -> FrameUniforms {
    let (view, projection, cam_pos) = world
        .query_entities::<CameraComponent>()
        .into_iter()
        .find_map(|e| {
            let cam = world.get_component::<CameraComponent>(e)?;
            if !cam.is_active {
                return None;
            }
            let pos = world
                .get_component::<TransformComponent>(e)
                .map(|t| {
                    let w = t.world_matrix.w_axis;
                    Vec3::new(w.x, w.y, w.z)
                })
                .unwrap_or(Vec3::ZERO);
            Some((cam.view_matrix, cam.projection_matrix, pos))
        })
        .unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO));
    let inv_vp = (projection * view).inverse();
    let light = build_light_uniforms();
    let ibl_flags = if disable_irradiance_diffuse_ibl {
        IBL_FLAG_DISABLE_IRRADIANCE_DIFFUSE
    } else {
        0
    };
    FrameUniforms {
        projection: projection.to_cols_array_2d(),
        view: view.to_cols_array_2d(),
        inv_view_projection: inv_vp.to_cols_array_2d(),
        camera_position: cam_pos.to_array(),
        _pad0: 0.0,
        light_direction: Vec3::new(-0.5, -1.0, -0.5).normalize().to_array(),
        _pad1: 0.0,
        light_color: [1.0, 0.95, 0.9],
        ambient_intensity: 0.12,
        total_time,
        _pad2: [0.0; 3],
        light_view_proj: light.view_proj,
        shadow_bias: light.shadow_bias,
        ibl_flags,
        ibl_diffuse_scale: 0.1,
        _pad3: 0.0,
    }
}

fn build_env_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    ibl: &IblContext,
    shadow: &ShadowPass,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("env bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&ibl.irradiance_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&ibl.prefiltered_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&ibl.brdf_lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&ibl.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&shadow.shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&shadow.comparison_sampler),
            },
        ],
    })
}
