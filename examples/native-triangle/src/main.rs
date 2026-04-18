use glam::{Quat, Vec3};
use pollster::FutureExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use w3gpu_assets::{load_from_bytes, load_hdr_from_bytes, primitives};
use w3gpu_ecs::{
    Scheduler, World,
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
};
use w3gpu_renderer::{
    AssetRegistry, FrameUniforms, GpuContext, IblContext, LightUniforms, MaterialTextures,
    ObjectUniforms, RenderCommand, RenderState, ShadowPass,
    camera_system, frustum_culling_system, transform_system, OBJECT_ALIGN,
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
            .create_window(Window::default_attributes().with_title("w3gpu — native client"))
            .unwrap();
        self.state = Some(State::new(window).block_on());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.context.resize(size.width, size.height);
                for e in state.world.query_entities::<CameraComponent>() {
                    if let Some(cam) = state.world.get_component_mut::<CameraComponent>(e) {
                        cam.aspect = size.width as f32 / size.height as f32;
                    }
                }
                state.window.request_redraw();
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
    ibl_context: IblContext,
    shadow_pass: ShadowPass,
    env_bind_group: wgpu::BindGroup,
    world: World,
    scheduler: Scheduler,
    scene_entities: Vec<u32>,
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
            std::path::Path::new(manifest)
                .parent().unwrap()
                .parent().unwrap()
                .to_path_buf()
        };

        let glb_path = std::env::args().nth(1).unwrap_or_else(|| {
            workspace.join("www/public/damaged_helmet_source_glb.glb").to_string_lossy().into_owned()
        });

        let scene_pairs: Vec<(u32, u32)> = match std::fs::read(&glb_path) {
            Ok(bytes) => {
                log::info!("Loading GLB: {glb_path}");
                match load_from_bytes(&bytes) {
                    Ok(prims) => prims
                        .into_iter()
                        .map(|prim| {
                            let mesh_id = asset_registry.upload_mesh(&prim.mesh, &context.device, &context.queue);
                            let textures = MaterialTextures {
                                albedo: prim.albedo_image.map(|img| {
                                    asset_registry.upload_texture_rgba8(&img.data, img.width, img.height, true, &context.device, &context.queue)
                                }),
                                normal: prim.normal_image.map(|img| {
                                    asset_registry.upload_texture_rgba8(&img.data, img.width, img.height, false, &context.device, &context.queue)
                                }),
                                metallic_roughness: prim.metallic_roughness_image.map(|img| {
                                    asset_registry.upload_texture_rgba8(&img.data, img.width, img.height, false, &context.device, &context.queue)
                                }),
                                emissive: prim.emissive_image.map(|img| {
                                    asset_registry.upload_texture_rgba8(&img.data, img.width, img.height, true, &context.device, &context.queue)
                                }),
                            };
                            let mat_id = asset_registry.upload_material(&prim.material, textures, &context.device, &render_state.material_bg_layout);
                            (mesh_id, mat_id)
                        })
                        .collect(),
                    Err(e) => {
                        log::warn!("Failed to parse GLB ({e}), falling back to cube");
                        fallback_cube(&mut asset_registry, &context, &render_state)
                    }
                }
            }
            Err(e) => {
                log::warn!("Cannot read '{glb_path}' ({e}), falling back to cube");
                fallback_cube(&mut asset_registry, &context, &render_state)
            }
        };

        let shadow_pass = ShadowPass::new(
            &context.device,
            &render_state.object_bg_layout,
        );

        // IBL: load HDR if present, otherwise use default
        let hdr_path = workspace.join("www/public/studio_small_03_2k.hdr");
        let ibl_context = match std::fs::read(&hdr_path) {
            Ok(bytes) => {
                log::info!("Loading HDR: {}", hdr_path.display());
                match load_hdr_from_bytes(&bytes) {
                    Ok(hdr) => IblContext::from_hdr(&hdr, &context.device, &context.queue),
                    Err(e) => {
                        log::warn!("Failed to parse HDR ({e}), using default IBL");
                        IblContext::new_default(&context.device, &context.queue)
                    }
                }
            }
            Err(_) => {
                log::info!("No HDR found at {}, using default IBL", hdr_path.display());
                IblContext::new_default(&context.device, &context.queue)
            }
        };

        let env_bind_group = build_env_bind_group(
            &context.device,
            &render_state.ibl_bg_layout,
            &ibl_context,
            &shadow_pass,
        );

        // Camera
        let camera = world.create_entity();
        world.add_component(camera, CameraComponent::new(
            60.0, size.width as f32 / size.height as f32, 0.1, 1000.0,
        ));
        let mut cam_t = TransformComponent::from_position(Vec3::new(0.0, 0.0, 3.0));
        cam_t.update_local_matrix();
        world.add_component(camera, cam_t);

        let scene_entities: Vec<u32> = scene_pairs
            .into_iter()
            .map(|(mesh_id, mat_id)| {
                let entity = world.create_entity();
                world.add_component(entity, RenderableComponent::new(mesh_id, mat_id));
                world.add_component(entity, TransformComponent::default());
                entity
            })
            .collect();

        // Ground plane
        {
            use w3gpu_assets::Material;
            let floor_mesh = asset_registry.upload_mesh(&primitives::cube(), &context.device, &context.queue);
            let floor_mat  = asset_registry.upload_material(
                &Material { albedo: [0.45, 0.45, 0.45, 1.0], roughness: 0.8, ..Default::default() },
                MaterialTextures::default(),
                &context.device,
                &render_state.material_bg_layout,
            );
            let floor = world.create_entity();
            world.add_component(floor, RenderableComponent::new(floor_mesh, floor_mat));
            let mut t = TransformComponent::from_position(Vec3::new(0.0, -1.2, 0.0));
            t.scale = Vec3::new(4.0, 0.05, 4.0);
            t.update_local_matrix();
            world.add_component(floor, t);
        }

        let mut scheduler = Scheduler::new();
        scheduler
            .add_system(transform_system)
            .add_system(camera_system)
            .add_system(frustum_culling_system);

        Self {
            window,
            context,
            render_state,
            asset_registry,
            ibl_context,
            shadow_pass,
            env_bind_group,
            world,
            scheduler,
            scene_entities,
            total_time: 0.0,
            last_instant: std::time::Instant::now(),
        }
    }

    fn tick(&mut self) {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;
        self.total_time += dt;

        let angle = self.total_time * 0.4;
        // -90° around X as base, then spin around world Y
        let base_x = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let y_spin = Quat::from_rotation_y(angle);
        let rot = y_spin * base_x;

        for &entity in &self.scene_entities {
            if let Some(t) = self.world.get_component_mut::<TransformComponent>(entity) {
                t.rotation = rot;
                t.update_local_matrix();
            }
        }

        self.scheduler.run(&mut self.world, dt, self.total_time);
        self.render();
    }

    fn render(&self) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => { log::warn!("surface error: {e}"); return; }
        };
        let view = output.texture.create_view(&Default::default());

        let frame_uniforms = build_frame_uniforms(&self.world, self.total_time);
        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer, 0,
            bytemuck::bytes_of(&frame_uniforms),
        );

        let light_uniforms = build_light_uniforms();
        self.shadow_pass.update_light(&self.context.queue, &light_uniforms);

        let commands = collect_render_commands(&self.world);
        for (i, cmd) in commands.iter().enumerate() {
            self.context.queue.write_buffer(
                &self.render_state.object_uniform_buffer,
                i as u64 * OBJECT_ALIGN,
                bytemuck::bytes_of(&ObjectUniforms { world: cmd.world_matrix }),
            );
        }

        let mut enc = self.context.device.create_command_encoder(&Default::default());

        // ── pass 1 : shadow depth ──────────────────────────────────────────────
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
            for (i, cmd) in commands.iter().enumerate() {
                if !cmd.cast_shadow { continue; }
                let offset = (i as u32) * OBJECT_ALIGN as u32;
                rp.set_bind_group(1, &self.render_state.object_bind_group, &[offset]);
                if let Some(m) = self.asset_registry.get_mesh(cmd.mesh_id) {
                    rp.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                    rp.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..m.index_count, 0, 0..1);
                }
            }
        }

        // ── pass 2 : PBR main ───────────────────────────��─────────────────────
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
            rp.set_bind_group(3, &self.env_bind_group, &[]);

            for (i, cmd) in commands.iter().enumerate() {
                let offset = (i as u32) * OBJECT_ALIGN as u32;
                rp.set_bind_group(1, &self.render_state.object_bind_group, &[offset]);
                let mat = self.asset_registry.get_material(cmd.material_id)
                    .or_else(|| self.asset_registry.get_material(0));
                let Some(mat) = mat else { continue };
                rp.set_bind_group(2, &mat.bind_group, &[]);
                if let Some(m) = self.asset_registry.get_mesh(cmd.mesh_id) {
                    rp.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                    rp.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..m.index_count, 0, 0..1);
                }
            }
        }

        self.context.queue.submit(std::iter::once(enc.finish()));
        output.present();
    }
}

