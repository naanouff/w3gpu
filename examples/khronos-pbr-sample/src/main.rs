//! Binaire winit : Phase A PBR (même logique que la lib `khronos_pbr`).

use std::path::PathBuf;
use pollster::FutureExt;
use w3drs_input::{winit_adapter::input_event_from_winit, InputAccumulator};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use khronos_pbr::pbr_state::{parse_render_graph_slot, State};

/// `--render-graph PATH`, `--render-graph-readback ID` (défaut `hdr_color`),
/// `--render-graph-slot pre|after_cull|post_pbr` (défaut `after_cull`).
fn parse_render_graph_cli() -> (Option<PathBuf>, String, khronos_pbr::pbr_state::RenderGraphSlot) {
    use khronos_pbr::pbr_state::RenderGraphSlot;
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

fn main() {
    env_logger::init();
    let (render_graph_json, render_graph_readback, render_graph_slot) = parse_render_graph_cli();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        state: None,
        input: InputAccumulator::new(),
        render_graph_json,
        render_graph_readback,
        render_graph_slot,
    };
    event_loop.run_app(&mut app).unwrap();
}

struct App {
    state: Option<State>,
    input: InputAccumulator,
    render_graph_json: Option<PathBuf>,
    render_graph_readback: String,
    render_graph_slot: khronos_pbr::pbr_state::RenderGraphSlot,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes().with_title("khronos-pbr-sample — Phase A GLB viewer"),
            )
            .unwrap();
        self.state = Some(
            State::new_winit(
                window,
                self.render_graph_json.clone(),
                self.render_graph_readback.clone(),
                self.render_graph_slot,
            )
            .block_on(),
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if let Some(input_event) = input_event_from_winit(&event) {
            self.input.begin_frame();
            self.input.consume_event(input_event, false);
            state.orbit.apply_input(&self.input.frame());
            if let Some(w) = &state.window {
                w.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                let w = size.width.max(1);
                let h = size.height.max(1);
                state.resize(w, h);
                if let Some(ww) = &state.window {
                    ww.request_redraw();
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        repeat,
                        ..
                    },
                ..
            } => match code {
                KeyCode::ArrowLeft if !repeat => state.prev_sample(),
                KeyCode::ArrowRight if !repeat => state.next_sample(),
                _ => {}
            },

            WindowEvent::RedrawRequested => {
                state.tick();
                if let Some(ww) = &state.window {
                    ww.request_redraw();
                }
            }
            _ => {}
        }
    }
}
