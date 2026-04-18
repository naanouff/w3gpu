use glam::{Mat4, Quat, Vec3};
use wasm_bindgen::prelude::*;
use w3gpu_assets::{load_from_bytes, load_hdr_from_bytes, primitives, Material};
use w3gpu_ecs::{
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
    Scheduler, World,
};
use w3gpu_renderer::{
    camera_system, frustum_culling_system, transform_system,
    AssetRegistry, FrameUniforms, GpuContext, IblContext, LightUniforms, MaterialTextures,
    ObjectUniforms, RenderCommand, RenderState, ShadowPass, OBJECT_ALIGN,
};

#[wasm_bindgen]
pub struct W3gpuEngine {
    world: World,
    scheduler: Scheduler,
    context: GpuContext,
    asset_registry: AssetRegistry,
    render_state: RenderState,
    ibl_context: IblContext,
    shadow_pass: ShadowPass,
    env_bind_group: wgpu::BindGroup,
    total_time: f32,
}

#[wasm_bindgen]
impl W3gpuEngine {
    pub async fn create(canvas_id: &str) -> Result<W3gpuEngine, JsValue> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let canvas = get_canvas(canvas_id)?;
        let width = canvas.width();
        let height = canvas.height();

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let context = GpuContext::new(&instance, surface, width, height)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let render_state = RenderState::new(&context.device, context.surface_format);
        let mut asset_registry = AssetRegistry::new(&context.device, &context.queue);

        asset_registry.upload_material(
            &Material::default(),
            MaterialTextures::default(),
            &context.device,
            &render_state.material_bg_layout,
        );

        let ibl_context = IblContext::new_default(
            &context.device,
            &context.queue,
        );

        let shadow_pass = ShadowPass::new(
            &context.device,
            &render_state.object_bg_layout,
        );

        let env_bind_group = build_env_bind_group(
            &context.device,
            &render_state.ibl_bg_layout,
            &ibl_context,
            &shadow_pass,
        );

        let mut scheduler = Scheduler::new();
        scheduler
            .add_system(transform_system)
            .add_system(camera_system)
            .add_system(frustum_culling_system);