// ── helpers ────────────────────────────────────────────────────────────────

fn fallback_cube(
    registry: &mut AssetRegistry,
    context: &GpuContext,
    render_state: &RenderState,
) -> Vec<(u32, u32)> {
    use w3gpu_assets::Material;
    let mesh_id = registry.upload_mesh(&primitives::cube(), &context.device, &context.queue);
    let mat_id  = registry.upload_material(
        &Material::default(),
        MaterialTextures::default(),
        &context.device,
        &render_state.material_bg_layout,
    );
    vec![(mesh_id, mat_id)]
}

fn build_light_uniforms() -> LightUniforms {
    let light_dir = glam::Vec3::new(-0.5, -1.0, -0.5).normalize();
    let light_pos = -light_dir * 20.0;
    let light_view = glam::Mat4::look_at_rh(light_pos, glam::Vec3::ZERO, glam::Vec3::Y);
    let light_proj = glam::Mat4::orthographic_rh(-10.0, 10.0, -10.0, 10.0, 0.1, 50.0);
    LightUniforms {
        view_proj: (light_proj * light_view).to_cols_array_2d(),
        shadow_bias: 0.001,
        _pad: [0.0; 3],
    }
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
    let light_uniforms = build_light_uniforms();

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
        light_view_proj: light_uniforms.view_proj,
        shadow_bias: light_uniforms.shadow_bias,
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

fn collect_render_commands(world: &World) -> Vec<RenderCommand> {
    let entities = world.query_entities::<RenderableComponent>();
    let mut commands = Vec::with_capacity(entities.len());
    for entity in entities {
        if world.has_component::<CulledComponent>(entity) { continue; }
        let (mesh_id, material_id, cast_shadow) = match world.get_component::<RenderableComponent>(entity) {
            Some(r) if r.visible => (r.mesh_id, r.material_id, r.cast_shadow),
            _ => continue,
        };
        let world_matrix = world.get_component::<TransformComponent>(entity)
            .map(|t| t.world_matrix)
            .unwrap_or(glam::Mat4::IDENTITY);
        commands.push(RenderCommand { mesh_id, material_id, world_matrix: world_matrix.to_cols_array_2d(), cast_shadow });
    }
    commands
}
