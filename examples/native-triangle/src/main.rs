use glam::{Quat, Vec3};
use pollster::FutureExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use w3gpu_assets::primitives;
use w3gpu_ecs::{
    Scheduler, World,
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
};
use w3gpu_renderer::{
    AssetRegistry, FrameUniforms, GpuContext, ObjectUniforms, RenderCommand, RenderState,
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
            .create_window(Window::default_attributes().with_title("w3gpu — rotating cube"))
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
    world: World,
    scheduler: Scheduler,
    cube_entity: u32,
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

        let mut asset_registry = AssetRegistry::new();
        let cube_mesh_id = asset_registry.upload_mesh(
            &primitives::cube(),
            &context.device,
            &context.queue,
        );

        let mut world = World::new();

        // Camera at (0, 0, 5)
        let camera = world.create_entity();
        world.add_component(camera, CameraComponent::new(60.0, size.width as f32 / size.height as f32, 0.1, 1000.0));
        let mut cam_t = TransformComponent::from_position(Vec3::new(0.0, 0.0, 5.0));
        cam_t.update_local_matrix();
        world.add_component(camera, cam_t);

        // Cube at origin
        let cube_entity = world.create_entity();
        world.add_component(cube_entity, RenderableComponent::new(cube_mesh_id, 0));
        world.add_component(cube_entity, TransformComponent::default());

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
            world,
            scheduler,
            cube_entity,
            total_time: 0.0,
            last_instant: std::time::Instant::now(),
        }
    }

    fn tick(&mut self) {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;
        self.total_time += dt;

        // Rotate cube around Y axis
        let angle = self.total_time * 0.8;
        let rot = Quat::from_rotation_y(angle);
        if let Some(t) = self.world.get_component_mut::<TransformComponent>(self.cube_entity) {
            t.rotation = rot;
            t.update_local_matrix();
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

        // FrameUniforms
        let frame_uniforms = build_frame_uniforms(&self.world, self.total_time);
        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer, 0,
            bytemuck::bytes_of(&frame_uniforms),
        );

        // Render commands
        let commands = collect_render_commands(&self.world);

        for (i, cmd) in commands.iter().enumerate() {
            self.context.queue.write_buffer(
                &self.render_state.object_uniform_buffer,
                i as u64 * OBJECT_ALIGN,
                bytemuck::bytes_of(&ObjectUniforms { world: cmd.world_matrix }),
            );
        }

        let mut enc = self.context.device.create_command_encoder(&Default::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
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

            for (i, cmd) in commands.iter().enumerate() {
                let offset = (i as u32) * OBJECT_ALIGN as u32;
                rp.set_bind_group(1, &self.render_state.object_bind_group, &[offset]);
                let mat_bg = self.asset_registry
                    .get_material(cmd.material_id)
                    .map(|m| &m.bind_group)
                    .unwrap_or(&self.render_state.fallback_material_bind_group);
                rp.set_bind_group(2, mat_bg, &[]);
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
    let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();

    FrameUniforms {
        projection: projection.to_cols_array_2d(),
        view: view.to_cols_array_2d(),
        inv_view_projection: inv_vp.to_cols_array_2d(),
        camera_position: cam_pos.to_array(),
        _pad0: 0.0,
        light_direction: light_dir.to_array(),
        _pad1: 0.0,
        light_color: [1.0, 0.95, 0.9],
        ambient_intensity: 0.12,
        total_time,
        _pad2: [0.0; 3],
    }
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
