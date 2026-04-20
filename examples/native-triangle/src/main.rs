use glam::{Quat, Vec3, Vec4};
use pollster::FutureExt;
use std::mem::size_of;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use w3gpu_assets::{load_hdr_from_bytes, primitives, Material};
use w3gpu_ecs::{
    Scheduler, World,
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
};
use w3gpu_renderer::{
    build_entity_list, derive_shadow_batches,
    AssetRegistry, BloomParams, CullPass, CullUniforms, DrawEntity,
    DrawIndexedIndirectArgs, FrameUniforms, GpuContext, HdrTarget, HizPass,
    IblContext, LightUniforms, MaterialTextures, MAX_CULL_ENTITIES,
    PostProcessPass, RenderState, ShadowPass, TonemapParams,
    camera_system, frustum_culling_system, transform_system,
};

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

// ── Orbit camera ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct OrbitCamera {
    yaw:      f32,
    pitch:    f32,
    distance: f32,
    target:   Vec3,
}

impl OrbitCamera {
    fn new(distance: f32, pitch: f32, yaw: f32, target: Vec3) -> Self {
        Self { yaw, pitch, distance, target }
    }

    fn eye(&self) -> Vec3 {
        let y  = self.distance * self.pitch.sin();
        let xz = self.distance * self.pitch.cos();
        self.target + Vec3::new(xz * self.yaw.sin(), y, xz * self.yaw.cos())
    }

    fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw   -= dx * 0.005;
        self.pitch  = (self.pitch + dy * 0.005).clamp(-1.5, 1.5);
    }

    fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(4.0, 120.0);
    }
}

// ── Scene selection ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SceneId { Wall, Sieve, Pendulum }

impl SceneId {
    fn name(self) -> &'static str {
        match self {
            Self::Wall    => "1:Wall",
            Self::Sieve   => "2:Sieve",
            Self::Pendulum => "3:Peekaboo",
        }
    }
}

