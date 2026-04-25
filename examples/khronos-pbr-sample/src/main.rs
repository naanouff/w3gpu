use glam::{Vec3, Vec4};
use pollster::FutureExt;
use std::mem::size_of;
use std::path::PathBuf;
use w3drs_assets::{
    load_from_bytes, load_hdr_from_bytes, load_phase_a_viewer_config_or_default, GltfPrimitive,
    Material, PhaseAViewerConfig,
};
use w3drs_ecs::{
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
    Scheduler, World,
};
use w3drs_render_graph::RenderGraphDocument;
use w3drs_renderer::{
    build_entity_list, camera_system, derive_shadow_batches, encode_render_graph_passes_v0,
    encode_render_graph_passes_v0_with_wgsl_host, parse_render_graph_json, transform_system,
    validate_render_graph_exec_v0, AssetRegistry, BloomParams, CullPass, CullUniforms, DrawEntity,
    DrawIndexedIndirectArgs, FrameUniforms, GpuContext, HdrTarget, HizPass, IblContext,
    IblGenerationSpec, LightUniforms, MaterialTextures, PostProcessPass, RenderGraphExecError,
    RenderGraphGpuRegistry, RenderGraphV0Host, RenderState, ShadowBatch, ShadowPass, Texture2dGpu,
    TonemapParams, MAX_CULL_ENTITIES, SHADOW_SIZE,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

/// Où insérer le sous-graphe déclaratif Phase B dans le `CommandEncoder` (B.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RenderGraphSlot {
    /// Avant Hi-Z / cull (premières commandes de la frame).
    PreFrame,
    /// Après cull + copie indirect → readback (défaut, ordre historique).
    #[default]
    AfterCullReadback,
    /// Après le main PBR sur `hdr_target`, avant post-process / swapchain.
    PostPbr,
}

fn parse_render_graph_slot(s: &str) -> Option<RenderGraphSlot> {
    match s.to_ascii_lowercase().as_str() {
        "pre" | "pre_frame" => Some(RenderGraphSlot::PreFrame),
        "after_cull" | "cull" => Some(RenderGraphSlot::AfterCullReadback),
        "post_pbr" | "post" | "post_hdr" => Some(RenderGraphSlot::PostPbr),
        _ => None,
    }
}

fn main() {
    env_logger::init();
    let (render_graph_json, render_graph_readback, render_graph_slot) = parse_render_graph_cli();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        state: None,
        render_graph_json,
        render_graph_readback,
        render_graph_slot,
    };
    event_loop.run_app(&mut app).unwrap();
}

/// `--render-graph PATH`, `--render-graph-readback ID` (défaut `hdr_color`),
/// `--render-graph-slot pre|after_cull|post_pbr` (défaut `after_cull`).
/// Avec un graphe, l’**ombre** est aussi data-driven (B.7) : `render_graph_shadow_khronos.json` +
/// `fixtures/phases/phase-b/shaders/shadow_depth.wgsl`, registre câblé sur
/// `ShadowPass` + buffer d’instances.
fn parse_render_graph_cli() -> (Option<PathBuf>, String, RenderGraphSlot) {
    let mut it = std::env::args();
    let mut json: Option<PathBuf> = None;
    let mut readback = "hdr_color".to_string();
    let mut slot = RenderGraphSlot::default();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--render-graph" => {
                json = it.next().map(PathBuf::from);
            }
            "--render-graph-readback" => {
                if let Some(id) = it.next() {
                    readback = id;
                }
            }
            "--render-graph-slot" => {
                if let Some(s) = it.next() {
                    if let Some(sl) = parse_render_graph_slot(&s) {
                        slot = sl;
                    } else {
                        log::warn!(
                            "unknown --render-graph-slot {s:?}, using after_cull (pre|after_cull|post_pbr)"
                        );
                    }
                }
            }
            _ => {}
        }
    }
    (json, readback, slot)
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

/// Désactive le pré-pass Hi‑Z + cull GPU (les sphères derrière le sol étaient trop souvent occlues).
const VIEWER_GPU_OCCLUSION: bool = false;
/// Désactive bloom + flous ; tonemap ACES + sortie (FXAA désactivé via `TonemapParams::flags`).
const VIEWER_FULL_BLOOM_POST: bool = false;