        Ok(W3gpuEngine {
            world: World::new(),
            scheduler,
            context,
            asset_registry,
            render_state,
            ibl_context,
            shadow_pass,
            env_bind_group,
            total_time: 0.0,
        })
    }

    // ── entity API ────────────────────────────────────────────────────────

    pub fn create_entity(&mut self) -> u32 {
        self.world.create_entity()
    }

    pub fn destroy_entity(&mut self, entity: u32) {
        self.world.destroy_entity(entity);
    }

    pub fn set_transform(
        &mut self, entity: u32,
        px: f32, py: f32, pz: f32,
        qx: f32, qy: f32, qz: f32, qw: f32,
        sx: f32, sy: f32, sz: f32,
    ) {
        let mut t = TransformComponent::new(
            Vec3::new(px, py, pz),
            Quat::from_xyzw(qx, qy, qz, qw),
            Vec3::new(sx, sy, sz),
        );
        t.update_local_matrix();
        self.world.add_component(entity, t);
    }

    pub fn set_mesh_renderer(&mut self, entity: u32, mesh_id: u32, material_id: u32) {
        self.world
            .add_component(entity, RenderableComponent::new(mesh_id, material_id));
    }

    pub fn add_camera(
        &mut self, entity: u32,
        fov_degrees: f32, aspect: f32, near: f32, far: f32,
    ) {
        self.world
            .add_component(entity, CameraComponent::new(fov_degrees, aspect, near, far));
    }

    // ── asset API ────────────────────────────────────────────────────────

    pub fn upload_cube_mesh(&mut self) -> u32 {
        let mesh = primitives::cube();
        self.asset_registry
            .upload_mesh(&mesh, &self.context.device, &self.context.queue)
    }

    pub fn upload_material(
        &mut self,
        r: f32, g: f32, b: f32, a: f32,
        metallic: f32, roughness: f32,
        er: f32, eg: f32, eb: f32,
    ) -> u32 {
        let mat = Material {
            albedo: [r, g, b, a],
            metallic,
            roughness,
            emissive: [er, eg, eb],
            ..Default::default()
        };
        self.asset_registry.upload_material(
            &mat,
            MaterialTextures::default(),
            &self.context.device,
            &self.render_state.material_bg_layout,
        )
    }

    /// Load all mesh/material pairs from a GLB byte slice (textures included).
    /// Returns a flat array [mesh_id0, mat_id0, mesh_id1, mat_id1, …].
    pub fn load_gltf(&mut self, bytes: &[u8]) -> Result<Vec<u32>, JsValue> {
        let primitives = load_from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut ids = Vec::with_capacity(primitives.len() * 2);
        for prim in primitives {
            let mesh_id = self.asset_registry
                .upload_mesh(&prim.mesh, &self.context.device, &self.context.queue);

            let textures = MaterialTextures {
                albedo: prim.albedo_image.map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data, img.width, img.height, true,
                        &self.context.device, &self.context.queue,
                    )
                }),
                normal: prim.normal_image.map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data, img.width, img.height, false,
                        &self.context.device, &self.context.queue,
                    )
                }),
                metallic_roughness: prim.metallic_roughness_image.map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data, img.width, img.height, false,
                        &self.context.device, &self.context.queue,
                    )
                }),
                emissive: prim.emissive_image.map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data, img.width, img.height, true,
                        &self.context.device, &self.context.queue,
                    )
                }),
            };

            let mat_id = self.asset_registry.upload_material(
                &prim.material,
                textures,
                &self.context.device,
                &self.render_state.material_bg_layout,
            );

            ids.push(mesh_id);
            ids.push(mat_id);
        }
        Ok(ids)
    }

    /// Load an equirectangular HDR image and precompute IBL (irradiance + prefiltered + BRDF LUT).
    pub fn load_hdr(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let hdr = load_hdr_from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.ibl_context = IblContext::from_hdr(
            &hdr,
            &self.context.device,
            &self.context.queue,
        );
        self.env_bind_group = build_env_bind_group(
            &self.context.device,
            &self.render_state.ibl_bg_layout,
            &self.ibl_context,
            &self.shadow_pass,
        );
        Ok(())
    }

    // ── frame ─────────────────────────────────────────────────────────────

    pub fn tick(&mut self, delta_time: f32) {
        self.total_time += delta_time;
        self.scheduler.run(&mut self.world, delta_time, self.total_time);
        self.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        for entity in self.world.query_entities::<CameraComponent>() {
            if let Some(cam) = self.world.get_component_mut::<CameraComponent>(entity) {
                if cam.is_active {
                    cam.aspect = width as f32 / height as f32;
                }
            }
        }
        self.context.resize(width, height);
    }

    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

// ── private rendering ─────────────────────────────────────────────────────────

