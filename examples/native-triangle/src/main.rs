use glam::{Quat, Vec3, Vec4};
use pollster::FutureExt;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use w3gpu_assets::{load_from_bytes, load_hdr_from_bytes, primitives};
use w3gpu_ecs::{
    Scheduler, World,
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
};
use w3gpu_renderer::{
    build_entity_list, derive_shadow_batches,
    AssetRegistry, CullPass, CullUniforms, DrawEntity, DrawIndexedIndirectArgs,
    FrameUniforms, GpuContext, HizPass, IblContext, LightUniforms,
    MaterialTextures, RenderState, ShadowPass,
    camera_system, frustum_culling_system, transform_system,
};

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_title("w3gpu"))
            .unwrap();
        self.state = Some(State::new(window).block_on());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.context.resize(size.width, size.height);
                state.hiz_pass.resize(&state.context.device, size.width, size.height);
                state.cull_pass.rebuild_hiz_bg(
                    &state.context.device, &state.hiz_pass.hiz_full_view,
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
                    physical_key: PhysicalKey::Code(KeyCode::Space),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                state.cull_enabled = !state.cull_enabled;
            }
            WindowEvent::RedrawRequested => {
                state.tick();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

struct State {
    window: Window,
    context: GpuContext,
    render_state: RenderState,
    asset_registry: AssetRegistry,
    #[allow(dead_code)]
    ibl_context: IblContext,
    shadow_pass: ShadowPass,
    env_bind_group: wgpu::BindGroup,
    hiz_pass: HizPass,
    cull_pass: CullPass,
    cull_enabled: bool,
    world: World,
    scheduler: Scheduler,
    scene_entities: Vec<u32>,
    entity_phases: Vec<f32>,
    total_time: f32,
    last_instant: std::time::Instant,
}

impl State {
    async fn new(window: Window) -> Self {
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

        use w3gpu_assets::Material;
        asset_registry.upload_material(
            &Material::default(),
            MaterialTextures::default(),
            &context.device,
            &render_state.material_bg_layout,
        );

        let mut world = World::new();

        let workspace = {
            let manifest = env!("CARGO_MANIFEST_DIR");
            std::path::Path::new(manifest).parent().unwrap().parent().unwrap().to_path_buf()
        };

        let glb_path = std::env::args().nth(1).unwrap_or_else(|| {
            workspace.join("www/public/damaged_helmet_source_glb.glb")
                .to_string_lossy().into_owned()
        });

        let scene_pairs: Vec<(u32, u32)> = match std::fs::read(&glb_path) {
            Ok(bytes) => {
                log::info!("Loading GLB: {glb_path}");
                match load_from_bytes(&bytes) {
                    Ok(prims) => prims.into_iter().map(|prim| {
                        let mesh_id = asset_registry.upload_mesh(
                            &prim.mesh, &context.device, &context.queue,
                        );
                        let textures = MaterialTextures {
                            albedo: prim.albedo_image.map(|img| {
                                asset_registry.upload_texture_rgba8(
                                    &img.data, img.width, img.height, true,
                                    &context.device, &context.queue,
                                )
                            }),
                            normal: prim.normal_image.map(|img| {
                                asset_registry.upload_texture_rgba8(
                                    &img.data, img.width, img.height, false,
                                    &context.device, &context.queue,
                                )
                            }),
                            metallic_roughness: prim.metallic_roughness_image.map(|img| {
                                asset_registry.upload_texture_rgba8(
                                    &img.data, img.width, img.height, false,
                                    &context.device, &context.queue,
                                )
                            }),
                            emissive: prim.emissive_image.map(|img| {
                                asset_registry.upload_texture_rgba8(
                                    &img.data, img.width, img.height, true,
                                    &context.device, &context.queue,
                                )
                            }),
                        };
                        let mat_id = asset_registry.upload_material(
                            &prim.material, textures,
                            &context.device, &render_state.material_bg_layout,
                        );
                        (mesh_id, mat_id)
                    }).collect(),
                    Err(e) => {
                        log::warn!("Failed to parse GLB ({e}), using cube");
                        fallback_cube(&mut asset_registry, &context, &render_state)
                    }
                }
            }
            Err(e) => {
                log::warn!("Cannot read '{glb_path}' ({e}), using cube");
                fallback_cube(&mut asset_registry, &context, &render_state)
            }
        };

        let shadow_pass = ShadowPass::new(&context.device, &render_state.instance_bg_layout);

        let hdr_path = workspace.join("www/public/studio_small_03_2k.hdr");
        let ibl_context = match std::fs::read(&hdr_path) {
            Ok(bytes) => match load_hdr_from_bytes(&bytes) {
                Ok(hdr) => IblContext::from_hdr(&hdr, &context.device, &context.queue),
                Err(e) => {
                    log::warn!("HDR parse failed ({e}), default IBL");
                    IblContext::new_default(&context.device, &context.queue)
                }
            },
            Err(_) => IblContext::new_default(&context.device, &context.queue),
        };

        let env_bind_group = build_env_bind_group(
            &context.device, &render_state.ibl_bg_layout, &ibl_context, &shadow_pass,
        );

        // ── camera ────────────────────────────────────────────────────────────
        let camera = world.create_entity();
        world.add_component(camera, CameraComponent::new(
            60.0, size.width as f32 / size.height as f32, 0.1, 200.0,
        ));
        let eye    = Vec3::new(0.0, 5.0, 16.0);
        let target = Vec3::new(0.0, 0.0, 0.0);
        let (_, cam_rot, _) = glam::Mat4::look_at_rh(eye, target, Vec3::Y)
            .inverse().to_scale_rotation_translation();
        let mut cam_t = TransformComponent::from_position(eye);
        cam_t.rotation = cam_rot;
        cam_t.update_local_matrix();
        world.add_component(camera, cam_t);

        // ── 5×5 helmet grid ───────────────────────────────────────────────────
        const GRID: i32 = 5;
        const SPACING: f32 = 2.4;
        let base_pair = scene_pairs[0];
        let mut scene_entities: Vec<u32> = Vec::new();
        let mut entity_phases: Vec<f32> = Vec::new();
        let base_x = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        for row in 0..GRID {
            for col in 0..GRID {
                let x     = (col - GRID / 2) as f32 * SPACING;
                let z     = (row - GRID / 2) as f32 * SPACING;
                let phase = (row * GRID + col) as f32 * (std::f32::consts::TAU / 25.0);
                let entity = world.create_entity();
                world.add_component(entity, RenderableComponent::new(base_pair.0, base_pair.1));
                let mut t = TransformComponent::from_position(Vec3::new(x, 0.0, z));
                t.rotation = base_x;
                t.update_local_matrix();
                world.add_component(entity, t);
                scene_entities.push(entity);
                entity_phases.push(phase);
            }
        }

        // ── occluder wall (Phase 4.2 demo) ────────────────────────────────────
        // Red metallic slab between Z=-1 and Z=0 — occludes the rear 2 helmet rows
        // (Z = -2.4 and Z = -4.8) from the camera at (0,5,16).
        {
            let wall_mesh = asset_registry.upload_mesh(
                &primitives::cube(), &context.device, &context.queue,
            );
            let wall_mat = asset_registry.upload_material(
                &Material {
                    albedo:    [0.8, 0.05, 0.05, 1.0],
                    metallic:  0.9,
                    roughness: 0.2,
                    ..Default::default()
                },
                MaterialTextures::default(),
                &context.device,
                &render_state.material_bg_layout,
            );
            let wall = world.create_entity();
            world.add_component(wall, RenderableComponent::new(wall_mesh, wall_mat));
            let mut t = TransformComponent::from_position(Vec3::new(0.0, 0.8, -1.2));
            t.scale = Vec3::new(7.0, 3.0, 0.25);
            t.update_local_matrix();
            world.add_component(wall, t);
        }

        // ── ground plane ──────────────────────────────────────────────────────
        {
            let floor_mesh = asset_registry.upload_mesh(
                &primitives::cube(), &context.device, &context.queue,
            );
            let floor_mat = asset_registry.upload_material(
                &Material { albedo: [0.35, 0.35, 0.35, 1.0], roughness: 0.9, ..Default::default() },
                MaterialTextures::default(),
                &context.device,
                &render_state.material_bg_layout,
            );
            let floor = world.create_entity();
            world.add_component(floor, RenderableComponent::new(floor_mesh, floor_mat));
            let mut t = TransformComponent::from_position(Vec3::new(0.0, -1.2, 0.0));
            t.scale = Vec3::new(14.0, 0.05, 14.0);
            t.update_local_matrix();
            world.add_component(floor, t);
        }

        let mut scheduler = Scheduler::new();
        scheduler
            .add_system(transform_system)
            .add_system(camera_system)
            .add_system(frustum_culling_system);

        // ── Phase 4.2: Hi-Z + cull pass ───────────────────────────────────────
        let mut hiz_pass = HizPass::new(
            &context.device,
            &render_state.instance_bg_layout,
            size.width, size.height,
        );
        // Ensure valid initial size (winit may report 0×0 before first resize)
        if size.width == 0 || size.height == 0 {
            hiz_pass.resize(&context.device, 800, 600);
        }
        let mut cull_pass = CullPass::new(&context.device);
        cull_pass.rebuild_hiz_bg(&context.device, &hiz_pass.hiz_full_view);

        Self {
            window,
            context,
            render_state,
            asset_registry,
            ibl_context,
            shadow_pass,
            env_bind_group,
            hiz_pass,
            cull_pass,
            cull_enabled: true,
            world,
            scheduler,
            scene_entities,
            entity_phases,
            total_time: 0.0,
            last_instant: std::time::Instant::now(),
        }
    }

    fn tick(&mut self) {
        let now = std::time::Instant::now();
        let dt  = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;
        self.total_time  += dt;

        // Per-entity staggered Y-spin
        let base_x = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        for (&entity, &phase) in self.scene_entities.iter().zip(&self.entity_phases) {
            if let Some(t) = self.world.get_component_mut::<TransformComponent>(entity) {
                let angle = self.total_time * 0.4 + phase;
                t.rotation = Quat::from_rotation_y(angle) * base_x;
                t.update_local_matrix();
            }
        }

        self.scheduler.run(&mut self.world, dt, self.total_time);

        // Collect entities (with world-space AABBs) → sort → build GPU buffers
        let entities = collect_draw_entities(&self.world, &self.asset_registry);
        let entity_count = entities.len() as u32;
        let (matrices, cull_data, sorted) = build_entity_list(entities);
        let shadow_batches = derive_shadow_batches(&sorted);

        // Upload instance matrices
        if !matrices.is_empty() {
            self.context.queue.write_buffer(
                &self.render_state.instance_buffer, 0,
                bytemuck::cast_slice(&matrices),
            );
        }

        // Upload entity cull data
        if !cull_data.is_empty() {
            self.context.queue.write_buffer(
                &self.cull_pass.entity_cull_buf, 0,
                bytemuck::cast_slice(&cull_data),
            );
        }

        // Upload cull uniforms
        let (view_proj, _) = camera_view_proj(&self.world);
        let cull_uniforms = CullUniforms {
            view_proj:    view_proj.to_cols_array_2d(),
            screen_size:  [self.hiz_pass.width as f32, self.hiz_pass.height as f32],
            entity_count,
            mip_levels:   self.hiz_pass.mip_count,
            cull_enabled: if self.cull_enabled { 1 } else { 0 },
            _pad:         [0; 3],
        };
        self.context.queue.write_buffer(
            &self.cull_pass.cull_uniform_buf, 0,
            bytemuck::bytes_of(&cull_uniforms),
        );

        // Upload camera to HizPass (for z-prepass)
        self.hiz_pass.update_camera(&self.context.queue, view_proj.to_cols_array_2d());

        let cull_str = if self.cull_enabled { "ON" } else { "OFF" };
        self.window.set_title(&format!(
            "w3gpu  |  {} entities  |  GPU Hi-Z: {cull_str}  [SPACE]",
            entity_count,
        ));

        self.render(entity_count, &sorted, &shadow_batches);
    }

    fn render(
        &self,
        entity_count: u32,
        sorted: &[DrawEntity],
        shadow_batches: &[w3gpu_renderer::ShadowBatch],
    ) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => { log::warn!("surface error: {e}"); return; }
        };
        let view = output.texture.create_view(&Default::default());

        // Frame uniforms (PBR main pass)
        let frame_uniforms = build_frame_uniforms(&self.world, self.total_time);
        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer, 0,
            bytemuck::bytes_of(&frame_uniforms),
        );

        // Light uniforms (shadow pass)
        let light_uniforms = build_light_uniforms();
        self.shadow_pass.update_light(&self.context.queue, &light_uniforms);

        let indirect_stride = std::mem::size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut enc = self.context.device.create_command_encoder(&Default::default());

        // ── 1. Z-prepass + Hi-Z pyramid build ─────────────────────────────────
        self.hiz_pass.encode(
            &mut enc,
            &self.render_state.instance_bind_group,
            &self.asset_registry,
            sorted,
        );

        // ── 2. GPU occlusion cull → writes entity_indirect_buf ────────────────
        self.cull_pass.encode(&mut enc, entity_count);

        // ── 3. Shadow depth pass (CPU-batched) ────────────────────────────────
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow pass"),
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
                let Some(m) = self.asset_registry.get_mesh(batch.mesh_id) else { continue };
                rp.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                rp.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(
                    0..m.index_count, 0,
                    batch.first_instance..batch.first_instance + batch.instance_count,
                );
            }
        }

        // ── 4. PBR main pass (GPU-indirect, per entity) ───────────────────────
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 }),
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
                let mat = self.asset_registry.get_material(entity.material_id)
                    .or_else(|| self.asset_registry.get_material(0));
                let Some(mat)  = mat else { continue };
                let Some(mesh) = self.asset_registry.get_mesh(entity.mesh_id) else { continue };
                rp.set_bind_group(2, &mat.bind_group, &[]);
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                // instance_count is 0 (culled) or 1 (visible) — GPU wrote this
                rp.draw_indexed_indirect(
                    &self.cull_pass.entity_indirect_buf,
                    idx as u64 * indirect_stride,
                );
            }
        }

        self.context.queue.submit(std::iter::once(enc.finish()));
        output.present();
    }
}