/// Sept GLB de référence Phase A / Khronos (chemins relatifs à la racine du workspace).
const KHRONOS_GLBS: &[(&str, &str)] = &[
    ("DamagedHelmet", "www/public/damaged_helmet_source_glb.glb"),
    (
        "AnisotropyBarnLamp",
        "fixtures/phases/phase-a/glb/AnisotropyBarnLamp.glb",
    ),
    (
        "ClearCoatCarPaint",
        "fixtures/phases/phase-a/glb/ClearCoatCarPaint.glb",
    ),
    (
        "ClearcoatWicker",
        "fixtures/phases/phase-a/glb/ClearcoatWicker.glb",
    ),
    ("IORTestGrid", "fixtures/phases/phase-a/glb/IORTestGrid.glb"),
    (
        "TextureTransformTest",
        "fixtures/phases/phase-a/glb/TextureTransformTest.glb",
    ),
    (
        "MetalRoughSpheres",
        "fixtures/phases/phase-a/glb/MetalRoughSpheres.glb",
    ),
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn bounds_from_primitives(primitives: &[GltfPrimitive]) -> (Vec3, f32) {
    if primitives.is_empty() {
        return (Vec3::ZERO, 2.0);
    }
    let mut mn = Vec3::splat(f32::MAX);
    let mut mx = Vec3::splat(f32::MIN);
    for p in primitives {
        mn = mn.min(p.mesh.aabb.min);
        mx = mx.max(p.mesh.aabb.max);
    }
    let center = (mn + mx) * 0.5;
    let radius = ((mx - mn).length() * 0.5).max(0.15);
    (center, radius)
}

// ── App ────────────────────────────────────────────────────────────────────────

struct App {
    state: Option<State>,
    render_graph_json: Option<PathBuf>,
    render_graph_readback: String,
    render_graph_slot: RenderGraphSlot,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes().with_title("khronos-pbr-sample — Phase A GLB viewer"),
            )
            .unwrap();
        self.state = Some(
            State::new(
                window,
                self.render_graph_json.clone(),
                self.render_graph_readback.clone(),
                self.render_graph_slot,
            )
            .block_on(),
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

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

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        repeat,
                        ..
                    },
                ..
            } => match code {
                KeyCode::ArrowLeft if !repeat => state.prev_sample(),
                KeyCode::ArrowRight if !repeat => state.next_sample(),
                _ => {}
            },

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

// ── State ──────────────────────────────────────────────────────────────────────

/// Sous-graphe B.7 (ombre) : mêmes GPU buffers que le viewer, encodé à l’étape « shadow ».
struct ShadowGraphInViewer {
    doc: RenderGraphDocument,
    registry: RenderGraphGpuRegistry,
    shader_root: PathBuf,
}

/// Optional Phase B JSON graph (own textures/buffers), validated at init; encoded each frame.
struct PhaseBRenderGraphHook {
    doc: RenderGraphDocument,
    registry: RenderGraphGpuRegistry,
    shader_root: PathBuf,
    /// Ombres en `raster_depth_mesh` (remplace le render pass manuel) — requiert le même
    /// [`workspace_root`]/`fixtures/phases/phase-b/shaders/shadow_depth.wgsl` que l’hôte moteur.
    shadow: ShadowGraphInViewer,
}

/// Hôte B.7 : même boucle d’`draw_indexed` qu’avant, dans le `RenderPass` ouvert par le graphe.
struct KhronosShadowHost<'a> {
    asset_registry: &'a AssetRegistry,
    shadow_batches: &'a [ShadowBatch],
}

impl RenderGraphV0Host for KhronosShadowHost<'_> {
    fn draw_raster_depth_mesh(&mut self, _pass_id: &str, rpass: &mut wgpu::RenderPass<'_>) {
        for batch in self.shadow_batches {
            let Some(m) = self.asset_registry.get_mesh(batch.mesh_id) else {
                continue;
            };
            rpass.set_vertex_buffer(0, m.vertex_buffer.slice(..));
            rpass.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(
                0..m.index_count,
                0,
                batch.first_instance..batch.first_instance + batch.instance_count,
            );
        }
    }
}

