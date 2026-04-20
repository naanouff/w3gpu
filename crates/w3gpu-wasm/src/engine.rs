use glam::{Mat4, Quat, Vec3, Vec4};
use wasm_bindgen::prelude::*;
use w3gpu_assets::{load_from_bytes, load_hdr_from_bytes, primitives, Material};
use w3gpu_ecs::{
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
    Scheduler, World,
};
use w3gpu_renderer::{
    build_entity_list, derive_shadow_batches,
    AssetRegistry, BloomParams, CullPass, CullUniforms, DrawEntity,
    DrawIndexedIndirectArgs, FrameUniforms, GpuContext, HdrTarget, HizPass,
    IblContext, LightUniforms, MaterialTextures, PostProcessPass, RenderState,
    ShadowPass, TonemapParams,
    camera_system, frustum_culling_system, transform_system,
};

#[wasm_bindgen]
pub struct W3gpuEngine {
    world:          World,
    scheduler:      Scheduler,
    context:        GpuContext,
    asset_registry: AssetRegistry,
    render_state:   RenderState,
    ibl_context:    IblContext,
    shadow_pass:    ShadowPass,
    env_bind_group: wgpu::BindGroup,
    hiz_pass:       HizPass,
    cull_pass:      CullPass,
    hdr_target:     HdrTarget,
    post_process:   PostProcessPass,
    cull_enabled:   bool,
    total_time:     f32,
}

#[wasm_bindgen]
impl W3gpuEngine {
    pub async fn create(canvas_id: &str) -> Result<W3gpuEngine, JsValue> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let canvas = get_canvas(canvas_id)?;
        let width  = canvas.width().max(1);
        let height = canvas.height().max(1);

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

        let ibl_context = IblContext::new_default(&context.device, &context.queue);
        let shadow_pass = ShadowPass::new(&context.device, &render_state.instance_bg_layout);
        let env_bind_group = build_env_bind_group(
            &context.device, &render_state.ibl_bg_layout, &ibl_context, &shadow_pass,
        );

        let hiz_pass = HizPass::new(
            &context.device,
            &render_state.instance_bg_layout,
            width, height,
        );
        let mut cull_pass = CullPass::new(&context.device);
        cull_pass.rebuild_hiz_bg(&context.device, &hiz_pass.hiz_full_view);

        let hdr_target = HdrTarget::new(&context.device, width, height);
        let post_process = PostProcessPass::new(
            &context.device,
            &hdr_target.view,
            context.surface_format,
            width, height,
            BloomParams   { threshold: 1.0, knee: 0.5,  _pad0: 0.0, _pad1: 0.0 },
            TonemapParams { exposure: 1.0, bloom_strength: 0.04, _pad0: 0.0, _pad1: 0.0 },
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
            hiz_pass,
            cull_pass,
            hdr_target,
            post_process,
            cull_enabled: true,
            total_time: 0.0,
        })
    }

    // ── entity API ────────────────────────────────────────────────────────────

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
        self.world.add_component(entity, RenderableComponent::new(mesh_id, material_id));
    }

    pub fn add_camera(
        &mut self, entity: u32,
        fov_degrees: f32, aspect: f32, near: f32, far: f32,
    ) {
        self.world.add_component(entity, CameraComponent::new(fov_degrees, aspect, near, far));
    }

    // ── asset API ────────────────────────────────────────────────────────────

    pub fn upload_cube_mesh(&mut self) -> u32 {
        self.asset_registry.upload_mesh(
            &primitives::cube(), &self.context.device, &self.context.queue,
        )
    }

    pub fn upload_material(
        &mut self,
        r: f32, g: f32, b: f32, a: f32,
        metallic: f32, roughness: f32,
        er: f32, eg: f32, eb: f32,
    ) -> u32 {
        let mat = Material {
            albedo: [r, g, b, a], metallic, roughness,
            emissive: [er, eg, eb],
            ..Default::default()
        };
        self.asset_registry.upload_material(
            &mat, MaterialTextures::default(),
            &self.context.device, &self.render_state.material_bg_layout,
        )
    }

    pub fn load_gltf(&mut self, bytes: &[u8]) -> Result<Vec<u32>, JsValue> {
        let primitives = load_from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut ids = Vec::with_capacity(primitives.len() * 2);
        for prim in primitives {
            let mesh_id = self.asset_registry.upload_mesh(
                &prim.mesh, &self.context.device, &self.context.queue,
            );
            let textures = MaterialTextures {
                albedo: prim.albedo_image.map(|img| self.asset_registry.upload_texture_rgba8(
                    &img.data, img.width, img.height, true,
                    &self.context.device, &self.context.queue,
                )),
                normal: prim.normal_image.map(|img| self.asset_registry.upload_texture_rgba8(
                    &img.data, img.width, img.height, false,
                    &self.context.device, &self.context.queue,
                )),
                metallic_roughness: prim.metallic_roughness_image.map(|img| {
                    self.asset_registry.upload_texture_rgba8(
                        &img.data, img.width, img.height, false,
                        &self.context.device, &self.context.queue,
                    )
                }),
                emissive: prim.emissive_image.map(|img| self.asset_registry.upload_texture_rgba8(
                    &img.data, img.width, img.height, true,
                    &self.context.device, &self.context.queue,
                )),
            };
            let mat_id = self.asset_registry.upload_material(
                &prim.material, textures,
                &self.context.device, &self.render_state.material_bg_layout,
            );
            ids.push(mesh_id);
            ids.push(mat_id);
        }
        Ok(ids)
    }

    pub fn load_hdr(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let hdr = load_hdr_from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.ibl_context = IblContext::from_hdr(&hdr, &self.context.device, &self.context.queue);
        self.env_bind_group = build_env_bind_group(
            &self.context.device, &self.render_state.ibl_bg_layout,
            &self.ibl_context, &self.shadow_pass,
        );
        Ok(())
    }

    // ── culling API ───────────────────────────────────────────────────────────

    pub fn set_cull_enabled(&mut self, enabled: bool) {
        self.cull_enabled = enabled;
    }

    // ── frame ─────────────────────────────────────────────────────────────────

    pub fn tick(&mut self, delta_time: f32) {
        self.total_time += delta_time;
        self.scheduler.run(&mut self.world, delta_time, self.total_time);

        let entities    = self.collect_draw_entities();
        let entity_count = entities.len() as u32;
        let (matrices, cull_data, sorted) = build_entity_list(entities);
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

        let (view_proj, _) = self.camera_view_proj();
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
        self.hiz_pass.update_camera(&self.context.queue, view_proj.to_cols_array_2d());

        self.render(entity_count, &sorted, &shadow_batches);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        for entity in self.world.query_entities::<CameraComponent>() {
            if let Some(cam) = self.world.get_component_mut::<CameraComponent>(entity) {
                if cam.is_active { cam.aspect = w as f32 / h as f32; }
            }
        }
        self.context.resize(w, h);
        self.hiz_pass.resize(&self.context.device, w, h);
        self.cull_pass.rebuild_hiz_bg(&self.context.device, &self.hiz_pass.hiz_full_view);
        self.hdr_target.resize(&self.context.device, w, h);
        self.post_process.resize(&self.context.device, &self.hdr_target.view, w, h);
    }

    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