// ── App ────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct App { state: Option<State> }

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_title("w3gpu"))
            .unwrap();
        self.state = Some(State::new(window).block_on());
    }

    fn window_event(
        &mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                state.context.resize(size.width, size.height);
                state.hiz_pass.resize(&state.context.device, size.width, size.height);
                state.cull_pass.rebuild_hiz_bg(
                    &state.context.device, &state.hiz_pass.hiz_full_view,
                );
                state.hdr_target.resize(&state.context.device, size.width, size.height);
                state.post_process.resize(
                    &state.context.device, &state.hdr_target.view,
                    size.width, size.height,
                );
                for e in state.world.query_entities::<CameraComponent>() {
                    if let Some(cam) = state.world.get_component_mut::<CameraComponent>(e) {
                        cam.aspect = size.width as f32 / size.height as f32;
                    }
                }
                state.window.request_redraw();
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(code),
                    state: ElementState::Pressed, ..
                }, ..
            } => match code {
                KeyCode::Space  => state.cull_enabled = !state.cull_enabled,
                KeyCode::Digit1 => state.load_scene(SceneId::Wall),
                KeyCode::Digit2 => state.load_scene(SceneId::Sieve),
                KeyCode::Digit3 => state.load_scene(SceneId::Pendulum),
                _ => {}
            },

            WindowEvent::MouseInput { button: MouseButton::Left, state: btn, .. } => {
                state.mouse_pressed = btn == ElementState::Pressed;
                if !state.mouse_pressed { state.last_cursor = None; }
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
                    MouseScrollDelta::LineDelta(_, y) => y * 3.0,
                    MouseScrollDelta::PixelDelta(p)   => p.y as f32 * 0.1,
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

struct State {
    window:         Window,
    context:        GpuContext,
    render_state:   RenderState,
    asset_registry: AssetRegistry,
    #[allow(dead_code)]
    ibl_context:    IblContext,
    shadow_pass:    ShadowPass,
    env_bind_group: wgpu::BindGroup,
    hiz_pass:       HizPass,
    cull_pass:      CullPass,
    hdr_target:     HdrTarget,
    post_process:   PostProcessPass,

    // Camera
    camera_entity: u32,
    orbit:         OrbitCamera,
    mouse_pressed: bool,
    last_cursor:   Option<(f32, f32)>,

    // Scene state
    current_scene:   SceneId,
    scene_entities:  Vec<u32>,
    animated:        Vec<(u32, f32)>,
    pendulum_entity: Option<u32>,

    // Shared GPU assets (mesh/mat IDs survive scene switches)
    sphere_mesh:  u32,
    cube_mesh:    u32,
    mat_occluded: u32,
    mat_witness:  u32,
    mat_wall:     u32,
    mat_sweeper:  u32,

    // Readback + metrics
    readback_buf:     wgpu::Buffer,
    last_hiz_visible: u32,
    potential_count:  u32,
    frustum_visible:  u32,

    cull_enabled: bool,
    total_time:   f32,
    last_instant: std::time::Instant,
    world:        World,
    scheduler:    Scheduler,
}

impl State {
    async fn new(window: Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(), ..Default::default()
        });
        let surface  = instance.create_surface(&window).unwrap();
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };
        let context  = GpuContext::new(&instance, surface, size.width, size.height)
            .await.expect("GPU context creation failed");

        let render_state   = RenderState::new(&context.device, context.surface_format);
        let mut asset_registry = AssetRegistry::new(&context.device, &context.queue);

        // Slot 0 — fallback material
        asset_registry.upload_material(
            &Material::default(), MaterialTextures::default(),
            &context.device, &render_state.material_bg_layout,
        );

        // ── Shared meshes ─────────────────────────────────────────────────────
        let sphere_mesh = asset_registry.upload_mesh(
            &primitives::uv_sphere(0.35, 16, 20), &context.device, &context.queue,
        );
        let cube_mesh = asset_registry.upload_mesh(
            &primitives::cube(), &context.device, &context.queue,
        );

        // ── Shared materials ──────────────────────────────────────────────────
        let mat_occluded = asset_registry.upload_material(
            &Material { albedo: [0.9, 0.75, 0.1, 1.0], metallic: 0.6, roughness: 0.35, ..Default::default() },
            MaterialTextures::default(), &context.device, &render_state.material_bg_layout,
        );
        let mat_witness = asset_registry.upload_material(
            &Material { albedo: [0.1, 0.85, 0.65, 1.0], metallic: 0.3, roughness: 0.4, ..Default::default() },
            MaterialTextures::default(), &context.device, &render_state.material_bg_layout,
        );
        let mat_wall = asset_registry.upload_material(
            &Material { albedo: [0.65, 0.65, 0.7, 1.0], metallic: 0.4, roughness: 0.5, ..Default::default() },
            MaterialTextures::default(), &context.device, &render_state.material_bg_layout,
        );
        let mat_sweeper = asset_registry.upload_material(
            &Material { albedo: [0.85, 0.25, 0.1, 1.0], metallic: 0.8, roughness: 0.2, ..Default::default() },
            MaterialTextures::default(), &context.device, &render_state.material_bg_layout,
        );
        let mat_ground = asset_registry.upload_material(
            &Material { albedo: [0.2, 0.2, 0.22, 1.0], metallic: 0.0, roughness: 0.95, ..Default::default() },
            MaterialTextures::default(), &context.device, &render_state.material_bg_layout,
        );

        // ── ECS ───────────────────────────────────────────────────────────────
        let mut world     = World::new();
        let mut scheduler = Scheduler::new();
        scheduler
            .add_system(transform_system)
            .add_system(camera_system)
            .add_system(frustum_culling_system);

        // Camera entity
        let camera_entity = world.create_entity();
        world.add_component(camera_entity, CameraComponent::new(
            60.0, size.width as f32 / size.height as f32, 0.1, 300.0,
        ));
        world.add_component(camera_entity, TransformComponent::default());

        // Ground plane (permanent, always visible)
        let ground = world.create_entity();
        world.add_component(ground, RenderableComponent::new(cube_mesh, mat_ground));
        {
            let mut t = TransformComponent::from_position(Vec3::new(0.0, -2.0, 0.0));
            t.scale = Vec3::new(60.0, 0.05, 60.0);
            t.update_local_matrix();
            world.add_component(ground, t);
        }

        // ── Shadow pass ───────────────────────────────────────────────────────
        let shadow_pass = ShadowPass::new(&context.device, &render_state.instance_bg_layout);

        // ── IBL ───────────────────────────────────────────────────────────────
        let workspace = {
            let manifest = env!("CARGO_MANIFEST_DIR");
            std::path::Path::new(manifest).parent().unwrap().parent().unwrap().to_path_buf()
        };
        let ibl_context = match std::fs::read(workspace.join("www/public/studio_small_03_2k.hdr")) {
            Ok(bytes) => match load_hdr_from_bytes(&bytes) {
                Ok(hdr) => IblContext::from_hdr(&hdr, &context.device, &context.queue),
                Err(e)  => { log::warn!("HDR parse failed ({e})"); IblContext::new_default(&context.device, &context.queue) }
            },
            Err(_) => IblContext::new_default(&context.device, &context.queue),
        };
        let env_bind_group = build_env_bind_group(
            &context.device, &render_state.ibl_bg_layout, &ibl_context, &shadow_pass,
        );

        // ── Hi-Z + cull passes ────────────────────────────────────────────────
        let mut hiz_pass = HizPass::new(
            &context.device, &render_state.instance_bg_layout,
            size.width.max(1), size.height.max(1),
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
            BloomParams  { threshold: 1.0, knee: 0.5,  _pad0: 0.0, _pad1: 0.0 },
            TonemapParams { exposure: 1.0, bloom_strength: 0.04, _pad0: 0.0, _pad1: 0.0 },
        );

        // ── Readback buffer ───────────────────────────────────────────────────
        let readback_buf = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indirect readback"),
            size:  MAX_CULL_ENTITIES * size_of::<DrawIndexedIndirectArgs>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let orbit = OrbitCamera::new(22.0, 0.18, 0.0, Vec3::ZERO);

        let mut state = Self {
            window, context, render_state, asset_registry, ibl_context,
            shadow_pass, env_bind_group, hiz_pass, cull_pass, hdr_target, post_process,
            camera_entity, orbit, mouse_pressed: false, last_cursor: None,
            current_scene: SceneId::Wall,
            scene_entities: Vec::new(),
            animated: Vec::new(),
            pendulum_entity: None,
            sphere_mesh, cube_mesh,
            mat_occluded, mat_witness, mat_wall, mat_sweeper,
            readback_buf,
            last_hiz_visible: 0, potential_count: 0, frustum_visible: 0,
            cull_enabled: true,
            total_time: 0.0,
            last_instant: std::time::Instant::now(),
            world, scheduler,
        };

        state.load_scene(SceneId::Wall);
        state
    }

    // ── Scene loading ─────────────────────────────────────────────────────────

    fn load_scene(&mut self, scene: SceneId) {
        for &e in &self.scene_entities { self.world.destroy_entity(e); }
        self.scene_entities.clear();
        self.animated.clear();
        self.pendulum_entity = None;
        self.total_time = 0.0;
        self.current_scene = scene;
        match scene {
            SceneId::Wall     => self.setup_wall(),
            SceneId::Sieve    => self.setup_sieve(),
            SceneId::Pendulum => self.setup_pendulum(),
        }
    }

    /// Scene 1 — Wall of Occlusion
    ///
    /// A single large gray slab sits between camera and a 40×30=1200 sphere grid.
    /// 6 cyan "witness" spheres placed outside the wall's x-extent stay visible.
    /// Expected: Hi-Z drawn ≈ 7 (wall + 6 witnesses).
    fn setup_wall(&mut self) {
        self.orbit = OrbitCamera::new(22.0, 0.18, 0.0, Vec3::ZERO);

        let wall = self.spawn_cube(Vec3::new(0.0, 0.0, 6.0), Vec3::new(20.0, 14.0, 0.5), self.mat_wall);
        self.scene_entities.push(wall);

        // 40 columns × 30 rows in XY plane at z = -4
        for row in 0..30i32 {
            for col in 0..40i32 {
                let x = (col - 20) as f32 * 0.5;   // -9.75 … +9.75
                let y = (row - 15) as f32 * 0.5;   // -7.25 … +7.25
                let e = self.spawn_sphere(Vec3::new(x, y, -4.0), self.mat_occluded);
                let phase = (row * 40 + col) as f32 * 0.025;
                self.scene_entities.push(e);
                self.animated.push((e, phase));
            }
        }

        // 6 witness spheres outside wall extent (wall covers x ∈ [-10, +10])
        for &sx in &[-1.0f32, 1.0] {
            for &sy in &[-2.0f32, 0.0, 2.0] {
                let e = self.spawn_sphere(Vec3::new(sx * 12.0, sy, -4.0), self.mat_witness);
                self.scene_entities.push(e);
            }
        }
    }

    /// Scene 2 — Sieve (Hi-Z mip selection validation)
    ///
    /// 10 thin vertical pillars spaced 2 units apart with 1.2-unit gaps.
    /// A 21×21=441 sphere grid behind the pillars.
    /// Expected: spheres behind pillars culled, spheres through gaps visible.
    fn setup_sieve(&mut self) {
        self.orbit = OrbitCamera::new(20.0, 0.12, 0.0, Vec3::ZERO);

        // 10 pillars: x ∈ {-9, -7, -5, -3, -1, 1, 3, 5, 7, 9}
        for i in 0..10i32 {
            let x = (i - 5) as f32 * 2.0 + 1.0;
            let p = self.spawn_cube(Vec3::new(x, 0.0, 6.0), Vec3::new(0.8, 18.0, 0.8), self.mat_wall);
            self.scene_entities.push(p);
        }

        // 21×21 = 441 spheres at z = -4 in XY grid
        for row in 0..21i32 {
            for col in 0..21i32 {
                let x = (col - 10) as f32 * 0.5;  // -5.0 … +5.0
                let y = (row - 10) as f32 * 0.5;  // -5.0 … +5.0
                let e = self.spawn_sphere(Vec3::new(x, y, -4.0), self.mat_occluded);
                self.scene_entities.push(e);
            }
        }

        // 6 witnesses outside pillar x-range (> |9|)
        for &sx in &[-1.0f32, 1.0] {
            for &sy in &[-2.0f32, 0.0, 2.0] {
                let e = self.spawn_sphere(Vec3::new(sx * 12.0, sy, -4.0), self.mat_witness);
                self.scene_entities.push(e);
            }
        }
    }

    /// Scene 3 — Temporal Peekaboo (synchronisation validation)
    ///
    /// An orange slab sweeps back and forth across a 20×20=400 sphere grid.
    /// Correct Hi-Z: spheres appear/disappear in perfect sync with the slab.
    /// Lag would cause 1-frame-delayed visibility changes.
    fn setup_pendulum(&mut self) {
        self.orbit = OrbitCamera::new(20.0, 0.12, 0.0, Vec3::ZERO);

        let sweeper = self.spawn_cube(Vec3::new(0.0, 0.0, 6.0), Vec3::new(14.0, 16.0, 0.5), self.mat_sweeper);
        self.pendulum_entity = Some(sweeper);
        self.scene_entities.push(sweeper);

        for row in 0..20i32 {
            for col in 0..20i32 {
                let x = (col - 10) as f32 * 0.5;  // -4.75 … +4.75
                let y = (row - 10) as f32 * 0.5;
                let e = self.spawn_sphere(Vec3::new(x, y, -3.0), self.mat_occluded);
                self.scene_entities.push(e);
            }
        }
    }

    fn spawn_sphere(&mut self, pos: Vec3, mat_id: u32) -> u32 {
        let e = self.world.create_entity();
        self.world.add_component(e, RenderableComponent::new(self.sphere_mesh, mat_id));
        let mut t = TransformComponent::from_position(pos);
        t.update_local_matrix();
        self.world.add_component(e, t);
        e
    }

    fn spawn_cube(&mut self, pos: Vec3, scale: Vec3, mat_id: u32) -> u32 {
        let e = self.world.create_entity();
        self.world.add_component(e, RenderableComponent::new(self.cube_mesh, mat_id));
        let mut t = TransformComponent::from_position(pos);
        t.scale = scale;
        t.update_local_matrix();
        self.world.add_component(e, t);
        e
    }

    // ── Per-frame update ──────────────────────────────────────────────────────

    fn update_orbit_camera(&mut self) {
        let eye    = self.orbit.eye();
        let target = self.orbit.target;
        let (_, rot, _) = glam::Mat4::look_at_rh(eye, target, Vec3::Y)
            .inverse()
            .to_scale_rotation_translation();
        if let Some(t) = self.world.get_component_mut::<TransformComponent>(self.camera_entity) {
            t.position = eye;
            t.rotation = rot;
            t.update_local_matrix();
        }
    }

    fn tick(&mut self) {
        let now = std::time::Instant::now();
        let dt  = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;
        self.total_time  += dt;

        self.update_orbit_camera();

        // Slow spin on occluded spheres (visual interest when culling disabled)
        for &(entity, phase) in &self.animated {
            if let Some(t) = self.world.get_component_mut::<TransformComponent>(entity) {
                t.rotation = Quat::from_rotation_y(self.total_time * 0.3 + phase);
                t.update_local_matrix();
            }
        }

        // Sweeping occluder (Scene 3)
        if let Some(pen) = self.pendulum_entity {
            if let Some(t) = self.world.get_component_mut::<TransformComponent>(pen) {
                t.position.x = (self.total_time * 1.2).sin() * 10.0;
                t.update_local_matrix();
            }
        }

        self.scheduler.run(&mut self.world, dt, self.total_time);

        let entities = collect_draw_entities(&self.world, &self.asset_registry);
        self.frustum_visible = entities.len() as u32;
        self.potential_count = count_visible_renderables(&self.world);

        let (matrices, cull_data, sorted) = build_entity_list(entities);
        let entity_count  = sorted.len() as u32;
        let shadow_batches = derive_shadow_batches(&sorted);

        if !matrices.is_empty() {
            self.context.queue.write_buffer(
                &self.render_state.instance_buffer, 0,
                bytemuck::cast_slice(&matrices),
            );
        }
        if !cull_data.is_empty() {
            self.context.queue.write_buffer(
                &self.cull_pass.entity_cull_buf, 0,
                bytemuck::cast_slice(&cull_data),
            );
        }

        let (view_proj, _) = camera_view_proj(&self.world);
        self.context.queue.write_buffer(
            &self.cull_pass.cull_uniform_buf, 0,
            bytemuck::bytes_of(&CullUniforms {
                view_proj:    view_proj.to_cols_array_2d(),
                screen_size:  [self.hiz_pass.width as f32, self.hiz_pass.height as f32],
                entity_count,
                mip_levels:   self.hiz_pass.mip_count,
                cull_enabled: if self.cull_enabled { 1 } else { 0 },
                _pad:         [0; 3],
            }),
        );
        self.hiz_pass.update_camera(&self.context.queue, view_proj.to_cols_array_2d());

        self.render(entity_count, &sorted, &shadow_batches);

        // ── Readback: sum instance_count fields from the indirect buffer ───────
        if entity_count > 0 {
            let stride = size_of::<DrawIndexedIndirectArgs>() as u64;
            let bytes  = entity_count as u64 * stride;
            let slice  = self.readback_buf.slice(..bytes);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            self.context.device.poll(wgpu::Maintain::Wait);
            {
                let view = slice.get_mapped_range();
                let args: &[DrawIndexedIndirectArgs] = bytemuck::cast_slice(&view);
                self.last_hiz_visible = args.iter().map(|a| a.instance_count).sum();
            }
            self.readback_buf.unmap();
        }

        // Monotonicity invariant: culling can only reduce draw count, never increase it.
        // A violation here means a logic error in the cull pass or the ECS pipeline.
        debug_assert!(
            self.last_hiz_visible <= self.frustum_visible,
            "Hi-Z culling emitted more draws than frustum: hiz={} frustum={}",
            self.last_hiz_visible, self.frustum_visible,
        );
        debug_assert!(
            self.frustum_visible <= self.potential_count,
            "Frustum culling emitted more draws than total: frustum={} total={}",
            self.frustum_visible, self.potential_count,
        );

        let cull_str = if self.cull_enabled { "ON" } else { "OFF" };
        self.window.set_title(&format!(
            "w3gpu | Scene: {} | Total: {} | Frustum: {} | Hi-Z drawn: {} | Cull: {} \
             | [1/2/3] switch  [SPACE] toggle  [LMB drag] orbit  [scroll] zoom",
            self.current_scene.name(),
            self.potential_count,
            self.frustum_visible,
            self.last_hiz_visible,
            cull_str,
        ));
    }

    // ── Render ────────────────────────────────────────────────────────────────

    fn render(
        &self,
        entity_count: u32,
        sorted:        &[DrawEntity],
        shadow_batches: &[w3gpu_renderer::ShadowBatch],
    ) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t)  => t,
            Err(e) => { log::warn!("surface error: {e}"); return; }
        };
        let view = output.texture.create_view(&Default::default());

        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer, 0,
            bytemuck::bytes_of(&build_frame_uniforms(&self.world, self.total_time)),
        );
        self.shadow_pass.update_light(&self.context.queue, &build_light_uniforms());

        let indirect_stride = size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut enc = self.context.device.create_command_encoder(&Default::default());

        // 1. Z-prepass + Hi-Z pyramid
        self.hiz_pass.encode(
            &mut enc, &self.render_state.instance_bind_group, &self.asset_registry, sorted,
        );

        // 2. GPU occlusion cull → writes entity_indirect_buf
        self.cull_pass.encode(&mut enc, entity_count);

        // 3. Copy indirect buffer → readback staging
        if entity_count > 0 {
            enc.copy_buffer_to_buffer(
                &self.cull_pass.entity_indirect_buf, 0,
                &self.readback_buf, 0,
                entity_count as u64 * indirect_stride,
            );
        }

        // 4. Shadow depth pass (CPU-batched)
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_pass.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
            });
            rp.set_pipeline(&self.shadow_pass.depth_pipeline);
            rp.set_bind_group(0, &self.shadow_pass.shadow_light_bind_group, &[]);
            rp.set_bind_group(1, &self.render_state.instance_bind_group, &[]);
            for batch in shadow_batches {
                let Some(m) = self.asset_registry.get_mesh(batch.mesh_id) else { continue };
                rp.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                rp.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(
                    0..m.index_count, 0,
                    batch.first_instance..batch.first_instance + batch.instance_count,
                );
            }
        }

        // 5. PBR main pass (GPU draw_indexed_indirect) → HDR target
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_target.view, resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.04, g: 0.04, b: 0.06, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.context.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
            });
            rp.set_pipeline(&self.render_state.pipeline);
            rp.set_bind_group(0, &self.render_state.frame_bind_group, &[]);
            rp.set_bind_group(1, &self.render_state.instance_bind_group, &[]);
            rp.set_bind_group(3, &self.env_bind_group, &[]);
            for (idx, entity) in sorted.iter().enumerate() {
                let mat = self.asset_registry.get_material(entity.material_id)
                    .or_else(|| self.asset_registry.get_material(0));
                let Some(mat)  = mat  else { continue };
                let Some(mesh) = self.asset_registry.get_mesh(entity.mesh_id) else { continue };
                rp.set_bind_group(2, &mat.bind_group, &[]);
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed_indirect(
                    &self.cull_pass.entity_indirect_buf, idx as u64 * indirect_stride,
                );
            }
        }

        // 6. Post-process: bloom + ACES tonemap + FXAA → swapchain
        self.post_process.encode(&mut enc, &view);

        self.context.queue.submit(std::iter::once(enc.finish()));
        output.present();
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn count_visible_renderables(world: &World) -> u32 {
    world.query_entities::<RenderableComponent>()
        .into_iter()
        .filter(|&e| world.get_component::<RenderableComponent>(e).map_or(false, |r| r.visible))
        .count() as u32
}