fn texture2d_gpu_shadow_map(sp: &ShadowPass) -> Texture2dGpu {
    Texture2dGpu {
        texture: sp.shadow_texture.clone(),
        view: sp.shadow_view.clone(),
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        width: SHADOW_SIZE,
        height: SHADOW_SIZE,
        mip_level_count: 1,
    }
}

struct State {
    window: Window,
    context: GpuContext,
    render_state: RenderState,
    /// `fixtures/phases/phase-a/materials/default.json` (data-driven, Phase A).
    phase_a_viewer: PhaseAViewerConfig,
    asset_registry: AssetRegistry,
    #[allow(dead_code)]
    ibl_context: IblContext,
    shadow_pass: ShadowPass,
    env_bind_group: wgpu::BindGroup,
    hiz_pass: HizPass,
    cull_pass: CullPass,
    hdr_target: HdrTarget,
    post_process: PostProcessPass,

    // Camera
    camera_entity: u32,
    orbit: OrbitCamera,
    mouse_pressed: bool,
    last_cursor: Option<(f32, f32)>,

    sample_idx: usize,
    model_entities: Vec<u32>,

    // Readback + metrics
    readback_buf: wgpu::Buffer,
    last_hiz_visible: u32,
    potential_count: u32,
    frustum_visible: u32,

    total_time: f32,
    last_instant: std::time::Instant,
    world: World,
    scheduler: Scheduler,

    /// Phase B.4: declarative passes composed into the viewer command buffer (no readback).
    phase_b_render_graph: Option<PhaseBRenderGraphHook>,
    /// Emplacement d’encodage du graphe (voir [`RenderGraphSlot`]) — ignoré si pas de `--render-graph`.
    render_graph_slot: RenderGraphSlot,
}

