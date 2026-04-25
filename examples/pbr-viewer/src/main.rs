//! Viewer PBR **natif** : pipeline 3D + **Phase B** (graphe JSON + ombre B.7, défaut
//! `fixtures/phases/phase-b/render_graph.json`, ou `--render-graph` comme `khronos-pbr-sample`) +
//! **panneau egui** (GLB/HDR, sliders, Hi-Z), aligné sur `www/src/viewer/ui.ts`.

mod pbr_state;

use pollster::FutureExt;
use w3drs_input::{winit_adapter::input_event_from_winit, InputAccumulator};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

struct App {
    state: Option<pbr_state::PbrState>,
    input: InputAccumulator,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes().with_title("pbr-viewer — panneau à gauche (egui)"),
            )
            .expect("winit: create_window");
        self.state = Some(pbr_state::PbrState::new(window).block_on());
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }

            WindowEvent::Resized(size) => {
                let w = size.width.max(1);
                let h = size.height.max(1);
                state.resize(w, h);
                let _ = state.on_egui_window_event(&event);
                state.window.request_redraw();
                return;
            }

            _ => {}
        }

        let egui_resp = state.on_egui_window_event(&event);

        if let Some(input_event) = input_event_from_winit(&event) {
            self.input.begin_frame();
            self.input.consume_event(input_event, egui_resp.consumed);
            state.apply_camera_input(&self.input.frame());
        }

        match &event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        repeat,
                        ..
                    },
                ..
            } => {
                let wants_kb = state.egui_context().wants_keyboard_input();
                match *code {
                    KeyCode::ArrowLeft if !*repeat && !wants_kb => state.prev_sample(),
                    KeyCode::ArrowRight if !*repeat && !wants_kb => state.next_sample(),
                    KeyCode::Space if !*repeat && !wants_kb => state.toggle_gpu_occlusion(),
                    _ => {}
                }
            }

            WindowEvent::RedrawRequested => {
                state.tick();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().expect("winit: EventLoop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        state: None,
        input: InputAccumulator::new(),
    };
    event_loop.run_app(&mut app).expect("winit: run");
}
