use glam::{Vec3, Vec4};
use std::mem::size_of;
use std::path::PathBuf;
use w3drs_assets::{
    load_from_bytes, load_hdr_from_bytes, load_phase_a_viewer_config_or_default, AlphaMode,
    GltfPrimitive, Material, PhaseAViewerConfig, ViewerLightState,
};
use w3drs_camera_controller::OrbitController;
use w3drs_ecs::{
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
    Scheduler, World,
};
use w3drs_render_graph::RenderGraphDocument;
use w3drs_renderer::gpu_context::create_depth_texture;
use w3drs_renderer::{
    active_camera_vpc, build_entity_list, build_frame_uniforms_for_viewer, camera_system,
    derive_shadow_batches, encode_render_graph_passes_v0,
    encode_render_graph_passes_v0_with_wgsl_host, light_uniforms_for_cascades,
    parse_render_graph_json, transform_system, validate_render_graph_exec_v0, AssetRegistry,
    BloomParams, CullPass, CullUniforms, DrawEntity, DrawIndexedIndirectArgs, GpuContext,
    HdrTarget, HizPass, IblContext, IblGenerationSpec, MaterialTextures, PostProcessPass,
    RenderGraphExecError, RenderGraphGpuRegistry, RenderGraphV0Host, RenderState, ShadowBatch,
    ShadowPass, Texture2dGpu, TonemapParams, MAX_CULL_ENTITIES, SHADOW_CASCADE_COUNT, SHADOW_SIZE,
};

/// Où insérer le sous-graphe déclaratif Phase B dans le `CommandEncoder` (B.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderGraphSlot {
    /// Avant Hi-Z / cull (premières commandes de la frame).
    PreFrame,
    /// Après cull + copie indirect → readback (défaut, ordre historique).
    #[default]
    AfterCullReadback,
    /// Après le main PBR sur `hdr_target`, avant post-process / swapchain.
    PostPbr,
}

pub fn parse_render_graph_slot(s: &str) -> Option<RenderGraphSlot> {
    match s.to_ascii_lowercase().as_str() {
        "pre" | "pre_frame" => Some(RenderGraphSlot::PreFrame),
        "after_cull" | "cull" => Some(RenderGraphSlot::AfterCullReadback),
        "post_pbr" | "post" | "post_hdr" => Some(RenderGraphSlot::PostPbr),
        _ => None,
    }
}

/// Désactive le pré-pass Hi‑Z + cull GPU (les sphères derrière le sol étaient trop souvent occlues).
const VIEWER_GPU_OCCLUSION: bool = false;
/// Désactive bloom + flous ; tonemap ACES + FXAA selon `tonemap.fxaa` dans le JSON Phase A.
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

// ── PBR (fenêtre ou hôte wgpu) ─────────────────────────────────────────────────

/// Sous-graphe B.7 (ombre) : mêmes GPU buffers que le viewer, encodé à l’étape « shadow ».
#[allow(dead_code)]
struct ShadowGraphInViewer {
    doc: RenderGraphDocument,
    registry: RenderGraphGpuRegistry,
    shader_root: PathBuf,
}

/// Optional Phase B JSON graph (own textures/buffers), validated at init; encoded each frame.
#[allow(dead_code)]
struct PhaseBRenderGraphHook {
    doc: RenderGraphDocument,
    registry: RenderGraphGpuRegistry,
    shader_root: PathBuf,
    /// Ombres en `raster_depth_mesh` (remplace le render pass manuel) — requiert le même
    /// [`workspace_root`]/`fixtures/phases/phase-b/shaders/shadow_depth.wgsl` que l’hôte moteur.
    shadow: ShadowGraphInViewer,
}

/// Hôte B.7 : même boucle d’`draw_indexed` qu’avant, dans le `RenderPass` ouvert par le graphe.
#[allow(dead_code)]
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
        view: sp.cascade_view(0).clone(),
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        width: SHADOW_SIZE,
        height: SHADOW_SIZE,
        mip_level_count: 1,
    }
}

/// GPU partagé entre le sample winit et l’éditeur (hôte wgpu sans surface).
pub struct ViewGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    pub surface_format: wgpu::TextureFormat,
    pub main_pass_msaa: u32,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
}