impl State {
    async fn new(
        window: Window,
        render_graph_json: Option<PathBuf>,
        render_graph_readback: String,
        render_graph_slot: RenderGraphSlot,
    ) -> Self {
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
        let asset_registry = AssetRegistry::new(&context.device, &context.queue);

        // ── ECS ───────────────────────────────────────────────────────────────
        let mut world = World::new();
        let mut scheduler = Scheduler::new();
        scheduler
            .add_system(transform_system)
            .add_system(camera_system);

        // Camera entity
        let camera_entity = world.create_entity();
        world.add_component(
            camera_entity,
            CameraComponent::new(60.0, size.width as f32 / size.height as f32, 0.1, 300.0),
        );
        world.add_component(camera_entity, TransformComponent::default());

        // ── Shadow pass ───────────────────────────────────────────────────────
        let shadow_pass = ShadowPass::new(&context.device, &render_state.instance_bg_layout);

        let workspace = workspace_root();
        let phase_a_viewer = load_phase_a_viewer_config_or_default(
            &workspace.join("fixtures/phases/phase-a/materials/default.json"),
        );
        let pav = phase_a_viewer.active_settings();
        let ibl_tier = pav.ibl_tier.clone();
        let ibl_spec = IblGenerationSpec::from_tier_name(ibl_tier.as_str());
        let tonemap_cfg = pav.tonemap.as_ref();
        let tonemap_exposure = tonemap_cfg.map(|t| t.exposure).unwrap_or(1.0);
        let tonemap_bloom = if VIEWER_FULL_BLOOM_POST {
            tonemap_cfg.map(|t| t.bloom_strength).unwrap_or(0.0)
        } else {
            0.0
        };

        // ── IBL (HDR par défaut, même fichier que `www/`, mesures alignées WASM) ─
        let hdr_path = workspace.join("www/public/studio_small_03_2k.hdr");
        let mut hdr_ok = false;
        let mut hdr_parse_ms = 0.0f64;
        let mut hdr_ibl_ms = 0.0f64;
        let ibl_context = match std::fs::read(&hdr_path) {
            Ok(bytes) => {
                let t_parse = std::time::Instant::now();
                match load_hdr_from_bytes(&bytes) {
                    Ok(hdr) => {
                        hdr_parse_ms = t_parse.elapsed().as_secs_f64() * 1e3;
                        let t_ibl = std::time::Instant::now();
                        let ctx = IblContext::from_hdr_with_spec(
                            &hdr,
                            &context.device,
                            &context.queue,
                            &ibl_spec,
                        );
                        hdr_ibl_ms = t_ibl.elapsed().as_secs_f64() * 1e3;
                        hdr_ok = true;
                        ctx
                    }
                    Err(e) => {
                        log::warn!("HDR parse failed ({e})");
                        IblContext::new_default(&context.device, &context.queue)
                    }
                }
            }
            Err(_) => IblContext::new_default(&context.device, &context.queue),
        };
        let t_env = std::time::Instant::now();
        let env_bind_group = build_env_bind_group(
            &context.device,
            &render_state.ibl_bg_layout,
            &ibl_context,
            &shadow_pass,
        );
        let hdr_env_bind_ms = t_env.elapsed().as_secs_f64() * 1e3;
        if hdr_ok {
            let total = hdr_parse_ms + hdr_ibl_ms + hdr_env_bind_ms;
            eprintln!(
                "HDR (natif) parse={:.1}ms ibl={:.1}ms env_bind={:.1}ms total={:.1}ms ({})",
                hdr_parse_ms,
                hdr_ibl_ms,
                hdr_env_bind_ms,
                total,
                hdr_path.display()
            );
            log::info!(
                "HDR (natif) parse={:.1}ms ibl={:.1}ms env_bind={:.1}ms total={:.1}ms ({})",
                hdr_parse_ms,
                hdr_ibl_ms,
                hdr_env_bind_ms,
                total,
                hdr_path.display()
            );
        }

        // ── Hi-Z + cull passes ────────────────────────────────────────────────
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

        // ── HDR target + post-process pass ───────────────────────────────────
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
                exposure: tonemap_exposure,
                bloom_strength: tonemap_bloom,
                flags: TonemapParams::FLAG_SKIP_FXAA,
                _pad1: 0.0,
            },
        );

        // ── Readback buffer ───────────────────────────────────────────────────
        let readback_buf = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indirect readback"),
            size: MAX_CULL_ENTITIES * size_of::<DrawIndexedIndirectArgs>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let phase_b_render_graph = render_graph_json.as_ref().map(|path| {
            let json = std::fs::read_to_string(path).unwrap_or_else(|e| {
                panic!("--render-graph {}: read failed: {e}", path.display());
            });
            let doc = parse_render_graph_json(&json).unwrap_or_else(|e| {
                panic!("--render-graph {}: parse failed: {e}", path.display());
            });
            validate_render_graph_exec_v0(&doc, &render_graph_readback).unwrap_or_else(|e| {
                panic!(
                    "--render-graph {}: validate (readback {:?}): {e}",
                    path.display(),
                    render_graph_readback
                );
            });
            let shader_root = path.parent().unwrap_or_else(|| {
                panic!(
                    "--render-graph {}: expected a parent directory (WGSL paths are relative to it)",
                    path.display()
                );
            });
            let registry = RenderGraphGpuRegistry::new(&context.device, &doc).unwrap_or_else(|e| {
                panic!("--render-graph {}: GPU registry: {e}", path.display());
            });

            let shadow_path =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("render_graph_shadow_khronos.json");
            let shadow_json = std::fs::read_to_string(&shadow_path).unwrap_or_else(|e| {
                panic!("{}: read failed: {e}", shadow_path.display());
            });
            let sdoc = parse_render_graph_json(&shadow_json)
                .unwrap_or_else(|e| panic!("{}: parse: {e}", shadow_path.display()));
            validate_render_graph_exec_v0(&sdoc, "hdr_readback_shadow_guard").unwrap_or_else(|e| {
                panic!("{}: validate: {e}", shadow_path.display());
            });
            let mut sreg = RenderGraphGpuRegistry::new(&context.device, &sdoc).unwrap_or_else(|e| {
                panic!("{shadow_path:?}: shadow GPU registry: {e}");
            });
            sreg.insert_buffer(
                "light_uniforms".to_string(),
                shadow_pass.light_uniform_buffer.clone(),
            );
            sreg.insert_buffer(
                "pbr_instance_matrices".to_string(),
                render_state.instance_buffer.clone(),
            );
            sreg.insert_texture_2d("shadow_map".to_string(), texture2d_gpu_shadow_map(&shadow_pass));

            let b7_shader_root = workspace_root().join("fixtures/phases/phase-b");
            if !b7_shader_root.join("shaders/shadow_depth.wgsl").is_file() {
                panic!(
                    "B.7 shadow: missing {}, copy from the repo fixture",
                    b7_shader_root.join("shaders/shadow_depth.wgsl").display()
                );
            }
            let shadow = ShadowGraphInViewer {
                doc: sdoc,
                registry: sreg,
                shader_root: b7_shader_root,
            };
            log::info!(
                "Phase B.4 render graph: {} resource(s), {} pass(es), slot {:?} — {}",
                doc.resources.len(),
                doc.passes.len(),
                render_graph_slot,
                path.display()
            );
            log::info!("Phase B.7 shadow: raster_depth_mesh via data (registry câble moteur).");
            PhaseBRenderGraphHook {
                doc,
                registry,
                shader_root: shader_root.to_path_buf(),
                shadow,
            }
        });

        let orbit = OrbitCamera::new(6.0, 0.22, 0.0, Vec3::ZERO);

        let mut state = Self {
            window,
            context,
            render_state,
            phase_a_viewer,
            asset_registry,
            ibl_context,
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
            sample_idx: 0,
            model_entities: Vec::new(),
            readback_buf,
            last_hiz_visible: 0,
            potential_count: 0,
            frustum_visible: 0,
            total_time: 0.0,
            last_instant: std::time::Instant::now(),
            world,
            scheduler,
            phase_b_render_graph,
            render_graph_slot,
        };

        state.load_sample(0);
        state
    }

    fn encode_phase_b_graph(&self, enc: &mut wgpu::CommandEncoder) {
        if let Some(hook) = &self.phase_b_render_graph {
            if let Err(e) = encode_render_graph_passes_v0(
                enc,
                &self.context.device,
                &hook.registry,
                &hook.doc,
                &hook.shader_root,
            ) {
                log::warn!("render graph encode: {e}");
            }
        }
    }

    /// B.7 : ombre = une passe `raster_depth_mesh` (JSON embarqué) + [`KhronosShadowHost`].
    fn encode_shadow_data_driven(
        &self,
        enc: &mut wgpu::CommandEncoder,
        shadow: &ShadowGraphInViewer,
        shadow_batches: &[ShadowBatch],
    ) {
        let mut load = |rel: &str| {
            std::fs::read_to_string(shadow.shader_root.join(rel)).map_err(RenderGraphExecError::from)
        };
        let mut host = KhronosShadowHost {
            asset_registry: &self.asset_registry,
            shadow_batches,
        };
        if let Err(e) = encode_render_graph_passes_v0_with_wgsl_host(
            enc,
            &self.context.device,
            &shadow.registry,
            &shadow.doc,
            &mut load,
            &mut host,
        ) {
            log::warn!("B.7 shadow graph encode: {e}");
        }
    }

    fn prev_sample(&mut self) {
        let n = KHRONOS_GLBS.len();
        self.load_sample((self.sample_idx + n - 1) % n);
    }

    fn next_sample(&mut self) {
        let n = KHRONOS_GLBS.len();
        self.load_sample((self.sample_idx + 1) % n);
    }

    /// Recharge un GLB Phase A : nouvelle `AssetRegistry` (libère GPU des meshes/mats précédents).
    fn load_sample(&mut self, idx: usize) {
        for &e in &self.model_entities {
            self.world.destroy_entity(e);
        }
        self.model_entities.clear();
        self.sample_idx = idx % KHRONOS_GLBS.len();
        let (label, rel_path) = KHRONOS_GLBS[self.sample_idx];
        let path = workspace_root().join(rel_path);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("lecture GLB échouée {} : {e}", path.display());
        });
        let primitives = load_from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("parse glTF {} : {e}", path.display()));

        self.asset_registry = AssetRegistry::new(&self.context.device, &self.context.queue);
        self.asset_registry.upload_material(
            &Material::default(),
            MaterialTextures::default(),
            &self.context.device,
            &self.render_state.material_bg_layout,
        );

        let (center, radius) = bounds_from_primitives(&primitives);
        let dist = (radius * 2.8).clamp(1.2, 80.0);
        self.orbit = OrbitCamera::new(dist, 0.22, 0.0, center);

        for prim in primitives {
            let mesh_id = self.asset_registry.upload_mesh(
                &prim.mesh,
                &self.context.device,
                &self.context.queue,
            );
            let textures = MaterialTextures {
                albedo: prim.albedo_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        true,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                normal: prim.normal_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                metallic_roughness: prim.metallic_roughness_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                emissive: prim.emissive_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        true,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                anisotropy: prim.anisotropy_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                clearcoat: prim.clearcoat_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                clearcoat_roughness: prim.clearcoat_roughness_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                transmission: prim.transmission_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                specular: prim.specular_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                specular_color: prim.specular_color_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        true,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
                thickness: prim.thickness_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.context.device,
                        &self.context.queue,
                    )
                }),
            };
            let mat_id = self.asset_registry.upload_material(
                &prim.material,
                textures,
                &self.context.device,
                &self.render_state.material_bg_layout,
            );
            let e = self.world.create_entity();
            self.world
                .add_component(e, RenderableComponent::new(mesh_id, mat_id));
            let mut t = TransformComponent::default();
            t.update_local_matrix();
            self.world.add_component(e, t);
            self.model_entities.push(e);
        }

        log::info!(
            "Modèle [{}/{}] {} — {}",
            self.sample_idx + 1,
            KHRONOS_GLBS.len(),
            label,
            path.display()
        );
        self.window.request_redraw();
    }

    // ── Per-frame update ──────────────────────────────────────────────────────

    fn update_orbit_camera(&mut self) {
        let eye = self.orbit.eye();
        let target = self.orbit.target;
        let (_, rot, _) = glam::Mat4::look_at_rh(eye, target, Vec3::Y)
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
        self.frustum_visible = entities.len() as u32;
        self.potential_count = count_visible_renderables(&self.world);

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
                cull_enabled: if VIEWER_GPU_OCCLUSION { 1 } else { 0 },
                _pad: [0; 3],
            }),
        );
        self.hiz_pass
            .update_camera(&self.context.queue, view_proj.to_cols_array_2d());

        self.render(entity_count, &sorted, &shadow_batches);

        // ── Readback: sum instance_count fields from the indirect buffer ───────
        if entity_count > 0 {
            let stride = size_of::<DrawIndexedIndirectArgs>() as u64;
            let bytes = entity_count as u64 * stride;
            let slice = self.readback_buf.slice(..bytes);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            self.context.device.poll(wgpu::Maintain::Wait);
            {
                let view = slice.get_mapped_range();
                let args: &[DrawIndexedIndirectArgs] = bytemuck::cast_slice(&view);
                self.last_hiz_visible = args.iter().map(|a| a.instance_count).sum();
            }
            self.readback_buf.unmap();
        } else {
            self.last_hiz_visible = 0;
        }

        // Monotonicity invariant: culling can only reduce draw count, never increase it.
        // A violation here means a logic error in the cull pass or the ECS pipeline.
        debug_assert!(
            self.last_hiz_visible <= self.frustum_visible,
            "Hi-Z culling emitted more draws than frustum: hiz={} frustum={}",
            self.last_hiz_visible,
            self.frustum_visible,
        );
        debug_assert!(
            self.frustum_visible <= self.potential_count,
            "Frustum culling emitted more draws than total: frustum={} total={}",
            self.frustum_visible,
            self.potential_count,
        );

        let (name, _) = KHRONOS_GLBS[self.sample_idx];
        let mode = if VIEWER_GPU_OCCLUSION {
            "Hi-Z on"
        } else {
            "Hi-Z off"
        };
        let pp = if VIEWER_FULL_BLOOM_POST {
            "bloom on"
        } else {
            "bloom off"
        };
        self.window.set_title(&format!(
            "khronos-pbr-sample | {name} ({}/{}) | {mode} {pp} | draws {} / vis {} | \
             [←/→] modèle  [LMB] orbite  [molette] zoom",
            self.sample_idx + 1,
            KHRONOS_GLBS.len(),
            self.last_hiz_visible,
            self.frustum_visible,
        ));
    }

    // ── Render ────────────────────────────────────────────────────────────────

    fn render(
        &self,
        entity_count: u32,
        sorted: &[DrawEntity],
        shadow_batches: &[ShadowBatch],
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
                &self.phase_a_viewer,
            )),
        );
        self.shadow_pass
            .update_light(&self.context.queue, &build_light_uniforms());

        let indirect_stride = size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut enc = self
            .context
            .device
            .create_command_encoder(&Default::default());

        if self.render_graph_slot == RenderGraphSlot::PreFrame {
            self.encode_phase_b_graph(&mut enc);
        }

        // 1. Z-prepass + Hi-Z pyramid (requis seulement si cull GPU activé)
        if VIEWER_GPU_OCCLUSION {
            self.hiz_pass.encode(
                &mut enc,
                &self.render_state.instance_bind_group,
                &self.asset_registry,
                sorted,
            );
        }

        // 2. GPU cull (Hi‑Z ou tout dessiner si `cull_enabled == 0` dans le shader)
        self.cull_pass.encode(&mut enc, entity_count);

        // 3. Copy indirect buffer → readback staging
        if entity_count > 0 {
            enc.copy_buffer_to_buffer(
                &self.cull_pass.entity_indirect_buf,
                0,
                &self.readback_buf,
                0,
                entity_count as u64 * indirect_stride,
            );
        }

        if self.render_graph_slot == RenderGraphSlot::AfterCullReadback {
            self.encode_phase_b_graph(&mut enc);
        }

        // 4. Shadow depth (CPU-batched) — B.7 data-driven via graphe quand Phase B.4 actif, sinon
        //    pipeline [`ShadowPass`] héritée.
        if let Some(hook) = &self.phase_b_render_graph {
            self.encode_shadow_data_driven(&mut enc, &hook.shadow, shadow_batches);
        } else {
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

        // 5. PBR main pass (GPU draw_indexed_indirect) → HDR target
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04,
                            g: 0.04,
                            b: 0.06,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.context.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
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

        if self.render_graph_slot == RenderGraphSlot::PostPbr {
            self.encode_phase_b_graph(&mut enc);
        }

        // 6. Post-process → swapchain
        if VIEWER_FULL_BLOOM_POST {
            self.post_process.encode(&mut enc, &view);
        } else {
            self.post_process.encode_tonemap_only(&mut enc, &view);
        }

        self.context.queue.submit(std::iter::once(enc.finish()));
        output.present();
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn count_visible_renderables(world: &World) -> u32 {
    world
        .query_entities::<RenderableComponent>()
        .into_iter()
        .filter(|&e| {
            world
                .get_component::<RenderableComponent>(e)
                .is_some_and(|r| r.visible)
        })
        .count() as u32
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
            .unwrap_or(glam::Mat4::IDENTITY);
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

fn transform_aabb(mat: &glam::Mat4, local_min: Vec3, local_max: Vec3) -> (Vec3, Vec3) {
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

fn camera_view_proj(world: &World) -> (glam::Mat4, glam::Mat4) {
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
        .unwrap_or((glam::Mat4::IDENTITY, glam::Mat4::IDENTITY))
}

fn build_light_uniforms() -> LightUniforms {
    let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();
    let light_pos = -light_dir * 30.0;
    let light_view = glam::Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = glam::Mat4::orthographic_rh(-25.0, 25.0, -25.0, 25.0, 0.1, 80.0);
    LightUniforms {
        view_proj: (light_proj * light_view).to_cols_array_2d(),
        shadow_bias: 0.001,
        _pad: [0.0; 3],
    }
}

fn build_frame_uniforms(
    world: &World,
    total_time: f32,
    phase_a: &PhaseAViewerConfig,
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
        .unwrap_or((glam::Mat4::IDENTITY, glam::Mat4::IDENTITY, Vec3::ZERO));
    let inv_vp = (projection * view).inverse();
    let light = build_light_uniforms();
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
        ibl_flags: 0,
        ibl_diffuse_scale: phase_a.ibl_diffuse_scale(),
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