// ── private ───────────────────────────────────────────────────────────────────

impl W3gpuEngine {
    fn render(
        &self,
        entity_count: u32,
        sorted: &[DrawEntity],
        shadow_batches: &[w3gpu_renderer::ShadowBatch],
    ) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let frame_uniforms = self.build_frame_uniforms();
        self.context.queue.write_buffer(
            &self.render_state.frame_uniform_buffer, 0,
            bytemuck::bytes_of(&frame_uniforms),
        );
        let light_uniforms = Self::build_light_uniforms();
        self.shadow_pass.update_light(&self.context.queue, &light_uniforms);

        let indirect_stride = std::mem::size_of::<DrawIndexedIndirectArgs>() as u64;
        let mut encoder = self.context.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // 1. Z-prepass + Hi-Z pyramid
        self.hiz_pass.encode(
            &mut encoder,
            &self.render_state.instance_bind_group,
            &self.asset_registry,
            sorted,
        );

        // 2. GPU occlusion cull
        self.cull_pass.encode(&mut encoder, entity_count);

        // 3. Shadow pass (CPU-batched)
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

        // 4. PBR main pass (GPU-indirect, per entity) → HDR target
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_target.view,
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
                rp.draw_indexed_indirect(
                    &self.cull_pass.entity_indirect_buf,
                    idx as u64 * indirect_stride,
                );
            }
        }

        // 5. Post-process: bloom + ACES tonemap + FXAA → swapchain
        self.post_process.encode(&mut encoder, &view);

        self.context.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn build_light_uniforms() -> LightUniforms {
        let light_dir = Vec3::new(-0.5, -1.0, -0.5).normalize();
        let light_pos = -light_dir * 20.0;
        let light_view = Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
        let light_proj = Mat4::orthographic_rh(-14.0, 14.0, -14.0, 14.0, 0.1, 60.0);
        LightUniforms {
            view_proj: (light_proj * light_view).to_cols_array_2d(),
            shadow_bias: 0.001,
            _pad: [0.0; 3],
        }
    }

    fn camera_view_proj(&self) -> (Mat4, Mat4) {
        self.world.query_entities::<CameraComponent>().into_iter().find_map(|e| {
            let cam = self.world.get_component::<CameraComponent>(e)?;
            if cam.is_active { Some((cam.view_matrix, cam.projection_matrix)) } else { None }
        }).unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY))
    }

    fn build_frame_uniforms(&self) -> FrameUniforms {
        let (view, projection, cam_pos) = self.world
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
        let light  = Self::build_light_uniforms();

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
            light_view_proj: light.view_proj,
            shadow_bias: light.shadow_bias,
            _pad3: [0.0; 3],
        }
    }

    fn collect_draw_entities(&self) -> Vec<DrawEntity> {
        let entities = self.world.query_entities::<RenderableComponent>();
        let mut result = Vec::with_capacity(entities.len());
        for entity in entities {
            if self.world.has_component::<CulledComponent>(entity) { continue; }
            let Some(r) = self.world.get_component::<RenderableComponent>(entity) else { continue };
            if !r.visible { continue; }
            let world_matrix = self.world.get_component::<TransformComponent>(entity)
                .map(|t| t.world_matrix)
                .unwrap_or(Mat4::IDENTITY);
            let Some(mesh) = self.asset_registry.get_mesh(r.mesh_id) else { continue };

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
}

// ── module-level helpers ──────────────────────────────────────────────────────

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
    let ws: Vec<Vec3> = corners.iter().map(|c| {
        let h = mat.mul_vec4(Vec4::new(c.x, c.y, c.z, 1.0));
        Vec3::new(h.x, h.y, h.z)
    }).collect();
    let min = ws.iter().copied().fold(Vec3::splat(f32::MAX), Vec3::min);
    let max = ws.iter().copied().fold(Vec3::splat(f32::MIN), Vec3::max);
    (min, max)
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

fn get_canvas(id: &str) -> Result<web_sys::HtmlCanvasElement, JsValue> {
    use wasm_bindgen::JsCast;
    let window   = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
    let elem     = document.get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("canvas '{}' not found", id)))?;
    elem.dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not a canvas"))
}