impl From<GpuContext> for ViewGpu {
    fn from(c: GpuContext) -> Self {
        Self {
            device: c.device,
            queue: c.queue,
            surface: Some(c.surface),
            surface_config: Some(c.surface_config),
            surface_format: c.surface_format,
            main_pass_msaa: c.main_pass_msaa,
            depth_texture: c.depth_texture,
            depth_view: c.depth_view,
        }
    }
}

impl ViewGpu {
    /// Cible de tonemap = `target_format` de l’hôte (ex. egui-wgpu `RenderState::target_format`).
    pub fn new_egui_host(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        target_format: wgpu::TextureFormat,
        main_pass_msaa: u32,
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let (depth_texture, depth_view) = create_depth_texture(&device, w, h, main_pass_msaa);
        Self {
            device,
            queue,
            surface: None,
            surface_config: None,
            surface_format: target_format,
            main_pass_msaa,
            depth_texture,
            depth_view,
        }
    }

    fn resize_winit(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let (Some(surf), Some(cfg)) = (&self.surface, &mut self.surface_config) {
            cfg.width = width;
            cfg.height = height;
            surf.configure(&self.device, cfg);
        }
        (self.depth_texture, self.depth_view) =
            create_depth_texture(&self.device, width, height, self.main_pass_msaa);
    }

    fn resize_egui(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        (self.depth_texture, self.depth_view) =
            create_depth_texture(&self.device, width, height, self.main_pass_msaa);
    }
}

pub struct State {
    /// Présent seulement pour le binaire `khronos-pbr-sample` (winit).
    pub window: Option<winit::window::Window>,
    pub vgpu: ViewGpu,
    render_state: RenderState,
    /// `fixtures/phases/phase-a/materials/default.json` (data-driven, Phase A).
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
    pub orbit: OrbitController,

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
    /// Même init que l’exemple, sans surface (éditeur egui / eframe) — mêmes chemins IBL, GLB, PBR.
    pub fn new_egui_host(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        target_format: wgpu::TextureFormat,
        main_pass_msaa: u32,
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let vgpu = ViewGpu::new_egui_host(device, queue, w, h, target_format, main_pass_msaa);
        Self::from_vgpu(
            vgpu,
            None,
            w,
            h,
            None,
            "hdr_color".to_string(),
            RenderGraphSlot::default(),
        )
    }

