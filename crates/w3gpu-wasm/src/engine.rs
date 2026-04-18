use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use w3gpu_ecs::{World, Scheduler, Entity};
use w3gpu_renderer::{GpuContext, TRIANGLE_WGSL};

#[wasm_bindgen]
pub struct W3gpuEngine {
    world: World,
    scheduler: Scheduler,
    context: GpuContext,
    triangle_pipeline: wgpu::RenderPipeline,
    total_time: f32,
}

#[wasm_bindgen]
impl W3gpuEngine {
    /// Async constructor — awaitable from TypeScript: `await new W3gpuEngine("canvas-id")`
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

        let triangle_pipeline = build_triangle_pipeline(&context);

        Ok(W3gpuEngine {
            world: World::new(),
            scheduler: Scheduler::new(),
            context,
            triangle_pipeline,
            total_time: 0.0,
        })
    }

    pub fn create_entity(&mut self) -> u32 {
        self.world.create_entity()
    }

    pub fn destroy_entity(&mut self, entity: u32) {
        self.world.destroy_entity(entity);
    }

    pub fn tick(&mut self, delta_time: f32) {
        self.total_time += delta_time;
        self.scheduler.run(&mut self.world, delta_time, self.total_time);
        self.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.context.resize(width, height);
    }

    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

impl W3gpuEngine {
    fn render(&self) {
        let output = match self.context.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.context.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("frame") }
        );
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.05, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rpass.set_pipeline(&self.triangle_pipeline);
            rpass.draw(0..3, 0..1);
        }
        self.context.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

fn build_triangle_pipeline(ctx: &GpuContext) -> wgpu::RenderPipeline {
    let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("triangle"),
        source: wgpu::ShaderSource::Wgsl(TRIANGLE_WGSL.into()),
    });
    let layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("triangle pipeline"),
        layout: Some(&layout),
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
                format: ctx.surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn get_canvas(id: &str) -> Result<web_sys::HtmlCanvasElement, JsValue> {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
    let elem = document.get_element_by_id(id)
        .ok_or_else(|| JsValue::from_str(&format!("canvas '{}' not found", id)))?;
    elem.dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not a canvas"))
}