// ── helpers ────────────────────────────────────────────────────────────────────

fn fallback_cube(
    registry: &mut AssetRegistry,
    context: &GpuContext,
    render_state: &RenderState,
) -> Vec<(u32, u32)> {
    use w3gpu_assets::Material;
    let mesh_id = registry.upload_mesh(&primitives::cube(), &context.device, &context.queue);
    let mat_id  = registry.upload_material(
        &Material::default(), MaterialTextures::default(),
        &context.device, &render_state.material_bg_layout,
    );
    vec![(mesh_id, mat_id)]
}

fn build_light_uniforms() -> LightUniforms {
    let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();
    let light_pos = -light_dir * 20.0;
    let light_view = glam::Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
    let light_proj = glam::Mat4::orthographic_rh(-14.0, 14.0, -14.0, 14.0, 0.1, 60.0);
    LightUniforms {
        view_proj: (light_proj * light_view).to_cols_array_2d(),
        shadow_bias: 0.001,
        _pad: [0.0; 3],
    }
}

fn camera_view_proj(world: &World) -> (glam::Mat4, glam::Mat4) {
    world.query_entities::<CameraComponent>().into_iter().find_map(|e| {
        let cam = world.get_component::<CameraComponent>(e)?;
        if cam.is_active { Some((cam.view_matrix, cam.projection_matrix)) } else { None }
    }).unwrap_or((glam::Mat4::IDENTITY, glam::Mat4::IDENTITY))
}

fn build_frame_uniforms(world: &World, total_time: f32) -> FrameUniforms {
    let (view, projection, cam_pos) = world
        .query_entities::<CameraComponent>()
        .into_iter()
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
        _pad3: [0.0; 3],
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
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&ibl.irradiance_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&ibl.prefiltered_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&ibl.brdf_lut_view) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&ibl.sampler) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&shadow.shadow_view) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&shadow.comparison_sampler) },
        ],
    })
}

/// Collect entities from ECS and compute their world-space AABBs.
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
            &world_matrix,
            Vec3::from(mesh.aabb_min),
            Vec3::from(mesh.aabb_max),
        );
        result.push(DrawEntity {
            mesh_id:      r.mesh_id,
            material_id:  r.material_id,
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

/// Transform a local-space AABB by a world matrix → world-space AABB.
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
    let min = ws.iter().copied().fold(Vec3::splat(f32::MAX), Vec3::min);
    let max = ws.iter().copied().fold(Vec3::splat(f32::MIN), Vec3::max);
    (min, max)
}
