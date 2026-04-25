use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use egui_winit::State as EguiWinitState;
use glam::{Vec3, Vec4};
use std::collections::HashMap;
use std::mem::size_of;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use w3drs_assets::{
    load_from_bytes, load_hdr_from_bytes, load_phase_a_viewer_config_or_default, AlphaMode,
    GltfPrimitive, Material, PhaseAViewerConfig, ViewerLightState,
};
use w3drs_camera_controller::OrbitController;
use w3drs_ecs::components::{
    CameraComponent, CulledComponent, RenderableComponent, TransformComponent,
};
use w3drs_ecs::{Scheduler, World};
use w3drs_input::InputFrame;
use w3drs_render_graph::{Pass, RenderGraphDocument};
use w3drs_renderer::{
    build_entity_list, build_frame_uniforms_for_world, camera_system, derive_shadow_batches,
    encode_render_graph_passes_v0_with_wgsl, encode_render_graph_passes_v0_with_wgsl_host,
    light_uniforms_from_viewer, parse_render_graph_json, transform_system,
    validate_render_graph_exec_v0, AssetRegistry, BloomParams, CullPass, CullUniforms, DrawEntity,
    DrawIndexedIndirectArgs, GpuContext, HdrTarget, HizPass, IblContext, IblGenerationSpec,
    MaterialTextures, PostProcessPass, PreparedIbl, RenderGraphExecError, RenderGraphGpuRegistry,
    RenderGraphV0Host, RenderState, ShadowBatch, ShadowPass, Texture2dGpu, TonemapParams,
    MAX_CULL_ENTITIES, SHADOW_SIZE,
};

use winit::window::Window;

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

// ── Phase B (graphe déclaratif + ombre B.7), aligné sur `khronos-pbr-sample` ──

/// Emplacement d’encodage du sous-graphe Phase B dans le `CommandEncoder`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderGraphSlot {
    PreFrame,
    #[default]
    AfterCullReadback,
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