fn collect_draw_entities(world: &World, registry: &AssetRegistry) -> Vec<DrawEntity> {
    let entities = world.query_entities::<RenderableComponent>();
    let mut result = Vec::with_capacity(entities.len());
    for entity in entities {
        if world.has_component::<CulledComponent>(entity) { continue; }
        let Some(r) = world.get_component::<RenderableComponent>(entity) else { continue };
        if !r.visible { continue; }
        let world_matrix = world.get_component::<TransformComponent>(entity)
            .map(|t| t.world_matrix)
            .unwrap_or(glam::Mat4::IDENTITY);
        let Some(mesh) = registry.get_mesh(r.mesh_id) else { continue };
        let (aabb_min, aabb_max) = transform_aabb(
            &world_matrix, Vec3::from(mesh.aabb_min), Vec3::from(mesh.aabb_max),
        );
        result.push(DrawEntity {
            mesh_id:     r.mesh_id,
            material_id: r.material_id,
            world_matrix: world_matrix.to_cols_array_2d(),
            cast_shadow:  r.cast_shadow,
            aabb_min:     aabb_min.to_array(),
            aabb_max:     aabb_max.to_array(),
            first_index:  0,
            index_count:  mesh.index_count,
            base_vertex:  0,
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
    let ws: Vec<Vec3> = corners.iter().map(|c| {
        let h = mat.mul_vec4(Vec4::new(c.x, c.y, c.z, 1.0));
        Vec3::new(h.x, h.y, h.z)
    }).collect();
    (
        ws.iter().copied().fold(Vec3::splat(f32::MAX), Vec3::min),
        ws.iter().copied().fold(Vec3::splat(f32::MIN), Vec3::max),
    )
}

fn camera_view_proj(world: &World) -> (glam::Mat4, glam::Mat4) {
    world.query_entities::<CameraComponent>().into_iter().find_map(|e| {
        let cam = world.get_component::<CameraComponent>(e)?;
        if cam.is_active { Some((cam.view_matrix, cam.projection_matrix)) } else { None }
    }).unwrap_or((glam::Mat4::IDENTITY, glam::Mat4::IDENTITY))
}

fn build_light_uniforms() -> LightUniforms {
    let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();
    let light_pos = -light_dir * 30.0;
    let light_view = glam::Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = glam::Mat4::orthographic_rh(-25.0, 25.0, -25.0, 25.0, 0.1, 80.0);
    LightUniforms {
        view_proj:    (light_proj * light_view).to_cols_array_2d(),
        shadow_bias:  0.001,
        _pad:         [0.0; 3],
    }
}

fn build_frame_uniforms(world: &World, total_time: f32) -> FrameUniforms {
    let (view, projection, cam_pos) = world
        .query_entities::<CameraComponent>().into_iter()
        .find_map(|e| {
            let cam = world.get_component::<CameraComponent>(e)?;
            if !cam.is_active { return None; }
            let pos = world.get_component::<TransformComponent>(e)
                .map(|t| { let w = t.world_matrix.w_axis; Vec3::new(w.x, w.y, w.z) })
                .unwrap_or(Vec3::ZERO);
            Some((cam.view_matrix, cam.projection_matrix, pos))
        })
        .unwrap_or((glam::Mat4::IDENTITY, glam::Mat4::IDENTITY, Vec3::ZERO));
    let inv_vp = (projection * view).inverse();
    let light   = build_light_uniforms();
    FrameUniforms {
        projection:          projection.to_cols_array_2d(),
        view:                view.to_cols_array_2d(),
        inv_view_projection: inv_vp.to_cols_array_2d(),
        camera_position:     cam_pos.to_array(), _pad0: 0.0,
        light_direction:     Vec3::new(-0.5, -1.0, -0.5).normalize().to_array(), _pad1: 0.0,
        light_color:         [1.0, 0.95, 0.9], ambient_intensity: 0.12,
        total_time,          _pad2: [0.0; 3],
        light_view_proj:     light.view_proj,
        shadow_bias:         light.shadow_bias, _pad3: [0.0; 3],
    }
}

fn build_env_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    ibl:    &IblContext,
    shadow: &ShadowPass,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("env bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&ibl.irradiance_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&ibl.prefiltered_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&ibl.brdf_lut_view) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&ibl.sampler) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&shadow.shadow_view) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&shadow.comparison_sampler) },
        ],
    })
}