impl W3gpuEngine {
    fn render(&self) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // upload per-frame uniforms
        let frame_uniforms = self.build_frame_uniforms();
        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer, 0,
            bytemuck::bytes_of(&frame_uniforms),
        );

        let light_uniforms = Self::build_light_uniforms();
        self.shadow_pass.update_light(&self.context.queue, &light_uniforms);

        let commands = self.collect_render_commands();
        for (i, cmd) in commands.iter().enumerate() {
            self.context.queue.write_buffer(
                &self.render_state.object_uniform_buffer,
                i as u64 * OBJECT_ALIGN,
                bytemuck::bytes_of(&ObjectUniforms { world: cmd.world_matrix }),
            );
        }

        let mut encoder = self.context.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // ── pass 1 : shadow depth ─────────────────────────────────────────────
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            rpass.set_pipeline(&self.shadow_pass.depth_pipeline);
            rpass.set_bind_group(0, &self.shadow_pass.shadow_light_bind_group, &[]);

            for (i, cmd) in commands.iter().enumerate() {
                if !cmd.cast_shadow { continue; }
                let offset = (i as u32) * OBJECT_ALIGN as u32;
                rpass.set_bind_group(1, &self.render_state.object_bind_group, &[offset]);
                if let Some(gpu_mesh) = self.asset_registry.get_mesh(cmd.mesh_id) {
                    rpass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    rpass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
                }
            }
        }

        // ── pass 2 : PBR main ─────────────────────────────────────────────────
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            rpass.set_pipeline(&self.render_state.pipeline);
            rpass.set_bind_group(0, &self.render_state.frame_bind_group, &[]);
            rpass.set_bind_group(3, &self.env_bind_group, &[]);

            for (i, cmd) in commands.iter().enumerate() {
                let offset = (i as u32) * OBJECT_ALIGN as u32;
                rpass.set_bind_group(1, &self.render_state.object_bind_group, &[offset]);

                if let Some(mat) = self.asset_registry.get_material(cmd.material_id) {
                    rpass.set_bind_group(2, &mat.bind_group, &[]);
                } else if let Some(default_mat) = self.asset_registry.get_material(0) {
                    rpass.set_bind_group(2, &default_mat.bind_group, &[]);
                } else {
                    continue;
                }

                if let Some(gpu_mesh) = self.asset_registry.get_mesh(cmd.mesh_id) {
                    rpass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    rpass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
                }
            }
        }

        self.context.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn build_light_uniforms() -> LightUniforms {
        let light_dir = glam::Vec3::new(-0.5, -1.0, -0.5).normalize();
        let light_pos = -light_dir * 20.0;
        let light_view = glam::Mat4::look_at_rh(light_pos, glam::Vec3::ZERO, glam::Vec3::Y);
        let light_proj = glam::Mat4::orthographic_rh(-10.0, 10.0, -10.0, 10.0, 0.1, 50.0);
        LightUniforms {
            view_proj: (light_proj * light_view).to_cols_array_2d(),
            shadow_bias: 0.005,
            _pad: [0.0; 3],
        }
    }

    fn build_frame_uniforms(&self) -> FrameUniforms {
        let (view, projection, cam_pos) = self
            .world
            .query_entities::<CameraComponent>()
            .into_iter()
            .find_map(|e| {
                let cam = self.world.get_component::<CameraComponent>(e)?;
                if !cam.is_active { return None; }
                let pos = self.world.get_component::<TransformComponent>(e)
                    .map(|t| { let w = t.world_matrix.w_axis; Vec3::new(w.x, w.y, w.z) })
                    .unwrap_or(Vec3::ZERO);
                Some((cam.view_matrix, cam.projection_matrix, pos))
            })
            .unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO));

        let inv_vp = (projection * view).inverse();
        let light_uniforms = Self::build_light_uniforms();

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
            total_time: self.total_time,
            _pad2: [0.0; 3],
            light_view_proj: light_uniforms.view_proj,
            shadow_bias: light_uniforms.shadow_bias,
            _pad3: [0.0; 3],
        }
    }

    fn collect_render_commands(&self) -> Vec<RenderCommand> {
        let entities = self.world.query_entities::<RenderableComponent>();
        let mut commands = Vec::with_capacity(entities.len());
        for entity in entities {
            if self.world.has_component::<CulledComponent>(entity) { continue; }
            let (mesh_id, material_id, cast_shadow) = match self.world.get_component::<RenderableComponent>(entity) {
                Some(r) if r.visible => (r.mesh_id, r.material_id, r.cast_shadow),
                _ => continue,
            };
            let world_matrix = self.world.get_component::<TransformComponent>(entity)
                .map(|t| t.world_matrix)
                .unwrap_or(Mat4::IDENTITY);
            commands.push(RenderCommand {
                mesh_id, material_id,
                world_matrix: world_matrix.to_cols_array_2d(),
                cast_shadow,
            });
        }
        commands
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

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

fn get_canvas(id: &str) -> Result<web_sys::HtmlCanvasElement, JsValue> {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
    let elem = document
        .get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("canvas '{}' not found", id)))?;
    elem.dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not a canvas"))
}