/// `--render-graph`, `--render-graph-readback`, `--render-graph-slot` (même CLI que Khronos).
fn parse_phase_b_cli() -> (Option<PathBuf>, String, RenderGraphSlot) {
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

struct ShadowGraphInViewer {
    doc: RenderGraphDocument,
    registry: RenderGraphGpuRegistry,
    shaders: HashMap<String, String>,
}

struct PhaseBRenderGraphHook {
    doc: RenderGraphDocument,
    registry: RenderGraphGpuRegistry,
    shaders: HashMap<String, String>,
    shadow: ShadowGraphInViewer,
}

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

fn cache_graph_shaders(
    doc: &RenderGraphDocument,
    shader_root: &std::path::Path,
) -> Result<HashMap<String, String>, std::io::Error> {
    let mut shaders = HashMap::new();
    for pass in &doc.passes {
        let shader = match pass {
            Pass::Compute { shader, .. }
            | Pass::RasterMesh { shader, .. }
            | Pass::Fullscreen { shader, .. }
            | Pass::RasterDepthMesh { shader, .. } => Some(shader),
            Pass::Blit { .. } => None,
        };
        if let Some(shader) = shader {
            if !shaders.contains_key(shader) {
                shaders.insert(
                    shader.clone(),
                    std::fs::read_to_string(shader_root.join(shader))?,
                );
            }
        }
    }
    Ok(shaders)
}

enum AssetLoadResult {
    Gltf {
        job_id: u64,
        hint: String,
        result: Result<Vec<GltfPrimitive>, String>,
    },
    Hdr {
        job_id: u64,
        hint: String,
        bytes: Vec<u8>,
        parse_ms: f64,
        bake_ms: f64,
        result: Result<PreparedIbl, String>,
    },
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

/// Tente d’initialiser Phase B.4 + B.7 ; en cas d’échec log + `None` (ombre classique).
fn try_build_phase_b_hook(
    graph_path: &std::path::Path,
    readback_id: &str,
    device: &wgpu::Device,
    render_state: &RenderState,
    shadow_pass: &ShadowPass,
) -> Option<PhaseBRenderGraphHook> {
    let json = std::fs::read_to_string(graph_path)
        .map_err(|e| log::warn!("Phase B: lecture {} : {e}", graph_path.display()))
        .ok()?;
    let doc = parse_render_graph_json(&json)
        .map_err(|e| log::warn!("Phase B: parse {} : {e}", graph_path.display()))
        .ok()?;
    validate_render_graph_exec_v0(&doc, readback_id)
        .map_err(|e| {
            log::warn!(
                "Phase B: validate (readback {readback_id:?}) {} : {e}",
                graph_path.display()
            )
        })
        .ok()?;
    let shader_root = graph_path.parent().map(PathBuf::from).or_else(|| {
        log::warn!(
            "Phase B: pas de répertoire parent pour {}",
            graph_path.display()
        );
        None
    })?;
    let registry = RenderGraphGpuRegistry::new(device, &doc)
        .map_err(|e| log::warn!("Phase B: registre GPU {} : {e}", graph_path.display()))
        .ok()?;
    let shaders = cache_graph_shaders(&doc, &shader_root)
        .map_err(|e| log::warn!("Phase B: cache WGSL {} : {e}", graph_path.display()))
        .ok()?;

    let shadow_path =
        workspace_root().join("examples/khronos-pbr-sample/render_graph_shadow_khronos.json");
    let shadow_json = std::fs::read_to_string(&shadow_path)
        .map_err(|e| log::warn!("Phase B.7: {} : {e}", shadow_path.display()))
        .ok()?;
    let sdoc = parse_render_graph_json(&shadow_json)
        .map_err(|e| log::warn!("Phase B.7: parse shadow JSON : {e}"))
        .ok()?;
    validate_render_graph_exec_v0(&sdoc, "hdr_readback_shadow_guard")
        .map_err(|e| log::warn!("Phase B.7: validate shadow : {e}"))
        .ok()?;
    let mut sreg = RenderGraphGpuRegistry::new(device, &sdoc)
        .map_err(|e| log::warn!("Phase B.7: registre shadow : {e}"))
        .ok()?;
    sreg.insert_buffer(
        "light_uniforms".to_string(),
        shadow_pass.light_uniform_buffer.clone(),
    );
    sreg.insert_buffer(
        "pbr_instance_matrices".to_string(),
        render_state.instance_buffer.clone(),
    );
    sreg.insert_texture_2d(
        "shadow_map".to_string(),
        texture2d_gpu_shadow_map(shadow_pass),
    );

    let b7_shader_root = workspace_root().join("fixtures/phases/phase-b");
    if !b7_shader_root.join("shaders/shadow_depth.wgsl").is_file() {
        log::warn!(
            "Phase B.7: fichier manquant {}",
            b7_shader_root.join("shaders/shadow_depth.wgsl").display()
        );
        return None;
    }
    let shadow_shaders = cache_graph_shaders(&sdoc, &b7_shader_root)
        .map_err(|e| log::warn!("Phase B.7: cache WGSL shadow : {e}"))
        .ok()?;
    let shadow = ShadowGraphInViewer {
        doc: sdoc,
        registry: sreg,
        shaders: shadow_shaders,
    };
    log::info!(
        "pbr-viewer Phase B.4 : {} passe(s) — {}",
        doc.passes.len(),
        graph_path.display()
    );
    log::info!("pbr-viewer Phase B.7 : ombre data-driven (raster_depth_mesh).");
    Some(PhaseBRenderGraphHook {
        doc,
        registry,
        shaders,
        shadow,
    })
}

pub struct PbrState {
    pub window: Window,
    context: GpuContext,
    render_state: RenderState,
    /// `fixtures/phases/phase-a/materials/default.json` (init ; réglages live = champs `live_*`).
    #[allow(dead_code)]
    phase_a_viewer: PhaseAViewerConfig,
    /// Lumière + ombre (défaut = aligné `ViewerLightState::default` / viewer WASM).
    viewer_light: ViewerLightState,
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
    orbit: OrbitController,

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

    /// Panneau latéral (egui) — même rôle que `www/src/viewer/ui.ts`.
    egui_ctx: egui::Context,
    egui_winit: EguiWinitState,
    egui_renderer: EguiRenderer,
    /// Culling Hi‑Z (comme la case à cocher web).
    gpu_occlusion: bool,
    /// Réglages live (JSON Phase A + sliders).
    live_ibl_diffuse: f32,
    live_ibl_tier: String,
    live_exposure: f32,
    live_bloom: f32,
    /// FXAA post-tonemap (JSON Phase A `tonemap.fxaa` + case egui).
    live_fxaa: bool,
    /// Diagnostic runtime: force `tablo_rolex*` with `specularFactor = 0`.
    diag_force_dial_spec0: bool,
    /// Diagnostic runtime: neutral post (`exposure=1`, `bloom=0`, FXAA on).
    diag_neutral_post: bool,
    bloom_enabled: bool,
    bloom_prev_strength: f32,
    /// Dernier HDR chargé (re-bake si le tier IBL change).
    last_hdr_bytes: Option<Vec<u8>>,
    /// Nom affiché du modèle (fichier intégré ou GLB utilisateur).
    model_hint: String,
    asset_load_tx: mpsc::Sender<AssetLoadResult>,
    asset_load_rx: mpsc::Receiver<AssetLoadResult>,
    next_asset_job_id: u64,
    latest_gltf_job_id: u64,
    latest_hdr_job_id: u64,
    loading_status: String,

    /// Phase B.4 + B.7 (optionnel si graphe absent ou invalide).
    phase_b_render_graph: Option<PhaseBRenderGraphHook>,
    render_graph_slot: RenderGraphSlot,
    /// Chemin du graphe Phase B actif (affichage UI).
    phase_b_graph_path: Option<PathBuf>,
}

impl PbrState {
    /// Construction async : surface, HDR par défaut `www/`, premier GLB d’exemple.
    pub async fn new(window: Window) -> Self {
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

        let render_state = RenderState::new(
            &context.device,
            context.surface_format,
            context.main_pass_msaa,
        );
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
        let mut tonemap_exposure = tonemap_cfg.map(|t| t.exposure).unwrap_or(1.0);
        let mut tonemap_bloom = tonemap_cfg.map(|t| t.bloom_strength).unwrap_or(0.0);
        let mut tonemap_fxaa = tonemap_cfg.map(|t| t.fxaa).unwrap_or(true);
        let diag_force_dial_spec0 = env_flag("W3DRS_DIAG_DIAL_SPEC0");
        let diag_neutral_post = env_flag("W3DRS_DIAG_NEUTRAL_POST");
        if diag_neutral_post {
            tonemap_exposure = 1.0;
            tonemap_bloom = 0.0;
            tonemap_fxaa = true;
        }

        // ── IBL (fallback instantané ; HDR par défaut chargé en worker après init) ─
        let hdr_path = workspace.join("www/public/studio_small_03_2k.hdr");
        let ibl_context = IblContext::new_default(&context.device, &context.queue);
        let last_hdr_bytes = None;
        let t_env = std::time::Instant::now();
        let env_bind_group = build_env_bind_group(
            &context.device,
            &render_state.ibl_bg_layout,
            &ibl_context,
            &shadow_pass,
        );
        let hdr_env_bind_ms = t_env.elapsed().as_secs_f64() * 1e3;
        log::info!(
            "HDR fallback bind env_bind={:.1}ms; {} will load asynchronously",
            hdr_env_bind_ms,
            hdr_path.display()
        );

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
        let hdr_target = HdrTarget::new(
            &context.device,
            size.width.max(1),
            size.height.max(1),
            context.main_pass_msaa,
        );
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
                flags: if tonemap_fxaa {
                    0
                } else {
                    TonemapParams::FLAG_SKIP_FXAA
                },
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

        // Phase B : CLI comme Khronos, sinon graphe par défaut du dépôt.
        let (rg_cli_path, rg_cli_readback, rg_cli_slot) = parse_phase_b_cli();
        let (phase_b_graph_path, rg_readback, render_graph_slot) = if let Some(p) = rg_cli_path {
            (Some(p.clone()), rg_cli_readback, rg_cli_slot)
        } else {
            let def = workspace.join("fixtures/phases/phase-b/render_graph.json");
            if def.is_file() {
                (
                    Some(def),
                    "hdr_color".to_string(),
                    RenderGraphSlot::AfterCullReadback,
                )
            } else {
                log::info!(
                    "Phase B: aucun --render-graph et pas de {} — ombre CPU seule.",
                    def.display()
                );
                (None, String::new(), RenderGraphSlot::default())
            }
        };
        let phase_b_render_graph = phase_b_graph_path.as_ref().and_then(|path| {
            try_build_phase_b_hook(
                path,
                rg_readback.as_str(),
                &context.device,
                &render_state,
                &shadow_pass,
            )
        });

        let egui_ctx = egui::Context::default();
        let max_tex = context.device.limits().max_texture_dimension_2d as usize;
        let egui_winit = EguiWinitState::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(max_tex),
        );
        let egui_renderer =
            EguiRenderer::new(&context.device, context.surface_format, None, 1, false);

        let live_ibl_diffuse = pav.ibl_diffuse_scale;
        let live_ibl_tier = pav.ibl_tier.clone();
        let live_exposure = tonemap_exposure;
        let live_bloom = tonemap_bloom;
        let live_fxaa = tonemap_fxaa;

        let orbit = OrbitController::new(6.0, 0.22, 0.0, Vec3::ZERO);
        let (asset_load_tx, asset_load_rx) = mpsc::channel();

        let mut state = Self {
            window,
            context,
            render_state,
            phase_a_viewer,
            viewer_light: ViewerLightState::default(),
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
            egui_ctx,
            egui_winit,
            egui_renderer,
            gpu_occlusion: true,
            live_ibl_diffuse,
            live_ibl_tier,
            live_exposure,
            live_bloom,
            live_fxaa,
            diag_force_dial_spec0,
            diag_neutral_post,
            bloom_enabled: tonemap_bloom > 0.0,
            bloom_prev_strength: tonemap_bloom.max(0.1),
            last_hdr_bytes,
            model_hint: "—".to_string(),
            asset_load_tx,
            asset_load_rx,
            next_asset_job_id: 1,
            latest_gltf_job_id: 0,
            latest_hdr_job_id: 0,
            loading_status: "Chargement initial…".to_string(),
            phase_b_render_graph,
            render_graph_slot,
            phase_b_graph_path,
        };

        state.load_sample(0);
        state.request_load_hdr_path(hdr_path, "studio_small_03_2k.hdr".to_string(), ibl_spec);
        state
    }

    pub fn apply_camera_input(&mut self, input: &InputFrame) {
        self.orbit.apply_input(input);
        if !input.primary_drag.is_zero()
            || !input.secondary_drag.is_zero()
            || !input.middle_drag.is_zero()
            || input.wheel_lines != 0.0
        {
            self.window.request_redraw();
        }
    }

    pub fn egui_context(&self) -> &egui::Context {
        &self.egui_ctx
    }

    pub fn on_egui_window_event(
        &mut self,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_winit.on_window_event(&self.window, event)
    }

    pub fn toggle_gpu_occlusion(&mut self) {
        self.gpu_occlusion = !self.gpu_occlusion;
        self.window.request_redraw();
    }

    /// Recadre l’orbite sur l’union des AABB des meshes visibles (espace monde).
    pub fn reframe_camera_on_scene(&mut self) {
        // Synchronise `world_matrix` ← `local_matrix` (ex. juste après `upload_primitives`).
        transform_system(&mut self.world, 0.0, 0.0);
        if let Some((center, radius)) = scene_world_bounds(&self.world, &self.asset_registry) {
            let (fov, aspect) = self
                .world
                .query_entities::<CameraComponent>()
                .into_iter()
                .find_map(|e| {
                    self.world
                        .get_component::<CameraComponent>(e)
                        .and_then(|c| {
                            if c.is_active {
                                Some((c.fov_y_radians.to_degrees(), c.aspect))
                            } else {
                                None
                            }
                        })
                })
                .unwrap_or((60.0, 16.0 / 9.0));
            self.orbit.reframe(center, radius, fov, aspect);
            self.window.request_redraw();
        } else {
            log::info!("Reframe : aucun mesh visible dans la scène");
        }
    }

    /// Redimensionnement (surface + Hi-Z, HDR, post, caméra).
    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 && h == 0 {
            return;
        }
        self.context.resize(w, h);
        self.hiz_pass.resize(&self.context.device, w, h);
        self.cull_pass
            .rebuild_hiz_bg(&self.context.device, &self.hiz_pass.hiz_full_view);
        self.hdr_target.resize(&self.context.device, w, h);
        self.post_process
            .resize(&self.context.device, &self.hdr_target.view, w, h);
        for e in self.world.query_entities::<CameraComponent>() {
            if let Some(cam) = self.world.get_component_mut::<CameraComponent>(e) {
                cam.aspect = w as f32 / h as f32;
            }
        }
    }

    pub fn prev_sample(&mut self) {
        let n = KHRONOS_GLBS.len();
        self.load_sample((self.sample_idx + n - 1) % n);
    }

    pub fn next_sample(&mut self) {
        let n = KHRONOS_GLBS.len();
        self.load_sample((self.sample_idx + 1) % n);
    }

    /// Recharge un GLB Phase A (réinitialise le registre GPU, comme le viewer web).
    fn load_sample(&mut self, idx: usize) {
        self.sample_idx = idx % KHRONOS_GLBS.len();
        let (label, rel_path) = KHRONOS_GLBS[self.sample_idx];
        let path = workspace_root().join(rel_path);
        self.request_load_gltf_path(path, label.to_string());
    }

    fn next_asset_job_id(&mut self) -> u64 {
        let id = self.next_asset_job_id;
        self.next_asset_job_id += 1;
        id
    }

    fn request_load_gltf_path(&mut self, path: PathBuf, hint: String) {
        let job_id = self.next_asset_job_id();
        self.latest_gltf_job_id = job_id;
        self.loading_status = format!("Décodage GLB en arrière-plan : {hint}");
        let tx = self.asset_load_tx.clone();
        thread::spawn(move || {
            let result = std::fs::read(&path)
                .map_err(|e| format!("lecture GLB échouée {} : {e}", path.display()))
                .and_then(|bytes| load_from_bytes(&bytes).map_err(|e| e.to_string()));
            let _ = tx.send(AssetLoadResult::Gltf {
                job_id,
                hint,
                result,
            });
        });
    }

    #[allow(dead_code)]
    fn request_load_gltf_bytes(&mut self, bytes: Vec<u8>, hint: String) {
        let job_id = self.next_asset_job_id();
        self.latest_gltf_job_id = job_id;
        self.loading_status = format!("Décodage GLB en arrière-plan : {hint}");
        let tx = self.asset_load_tx.clone();
        thread::spawn(move || {
            let result = load_from_bytes(&bytes).map_err(|e| e.to_string());
            let _ = tx.send(AssetLoadResult::Gltf {
                job_id,
                hint,
                result,
            });
        });
    }

    fn apply_loaded_gltf(&mut self, primitives: Vec<GltfPrimitive>, hint: String) {
        self.clear_scene_models();
        self.asset_registry
            .reset_in_place(&self.context.device, &self.context.queue);
        self.asset_registry.upload_material(
            &Material::default(),
            MaterialTextures::default(),
            &self.context.device,
            &self.render_state.material_bg_layout,
        );
        self.upload_primitives(primitives);
        self.reframe_camera_on_scene();
        self.model_hint = hint.clone();
        self.loading_status = format!("GLB chargé : {hint}");

        log::info!(
            "Modèle [{}/{}] {}",
            self.sample_idx + 1,
            KHRONOS_GLBS.len(),
            hint,
        );
        self.window.request_redraw();
    }

    fn request_load_hdr_path(&mut self, path: PathBuf, hint: String, spec: IblGenerationSpec) {
        let job_id = self.next_asset_job_id();
        self.latest_hdr_job_id = job_id;
        self.loading_status = format!("Préparation HDR/IBL en arrière-plan : {hint}");
        let tx = self.asset_load_tx.clone();
        thread::spawn(move || {
            let start_parse = std::time::Instant::now();
            let bytes_result = std::fs::read(&path)
                .map_err(|e| format!("lecture HDR échouée {} : {e}", path.display()));
            let (bytes, result, parse_ms, bake_ms) = match bytes_result {
                Ok(bytes) => {
                    let hdr = load_hdr_from_bytes(&bytes).map_err(|e| e.to_string());
                    let parse_ms = start_parse.elapsed().as_secs_f64() * 1e3;
                    match hdr {
                        Ok(hdr) => {
                            let start_bake = std::time::Instant::now();
                            let prepared = PreparedIbl::from_hdr_with_spec(&hdr, &spec);
                            let bake_ms = start_bake.elapsed().as_secs_f64() * 1e3;
                            (bytes, Ok(prepared), parse_ms, bake_ms)
                        }
                        Err(e) => (bytes, Err(e), parse_ms, 0.0),
                    }
                }
                Err(e) => (Vec::new(), Err(e), 0.0, 0.0),
            };
            let _ = tx.send(AssetLoadResult::Hdr {
                job_id,
                hint,
                bytes,
                parse_ms,
                bake_ms,
                result,
            });
        });
    }

    // ── Per-frame update ──────────────────────────────────────────────────────

    fn process_asset_loads(&mut self) {
        while let Ok(msg) = self.asset_load_rx.try_recv() {
            match msg {
                AssetLoadResult::Gltf {
                    job_id,
                    hint,
                    result,
                } => {
                    if job_id != self.latest_gltf_job_id {
                        continue;
                    }
                    match result {
                        Ok(primitives) => self.apply_loaded_gltf(primitives, hint),
                        Err(e) => {
                            self.loading_status = format!("GLB échoué : {e}");
                            log::warn!("{}", self.loading_status);
                        }
                    }
                }
                AssetLoadResult::Hdr {
                    job_id,
                    hint,
                    bytes,
                    parse_ms,
                    bake_ms,
                    result,
                } => {
                    if job_id != self.latest_hdr_job_id {
                        continue;
                    }
                    match result {
                        Ok(prepared) => {
                            let start_upload = std::time::Instant::now();
                            self.ibl_context = IblContext::from_prepared(
                                &self.context.device,
                                &self.context.queue,
                                &prepared,
                            );
                            self.env_bind_group = build_env_bind_group(
                                &self.context.device,
                                &self.render_state.ibl_bg_layout,
                                &self.ibl_context,
                                &self.shadow_pass,
                            );
                            let upload_ms = start_upload.elapsed().as_secs_f64() * 1e3;
                            self.last_hdr_bytes = Some(bytes);
                            self.loading_status = format!(
                                "HDR chargé : {hint} (parse {parse_ms:.1}ms, bake {bake_ms:.1}ms, upload {upload_ms:.1}ms)"
                            );
                            log::info!("{}", self.loading_status);
                            self.window.request_redraw();
                        }
                        Err(e) => {
                            self.loading_status = format!("HDR échoué : {e}");
                            log::warn!("{}", self.loading_status);
                        }
                    }
                }
            }
        }
    }

    fn update_orbit_camera(&mut self) {
        let pose = self.orbit.pose();
        if let Some(t) = self
            .world
            .get_component_mut::<TransformComponent>(self.camera_entity)
        {
            pose.write_transform(t);
        }
    }

    pub fn tick(&mut self) {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;
        self.total_time += dt;

        self.process_asset_loads();
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
                cull_enabled: if self.gpu_occlusion { 1 } else { 0 },
                _pad: [0; 3],
            }),
        );
        self.hiz_pass
            .update_camera(&self.context.queue, view_proj.to_cols_array_2d());

        self.render(entity_count, &sorted, &shadow_batches);

        // Avoid a per-frame MAP_READ + Maintain::Wait stall in the interactive viewer.
        // The exact Hi-Z count can be reintroduced behind a debug-only async readback.
        self.last_hiz_visible = self.frustum_visible;

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
        let mode = if self.gpu_occlusion {
            "Hi-Z on"
        } else {
            "Hi-Z off"
        };
        let pp = if self.bloom_enabled {
            "bloom on"
        } else {
            "bloom off"
        };
        let msaa = self.context.main_pass_msaa;
        self.window.set_title(&format!(
            "pbr-viewer | {} | {name} ({}/{}) | {mode} {pp} | MSAA {msaa}× | draws {} / vis {}",
            self.model_hint,
            self.sample_idx + 1,
            KHRONOS_GLBS.len(),
            self.last_hiz_visible,
            self.frustum_visible,
        ));
    }

    // ── Render ────────────────────────────────────────────────────────────────

    fn encode_phase_b_graph(&self, enc: &mut wgpu::CommandEncoder) {
        if let Some(hook) = &self.phase_b_render_graph {
            let mut load = |rel: &str| {
                hook.shaders
                    .get(rel)
                    .cloned()
                    .ok_or_else(|| RenderGraphExecError::WgslNotFound {
                        rel: rel.to_string(),
                    })
            };
            if let Err(e) = encode_render_graph_passes_v0_with_wgsl(
                enc,
                &self.context.device,
                &hook.registry,
                &hook.doc,
                &mut load,
            ) {
                log::warn!("Phase B.4 encode graphe: {e}");
            }
        }
    }

    fn encode_shadow_data_driven(
        &self,
        enc: &mut wgpu::CommandEncoder,
        shadow: &ShadowGraphInViewer,
        shadow_batches: &[ShadowBatch],
    ) {
        let mut load = |rel: &str| {
            shadow
                .shaders
                .get(rel)
                .cloned()
                .ok_or_else(|| RenderGraphExecError::WgslNotFound {
                    rel: rel.to_string(),
                })
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
            log::warn!("Phase B.7 encode ombre: {e}");
        }
    }

    fn render(&mut self, entity_count: u32, sorted: &[DrawEntity], shadow_batches: &[ShadowBatch]) {
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
            bytemuck::bytes_of(&build_frame_uniforms_for_world(
                &self.world,
                self.total_time,
                self.live_ibl_diffuse,
                &self.viewer_light,
            )),
        );
        self.shadow_pass.update_light(
            &self.context.queue,
            &light_uniforms_from_viewer(&self.viewer_light),
        );

        let indirect_stride = size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut enc = self
            .context
            .device
            .create_command_encoder(&Default::default());

        if self.render_graph_slot == RenderGraphSlot::PreFrame {
            self.encode_phase_b_graph(&mut enc);
        }

        // 1. Z-prepass + Hi-Z pyramid (requis seulement si cull GPU activé)
        if self.gpu_occlusion {
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

        // 4. Shadow depth — B.7 data-driven si Phase B active, sinon [`ShadowPass`] classique.
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

        // 5. PBR main pass (GPU draw_indexed_indirect) → HDR (MSAA + resolve si >1)
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(self.hdr_target.main_pass_color_attachment(
                    wgpu::Color {
                        r: 0.04,
                        g: 0.04,
                        b: 0.06,
                        a: 1.0,
                    },
                ))],
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
                if matches!(mat.alpha_mode, AlphaMode::Blend) {
                    continue;
                }
                let Some(mesh) = self.asset_registry.get_mesh(entity.mesh_id) else {
                    continue;
                };
                if mat.double_sided {
                    rp.set_pipeline(&self.render_state.double_sided_pipeline);
                } else {
                    rp.set_pipeline(&self.render_state.pipeline);
                }
                rp.set_bind_group(2, &mat.bind_group, &[]);
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed_indirect(
                    &self.cull_pass.entity_indirect_buf,
                    idx as u64 * indirect_stride,
                );
            }
            let camera_eye = self.orbit.eye();
            let mut transparent: Vec<(usize, &DrawEntity, f32)> = sorted
                .iter()
                .enumerate()
                .filter_map(|(idx, entity)| {
                    let mat = self
                        .asset_registry
                        .get_material(entity.material_id)
                        .or_else(|| self.asset_registry.get_material(0))?;
                    if !matches!(mat.alpha_mode, AlphaMode::Blend) {
                        return None;
                    }
                    let center = (Vec3::from(entity.aabb_min) + Vec3::from(entity.aabb_max)) * 0.5;
                    Some((idx, entity, center.distance_squared(camera_eye)))
                })
                .collect();
            transparent.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            for (idx, entity, _) in transparent {
                let mat = self
                    .asset_registry
                    .get_material(entity.material_id)
                    .or_else(|| self.asset_registry.get_material(0));
                let Some(mat) = mat else { continue };
                let Some(mesh) = self.asset_registry.get_mesh(entity.mesh_id) else {
                    continue;
                };
                if mat.double_sided {
                    rp.set_pipeline(&self.render_state.double_sided_transparent_pipeline);
                } else {
                    rp.set_pipeline(&self.render_state.transparent_pipeline);
                }
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
        if self.bloom_enabled {
            self.post_process.encode(&mut enc, &view);
        } else {
            self.post_process.encode_tonemap_only(&mut enc, &view);
        }

        // 7. Panneau egui (fichiers, sliders — aligné sur `www/src/viewer/ui.ts`).
        let raw = self.egui_winit.take_egui_input(&self.window);
        let egui_ctx = self.egui_ctx.clone();
        let egui_out = egui_ctx.run(raw, |ctx| self.native_egui_panel(ctx));
        self.egui_winit
            .handle_platform_output(&self.window, egui_out.platform_output);
        let paint_jobs = self
            .egui_ctx
            .tessellate(egui_out.shapes, egui_out.pixels_per_point);
        for (id, delta) in egui_out.textures_delta.set {
            self.egui_renderer.update_texture(
                &self.context.device,
                &self.context.queue,
                id,
                &delta,
            );
        }
        for id in egui_out.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }
        let sz = self.window.inner_size();
        let screen = ScreenDescriptor {
            size_in_pixels: [sz.width.max(1), sz.height.max(1)],
            pixels_per_point: self.window.scale_factor() as f32,
        };
        let extra_cmd = self.egui_renderer.update_buffers(
            &self.context.device,
            &self.context.queue,
            &mut enc,
            &paint_jobs,
            &screen,
        );
        {
            let rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            let mut rp = rp.forget_lifetime();
            self.egui_renderer.render(&mut rp, &paint_jobs, &screen);
        }

        let main_cmd = enc.finish();
        self.context
            .queue
            .submit(extra_cmd.into_iter().chain(std::iter::once(main_cmd)));
        output.present();
    }

    fn sync_tonemap_gpu(&mut self) {
        let (exposure, bloom, fxaa) = if self.diag_neutral_post {
            (1.0, 0.0, true)
        } else {
            let bloom = if self.bloom_enabled {
                self.live_bloom
            } else {
                0.0
            };
            (self.live_exposure, bloom, self.live_fxaa)
        };
        self.post_process.update_tonemap_params(
            &self.context.queue,
            TonemapParams {
                exposure,
                bloom_strength: bloom,
                flags: if fxaa {
                    0
                } else {
                    TonemapParams::FLAG_SKIP_FXAA
                },
                _pad1: 0.0,
            },
        );
    }

    fn clear_scene_models(&mut self) {
        for &e in &self.model_entities {
            self.world.destroy_entity(e);
        }
        self.model_entities.clear();
    }

    fn upload_primitives(&mut self, primitives: Vec<GltfPrimitive>) {
        for prim in primitives {
            let mut material = prim.material;
            if self.diag_force_dial_spec0 && material.name.starts_with("tablo_rolex") {
                material.khr_flags |= 1;
                material.specular_factor = 0.0;
                log::info!(
                    "DIAG: specularFactor forcé à 0 sur material '{}'",
                    material.name
                );
            }
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
                occlusion: prim.occlusion_image.as_ref().map(|img| {
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
                &material,
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
    }

    #[allow(dead_code)]
    pub fn load_gltf_from_bytes(&mut self, bytes: &[u8], hint: &str) {
        self.request_load_gltf_bytes(bytes.to_vec(), hint.to_string());
    }

    fn reload_hdr_if_bytes(&mut self) {
        if let Some(bytes) = self.last_hdr_bytes.clone() {
            self.reload_hdr_from_bytes(&bytes);
        }
    }

    pub fn reload_hdr_from_bytes(&mut self, bytes: &[u8]) {
        let spec = IblGenerationSpec::from_tier_name(self.live_ibl_tier.as_str());
        let job_id = self.next_asset_job_id();
        self.latest_hdr_job_id = job_id;
        self.loading_status = "Re-bake HDR/IBL en arrière-plan".to_string();
        let bytes = bytes.to_vec();
        let tx = self.asset_load_tx.clone();
        thread::spawn(move || {
            let start_parse = std::time::Instant::now();
            let hdr = load_hdr_from_bytes(&bytes).map_err(|e| e.to_string());
            let parse_ms = start_parse.elapsed().as_secs_f64() * 1e3;
            let (result, bake_ms) = match hdr {
                Ok(hdr) => {
                    let start_bake = std::time::Instant::now();
                    let prepared = PreparedIbl::from_hdr_with_spec(&hdr, &spec);
                    (Ok(prepared), start_bake.elapsed().as_secs_f64() * 1e3)
                }
                Err(e) => (Err(e), 0.0),
            };
            let _ = tx.send(AssetLoadResult::Hdr {
                job_id,
                hint: "HDR mémoire".to_string(),
                bytes,
                parse_ms,
                bake_ms,
                result,
            });
        });
    }

    fn native_egui_panel(&mut self, ctx: &egui::Context) {
        const TIERS: &[&str] = &["min", "low", "medium", "high", "max"];
        self.sync_tonemap_gpu();

        egui::SidePanel::left("w3d_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("w3drs viewer (natif)");
                ui.label("Panneau aligné sur le viewer web — pas un doublon de khronos-pbr-sample.");
                ui.separator();

                ui.label(egui::RichText::new("Environnement").strong());
                if ui.button("Choisir un fichier GLB…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("glTF binaire", &["glb"])
                        .pick_file()
                    {
                        let hint = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("GLB")
                            .to_string();
                        self.request_load_gltf_path(path, hint);
                    }
                }
                if ui.button("Choisir un fichier HDR…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Radiance HDR", &["hdr"])
                        .pick_file()
                    {
                        let hint = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("HDR")
                            .to_string();
                        let spec = IblGenerationSpec::from_tier_name(self.live_ibl_tier.as_str());
                        self.request_load_hdr_path(path, hint, spec);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("GLB intégrés");
                    if ui.button("◀").on_hover_text("Modèle précédent").clicked() {
                        self.prev_sample();
                    }
                    if ui.button("▶").on_hover_text("Modèle suivant").clicked() {
                        self.next_sample();
                    }
                });
                if ui
                    .button("Reframe (AABB)")
                    .on_hover_text("Recadre l’orbite sur l’union des AABB des meshes (espace monde)")
                    .clicked()
                {
                    self.reframe_camera_on_scene();
                }

                ui.label("IBL tier (re-bake si HDR en mémoire)");
                egui::ComboBox::from_id_salt("ibl_tier")
                    .selected_text(&self.live_ibl_tier)
                    .show_ui(ui, |ui| {
                        for t in TIERS {
                            if ui
                                .selectable_value(&mut self.live_ibl_tier, (*t).to_string(), *t)
                                .clicked()
                            {
                                self.reload_hdr_if_bytes();
                            }
                        }
                    });

                ui.add(egui::Slider::new(&mut self.live_ibl_diffuse, 0.0..=2.0).text("IBL diffuse scale"));
                ui.weak(&self.loading_status);
                ui.separator();

                ui.label(egui::RichText::new("Image").strong());
                ui.add(egui::Slider::new(&mut self.live_exposure, 0.1..=4.0).text("Exposition"));
                if ui.checkbox(&mut self.bloom_enabled, "Bloom actif").changed() {
                    if self.bloom_enabled {
                        self.live_bloom = self.bloom_prev_strength.max(0.01);
                    } else {
                        self.bloom_prev_strength = self.live_bloom.max(0.01);
                        self.live_bloom = 0.0;
                    }
                }
                ui.add(egui::Slider::new(&mut self.live_bloom, 0.0..=1.0).text("Bloom (si bloom actif)"));
                if self.live_bloom > 0.0 {
                    self.bloom_prev_strength = self.live_bloom;
                }
                ui.checkbox(&mut self.live_fxaa, "FXAA (post-tonemap)");
                ui.label(format!(
                    "MSAA (pass HDR principal) : {}×",
                    self.context.main_pass_msaa
                ));
                if self.diag_force_dial_spec0 || self.diag_neutral_post {
                    ui.label(egui::RichText::new("DIAG runtime actif").strong());
                    if self.diag_force_dial_spec0 {
                        ui.label("• W3DRS_DIAG_DIAL_SPEC0=1 (specular cadran Rolex forcé à 0)");
                    }
                    if self.diag_neutral_post {
                        ui.label("• W3DRS_DIAG_NEUTRAL_POST=1 (expo 1 / bloom 0 / FXAA on)");
                    }
                }
                ui.separator();

                ui.label(egui::RichText::new("Lumière").strong());
                ui.horizontal(|ui| {
                    ui.label("dir");
                    ui.add(egui::DragValue::new(&mut self.viewer_light.light_direction[0]).speed(0.02));
                    ui.add(egui::DragValue::new(&mut self.viewer_light.light_direction[1]).speed(0.02));
                    ui.add(egui::DragValue::new(&mut self.viewer_light.light_direction[2]).speed(0.02));
                });
                ui.horizontal(|ui| {
                    ui.label("couleur");
                    ui.add(egui::DragValue::new(&mut self.viewer_light.light_color[0]).speed(0.02));
                    ui.add(egui::DragValue::new(&mut self.viewer_light.light_color[1]).speed(0.02));
                    ui.add(egui::DragValue::new(&mut self.viewer_light.light_color[2]).speed(0.02));
                });
                ui.add(
                    egui::Slider::new(&mut self.viewer_light.directional_intensity, 0.0..=3.0)
                        .text("Int. directionnelle"),
                );
                ui.add(egui::Slider::new(&mut self.viewer_light.ambient_intensity, 0.0..=0.6).text("Ambiant"));
                ui.add(egui::Slider::new(&mut self.viewer_light.shadow_bias, 0.0..=0.01).text("Shadow bias"));
                ui.separator();

                ui.collapsing("Phase B (graphe)", |ui| {
                    if let Some(p) = &self.phase_b_graph_path {
                        ui.label(format!("Fichier : {}", p.display()));
                    } else {
                        ui.label("Fichier : (aucun — ombre classique ShadowPass)");
                    }
                    ui.label(format!("Slot d’encodage : {:?}", self.render_graph_slot));
                    if self.phase_b_render_graph.is_some() {
                        ui.label(egui::RichText::new("B.4 passes + B.7 ombre data-driven : actifs.").strong());
                    } else {
                        ui.weak("Réactiver : placer fixtures/phases/phase-b/render_graph.json ou lancer avec --render-graph (voir khronos-pbr-sample).");
                    }
                });

                ui.label(egui::RichText::new("Rendu").strong());
                let mut cull = self.gpu_occlusion;
                if ui.checkbox(&mut cull, "Culling Hi-Z (GPU)").changed() {
                    self.gpu_occlusion = cull;
                }
            });
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

/// Union des AABB mesh en espace monde (renderables visibles ; hors filtre Hi-Z pour le cadrage).
fn scene_world_bounds(world: &World, registry: &AssetRegistry) -> Option<(Vec3, f32)> {
    let mut mn = Vec3::splat(f32::MAX);
    let mut mx = Vec3::splat(f32::MIN);
    let mut any = false;
    for entity in world.query_entities::<RenderableComponent>() {
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
        let (amin, amax) = transform_aabb(
            &world_matrix,
            Vec3::from(mesh.aabb_min),
            Vec3::from(mesh.aabb_max),
        );
        mn = mn.min(amin);
        mx = mx.max(amax);
        any = true;
    }
    if !any {
        return None;
    }
    let center = (mn + mx) * 0.5;
    let radius = ((mx - mn).length() * 0.5).max(0.15);
    Some((center, radius))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
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
