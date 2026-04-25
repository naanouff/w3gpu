//! Normalized input events shared by native viewers and future browser adapters.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerPosition {
    pub x: f32,
    pub y: f32,
}

impl PointerPosition {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerDelta {
    pub dx: f32,
    pub dy: f32,
}

impl PointerDelta {
    pub const fn new(dx: f32, dy: f32) -> Self {
        Self { dx, dy }
    }

    pub fn is_zero(self) -> bool {
        self.dx == 0.0 && self.dy == 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub logo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    PointerButton {
        button: PointerButton,
        pressed: bool,
    },
    PointerMoved {
        position: PointerPosition,
    },
    Wheel {
        lines: f32,
    },
    ModifiersChanged(Modifiers),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputFrame {
    pub primary_drag: PointerDelta,
    pub secondary_drag: PointerDelta,
    pub middle_drag: PointerDelta,
    pub wheel_lines: f32,
    pub pointer_position: Option<PointerPosition>,
    pub modifiers: Modifiers,
}

impl InputFrame {
    pub fn reset_deltas(&mut self) {
        self.primary_drag = PointerDelta::default();
        self.secondary_drag = PointerDelta::default();
        self.middle_drag = PointerDelta::default();
        self.wheel_lines = 0.0;
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputAccumulator {
    frame: InputFrame,
    primary_pressed: bool,
    secondary_pressed: bool,
    middle_pressed: bool,
    last_position: Option<PointerPosition>,
}

impl InputAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame(&mut self) {
        self.frame.reset_deltas();
    }

    pub fn frame(&self) -> InputFrame {
        self.frame
    }

    pub fn consume_event(&mut self, event: InputEvent, ui_consumed: bool) {
        match event {
            InputEvent::ModifiersChanged(modifiers) => {
                self.frame.modifiers = modifiers;
            }
            InputEvent::PointerButton { button, pressed } => {
                if ui_consumed {
                    return;
                }
                self.set_button(button, pressed);
                if !pressed && !self.any_button_pressed() {
                    self.last_position = None;
                }
            }
            InputEvent::PointerMoved { position } => {
                self.frame.pointer_position = Some(position);
                if ui_consumed {
                    self.last_position = Some(position);
                    return;
                }
                if let Some(last) = self.last_position {
                    let delta = PointerDelta::new(position.x - last.x, position.y - last.y);
                    if self.primary_pressed {
                        self.frame.primary_drag.dx += delta.dx;
                        self.frame.primary_drag.dy += delta.dy;
                    }
                    if self.secondary_pressed {
                        self.frame.secondary_drag.dx += delta.dx;
                        self.frame.secondary_drag.dy += delta.dy;
                    }
                    if self.middle_pressed {
                        self.frame.middle_drag.dx += delta.dx;
                        self.frame.middle_drag.dy += delta.dy;
                    }
                }
                self.last_position = Some(position);
            }
            InputEvent::Wheel { lines } => {
                if !ui_consumed {
                    self.frame.wheel_lines += lines;
                }
            }
        }
    }

    fn set_button(&mut self, button: PointerButton, pressed: bool) {
        match button {
            PointerButton::Primary => self.primary_pressed = pressed,
            PointerButton::Secondary => self.secondary_pressed = pressed,
            PointerButton::Middle => self.middle_pressed = pressed,
            PointerButton::Other(_) => {}
        }
    }

    fn any_button_pressed(&self) -> bool {
        self.primary_pressed || self.secondary_pressed || self.middle_pressed
    }
}

#[cfg(feature = "winit")]
pub mod winit_adapter {
    use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

    use crate::{InputEvent, Modifiers, PointerButton, PointerPosition};

    pub fn input_event_from_winit(event: &WindowEvent) -> Option<InputEvent> {
        match event {
            WindowEvent::MouseInput { button, state, .. } => Some(InputEvent::PointerButton {
                button: pointer_button_from_winit(*button),
                pressed: *state == ElementState::Pressed,
            }),
            WindowEvent::CursorMoved { position, .. } => Some(InputEvent::PointerMoved {
                position: PointerPosition::new(position.x as f32, position.y as f32),
            }),
            WindowEvent::MouseWheel { delta, .. } => Some(InputEvent::Wheel {
                lines: wheel_lines_from_winit(delta),
            }),
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                Some(InputEvent::ModifiersChanged(Modifiers {
                    alt: state.alt_key(),
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                    logo: state.super_key(),
                }))
            }
            _ => None,
        }
    }

    fn pointer_button_from_winit(button: MouseButton) -> PointerButton {
        match button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Middle,
            MouseButton::Back => PointerButton::Other(4),
            MouseButton::Forward => PointerButton::Other(5),
            MouseButton::Other(value) => PointerButton::Other(value),
        }
    }

    fn wheel_lines_from_winit(delta: &MouseScrollDelta) -> f32 {
        match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 30.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_drag_accumulates_until_frame_reset() {
        let mut input = InputAccumulator::new();
        input.consume_event(
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed: true,
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerMoved {
                position: PointerPosition::new(10.0, 10.0),
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerMoved {
                position: PointerPosition::new(16.0, 7.0),
            },
            false,
        );
        assert_eq!(input.frame().primary_drag, PointerDelta::new(6.0, -3.0));
        input.begin_frame();
        assert_eq!(input.frame().primary_drag, PointerDelta::default());
    }

    #[test]
    fn ui_consumed_pointer_move_does_not_drag() {
        let mut input = InputAccumulator::new();
        input.consume_event(
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed: true,
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerMoved {
                position: PointerPosition::new(0.0, 0.0),
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerMoved {
                position: PointerPosition::new(20.0, 0.0),
            },
            true,
        );
        assert_eq!(input.frame().primary_drag, PointerDelta::default());
    }

    #[test]
    fn wheel_is_gated_by_ui() {
        let mut input = InputAccumulator::new();
        input.consume_event(InputEvent::Wheel { lines: 2.0 }, false);
        input.consume_event(InputEvent::Wheel { lines: 5.0 }, true);
        assert_eq!(input.frame().wheel_lines, 2.0);
    }

    #[test]
    fn secondary_and_middle_drag_are_independent() {
        let mut input = InputAccumulator::new();
        input.consume_event(
            InputEvent::PointerButton {
                button: PointerButton::Secondary,
                pressed: true,
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerButton {
                button: PointerButton::Middle,
                pressed: true,
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerMoved {
                position: PointerPosition::new(1.0, 1.0),
            },
            false,
        );
        input.consume_event(
            InputEvent::PointerMoved {
                position: PointerPosition::new(4.0, 5.0),
            },
            false,
        );
        assert_eq!(input.frame().secondary_drag, PointerDelta::new(3.0, 4.0));
        assert_eq!(input.frame().middle_drag, PointerDelta::new(3.0, 4.0));
        assert_eq!(input.frame().primary_drag, PointerDelta::default());
    }
}