    /// Fenêtre winit (binaire `khronos-pbr-sample`) — IBL + éventuellement graphe Phase B.
    pub async fn new_winit(
        window: winit::window::Window,
        render_graph_json: Option<PathBuf>,
        render_graph_readback: String,
        render_graph_slot: RenderGraphSlot,
    ) -> Self {
        let size = window.inner_size();
        let w = size.width;
        let h = size.height;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(&window).unwrap();
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };
        let vgpu: ViewGpu = GpuContext::new(&instance, surface, w, h)
            .await
            .expect("GPU context creation failed")
            .into();
        Self::from_vgpu(
            vgpu,
            Some(window),
            w,
            h,
            render_graph_json,
            render_graph_readback,
            render_graph_slot,
        )
    }

    fn from_vgpu(
        vgpu: ViewGpu,
        window: Option<winit::window::Window>,
        width: u32,
        height: u32,
        render_graph_json: Option<PathBuf>,
        render_graph_readback: String,
        render_graph_slot: RenderGraphSlot,
    ) -> Self {
        let w0 = if width == 0 || height == 0 { 800 } else { width };
        let h0 = if width == 0 || height == 0 { 600 } else { height };

        let render_state = RenderState::new(
            &vgpu.device,
            vgpu.surface_format,
            vgpu.main_pass_msaa,
        );
        let asset_registry = AssetRegistry::new(&vgpu.device, &vgpu.queue);

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
            CameraComponent::new(60.0, w0 as f32 / h0 as f32, 0.1, 300.0),
        );
        world.add_component(camera_entity, TransformComponent::default());

        // ── Shadow pass ───────────────────────────────────────────────────────
        let shadow_pass = ShadowPass::new(&vgpu.device, &render_state.instance_bg_layout);

        let workspace = workspace_root();
        let phase_a_viewer = load_phase_a_viewer_config_or_default(
            &workspace.join("fixtures/phases/phase-a/materials/default.json"),
        );
        let pav = phase_a_viewer.active_settings();
        let ibl_tier = pav.ibl_tier.clone();
        let ibl_spec = IblGenerationSpec::from_tier_name(ibl_tier.as_str());
        let tonemap_cfg = pav.tonemap.as_ref();
        let tonemap_exposure = tonemap_cfg.map(|t| t.exposure).unwrap_or(1.0);
        let tonemap_fxaa = tonemap_cfg.map(|t| t.fxaa).unwrap_or(true);
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
                            &vgpu.device,
                            &vgpu.queue,
                            &ibl_spec,
                        );
                        hdr_ibl_ms = t_ibl.elapsed().as_secs_f64() * 1e3;
                        hdr_ok = true;
                        ctx
                    }
                    Err(e) => {
                        log::warn!("HDR parse failed ({e})");
                        IblContext::new_default(&vgpu.device, &vgpu.queue)
                    }
                }
            }
            Err(_) => IblContext::new_default(&vgpu.device, &vgpu.queue),
        };
        let t_env = std::time::Instant::now();
        let env_bind_group = build_env_bind_group(
            &vgpu.device,
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
            &vgpu.device,
            &render_state.instance_bg_layout,
            w0,
            h0,
        );
        if width == 0 || height == 0 {
            hiz_pass.resize(&vgpu.device, 800, 600);
        }
        let mut cull_pass = CullPass::new(&vgpu.device);
        cull_pass.rebuild_hiz_bg(&vgpu.device, &hiz_pass.hiz_full_view);

        // ── HDR target + post-process pass ───────────────────────────────────
        let hdr_target = HdrTarget::new(
            &vgpu.device,
            w0,
            h0,
            vgpu.main_pass_msaa,
        );
        let post_process = PostProcessPass::new(
            &vgpu.device,
            &hdr_target.view,
            vgpu.surface_format,
            w0,
            h0,
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
        let readback_buf = vgpu.device.create_buffer(&wgpu::BufferDescriptor {
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
            let registry = RenderGraphGpuRegistry::new(&vgpu.device, &doc).unwrap_or_else(|e| {
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
            let mut sreg = RenderGraphGpuRegistry::new(&vgpu.device, &sdoc).unwrap_or_else(|e| {
                panic!("{shadow_path:?}: shadow GPU registry: {e}");
            });
            sreg.insert_buffer(
                "light_uniforms".to_string(),
                shadow_pass.light_uniform_buffers[0].clone(),
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

        let orbit = OrbitController::new(6.0, 0.22, 0.0, Vec3::ZERO);

        let mut state = Self {
            window,
            vgpu,
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
            phase_b_render_graph,
            render_graph_slot,
        };

        state.load_sample(0);
        state
    }

    /// Redimensionnement cohérent (Hi-Z, HDR, post, profondeur, aspect caméra).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.window.is_some() {
            self.vgpu.resize_winit(width, height);
        } else {
            self.vgpu.resize_egui(width, height);
        }
        self.hiz_pass
            .resize(&self.vgpu.device, width, height);
        self.cull_pass
            .rebuild_hiz_bg(&self.vgpu.device, &self.hiz_pass.hiz_full_view);
        self.hdr_target.resize(&self.vgpu.device, width, height);
        self.post_process.resize(
            &self.vgpu.device,
            &self.hdr_target.view,
            width,
            height,
        );
        for e in self.world.query_entities::<CameraComponent>() {
            if let Some(cam) = self.world.get_component_mut::<CameraComponent>(e) {
                cam.aspect = width as f32 / height as f32;
            }
        }
    }

    fn encode_phase_b_graph(&self, enc: &mut wgpu::CommandEncoder) {
        if let Some(hook) = &self.phase_b_render_graph {
            if let Err(e) = encode_render_graph_passes_v0(
                enc,
                &self.vgpu.device,
                &hook.registry,
                &hook.doc,
                &hook.shader_root,
            ) {
                log::warn!("render graph encode: {e}");
            }
        }
    }

    /// B.7 : ombre = une passe `raster_depth_mesh` (JSON embarqué) + [`KhronosShadowHost`].
    #[allow(dead_code)]
    fn encode_shadow_data_driven(
        &self,
        enc: &mut wgpu::CommandEncoder,
        shadow: &ShadowGraphInViewer,
        shadow_batches: &[ShadowBatch],
    ) {
        let mut load = |rel: &str| {
            std::fs::read_to_string(shadow.shader_root.join(rel))
                .map_err(RenderGraphExecError::from)
        };
        let mut host = KhronosShadowHost {
            asset_registry: &self.asset_registry,
            shadow_batches,
        };
        if let Err(e) = encode_render_graph_passes_v0_with_wgsl_host(
            enc,
            &self.vgpu.device,
            &shadow.registry,
            &shadow.doc,
            &mut load,
            &mut host,
        ) {
            log::warn!("B.7 shadow graph encode: {e}");
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

        self.asset_registry = AssetRegistry::new(&self.vgpu.device, &self.vgpu.queue);
        self.asset_registry.upload_material(
            &Material::default(),
            MaterialTextures::default(),
            &self.vgpu.device,
            &self.render_state.material_bg_layout,
        );

        let (center, radius) = bounds_from_primitives(&primitives);
        let dist = (radius * 2.8).clamp(1.2, 80.0);
        self.orbit = OrbitController::new(dist, 0.22, 0.0, center);

        for prim in primitives {
            let mesh_id = self.asset_registry.upload_mesh(
                &prim.mesh,
                &self.vgpu.device,
                &self.vgpu.queue,
            );
            let textures = MaterialTextures {
                albedo: prim.albedo_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        true,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                normal: prim.normal_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                metallic_roughness: prim.metallic_roughness_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                emissive: prim.emissive_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        true,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                anisotropy: prim.anisotropy_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                clearcoat: prim.clearcoat_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                clearcoat_roughness: prim.clearcoat_roughness_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                occlusion: prim.occlusion_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                transmission: prim.transmission_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                specular: prim.specular_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                specular_color: prim.specular_color_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        true,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
                thickness: prim.thickness_image.as_ref().map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data,
                        img.width,
                        img.height,
                        false,
                        &self.vgpu.device,
                        &self.vgpu.queue,
                    )
                }),
            };
            let mat_id = self.asset_registry.upload_material(
                &prim.material,
                textures,
                &self.vgpu.device,
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
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    // ── Per-frame update ──────────────────────────────────────────────────────

    fn update_orbit_camera(&mut self) {
        let pose = self.orbit.pose();
        if let Some(t) = self
            .world
            .get_component_mut::<TransformComponent>(self.camera_entity)
        {
            pose.write_transform(t);
        }
    }

    /// Boucle winit (swapchain) — binaire d’exemple.
    pub fn tick(&mut self) {
        self.tick_inner(None);
    }

    /// Même logique, rendu sur une `TextureView` hôte (éditeur egui / surface egui).
    pub fn tick_egui(&mut self, out: &wgpu::TextureView) {
        self.tick_inner(Some(out));
    }

    /// `pbr_out`: `None` = surface winit, `Some` = cible hôte.
    fn tick_inner(&mut self, pbr_out: Option<&wgpu::TextureView>) {
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
            self.vgpu.queue.write_buffer(
                &self.render_state.instance_buffer,
                0,
                bytemuck::cast_slice(&matrices),
            );
        }
        if !cull_data.is_empty() {
            self.vgpu.queue.write_buffer(
                &self.cull_pass.entity_cull_buf,
                0,
                bytemuck::cast_slice(&cull_data),
            );
        }

        // `camera_view_proj` returns `(view, projection)` — combine them so the
        // cull / Hi-Z passes receive `projection * view` (clip-space matrix).
        let (cam_view, cam_proj) = camera_view_proj(&self.world);
        let cull_vp = cam_proj * cam_view;
        self.vgpu.queue.write_buffer(
            &self.cull_pass.cull_uniform_buf,
            0,
            bytemuck::bytes_of(&CullUniforms {
                view_proj: cull_vp.to_cols_array_2d(),
                screen_size: [self.hiz_pass.width as f32, self.hiz_pass.height as f32],
                entity_count,
                mip_levels: self.hiz_pass.mip_count,
                cull_enabled: if VIEWER_GPU_OCCLUSION { 1 } else { 0 },
                _pad: [0; 3],
            }),
        );
        self.hiz_pass
            .update_camera(&self.vgpu.queue, cull_vp.to_cols_array_2d());

        match pbr_out {
            Some(v) => self.pbr_to_final_view(v, entity_count, &sorted, &shadow_batches),
            None => self.pbr_to_swapchain(entity_count, &sorted, &shadow_batches),
        }

        // ── Readback: sum instance_count fields from the indirect buffer ───────
        if entity_count > 0 {
            let stride = size_of::<DrawIndexedIndirectArgs>() as u64;
            let bytes = entity_count as u64 * stride;
            let slice = self.readback_buf.slice(..bytes);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            let _ = self
                .vgpu
                .device
                .poll(wgpu::PollType::wait_indefinitely());
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
        if let Some(w) = &self.window {
            w.set_title(&format!(
                "khronos-pbr-sample | {name} ({}/{}) | {mode} {pp} | draws {} / vis {} | \
                 [←/→] modèle  [LMB] orbite  [molette] zoom",
                self.sample_idx + 1,
                KHRONOS_GLBS.len(),
                self.last_hiz_visible,
                self.frustum_visible,
            ));
        }
    }

    // ── Render ────────────────────────────────────────────────────────────────

    fn pbr_to_swapchain(
        &self,
        entity_count: u32,
        sorted: &[DrawEntity],
        shadow_batches: &[ShadowBatch],
    ) {
        let Some(surf) = self.vgpu.surface.as_ref() else {
            log::warn!("pbr_to_swapchain: surface manquante");
            return;
        };
        let output = match surf.get_current_texture() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("surface error: {e}");
                return;
            }
        };
        let out_view = output.texture.create_view(&Default::default());
        self.pbr_to_final_view(&out_view, entity_count, sorted, shadow_batches);
        output.present();
    }

    /// Pipeline complet PBR + ombre + tonemap / FXAA vers `out` (RTV de même format que `ViewGpu::surface_format`).
    fn pbr_to_final_view(
        &self,
        out: &wgpu::TextureView,
        entity_count: u32,
        sorted: &[DrawEntity],
        shadow_batches: &[ShadowBatch],
    ) {
        let (view_m, proj_m, cam_pos) = active_camera_vpc(&self.world);
        let frame_uniforms = build_frame_uniforms_for_viewer(
            view_m,
            proj_m,
            cam_pos,
            self.total_time,
            self.phase_a_viewer.ibl_diffuse_scale(),
            &self.viewer_light,
        );
        let (shadow_cascades, _splits) =
            light_uniforms_for_cascades(view_m, proj_m, cam_pos, &self.viewer_light);
        self.vgpu.queue.write_buffer(
            &self.render_state.frame_uniform_buffer,
            0,
            bytemuck::bytes_of(&frame_uniforms),
        );

        let indirect_stride = size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut enc = self
            .vgpu
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

        // 4. Shadow depth (CSM): upload all cascade uniforms once, render each cascade.
        self.shadow_pass
            .update_cascade_lights(&self.vgpu.queue, &shadow_cascades);
        for cascade_idx in 0..SHADOW_CASCADE_COUNT {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.shadow_pass.cascade_view(cascade_idx),
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
            rp.set_bind_group(
                0,
                &self.shadow_pass.shadow_light_bind_groups[cascade_idx],
                &[],
            );
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
                    view: &self.vgpu.depth_view,
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
                rp.set_bind_group(2, &mat.bind_group, &[]);
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed_indirect(
                    &self.cull_pass.entity_indirect_buf,
                    idx as u64 * indirect_stride,
                );
            }
            rp.set_pipeline(&self.render_state.transparent_pipeline);
            for (idx, entity) in sorted.iter().enumerate() {
                let mat = self
                    .asset_registry
                    .get_material(entity.material_id)
                    .or_else(|| self.asset_registry.get_material(0));
                let Some(mat) = mat else { continue };
                if !matches!(mat.alpha_mode, AlphaMode::Blend) {
                    continue;
                }
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

        // 6. Post-process → cible (swapchain ou texture hôte)
        if VIEWER_FULL_BLOOM_POST {
            self.post_process.encode(&mut enc, out);
        } else {
            self.post_process.encode_tonemap_only(&mut enc, out);
        }

        self.vgpu.queue.submit(std::iter::once(enc.finish()));
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
                resource: wgpu::BindingResource::TextureView(&shadow.shadow_array_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&shadow.comparison_sampler),
            },
        ],
    })
}
