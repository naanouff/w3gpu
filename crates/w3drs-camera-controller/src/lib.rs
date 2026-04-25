//! Reusable camera controllers for native viewers.

use glam::{Mat4, Quat, Vec3};
use w3drs_ecs::components::TransformComponent;
use w3drs_input::{InputFrame, PointerDelta};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitConfig {
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Default for OrbitConfig {
    fn default() -> Self {
        Self {
            orbit_sensitivity: 0.005,
            pan_sensitivity: 0.0015,
            zoom_sensitivity: 0.75,
            min_pitch: -1.5,
            max_pitch: 1.5,
            min_distance: 0.35,
            max_distance: 120.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub eye: Vec3,
    pub target: Vec3,
    pub rotation: Quat,
}

impl CameraPose {
    pub fn write_transform(self, transform: &mut TransformComponent) {
        transform.position = self.eye;
        transform.rotation = self.rotation;
        transform.update_local_matrix();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitController {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub config: OrbitConfig,
}

impl OrbitController {
    pub fn new(distance: f32, pitch: f32, yaw: f32, target: Vec3) -> Self {
        Self {
            yaw,
            pitch,
            distance,
            target,
            config: OrbitConfig::default(),
        }
        .clamped()
    }

    pub fn with_config(mut self, config: OrbitConfig) -> Self {
        self.config = config;
        self.clamped()
    }

    pub fn eye(&self) -> Vec3 {
        let y = self.distance * self.pitch.sin();
        let xz = self.distance * self.pitch.cos();
        self.target + Vec3::new(xz * self.yaw.sin(), y, xz * self.yaw.cos())
    }

    pub fn pose(&self) -> CameraPose {
        let eye = self.eye();
        let (_, rotation, _) = Mat4::look_at_rh(eye, self.target, Vec3::Y)
            .inverse()
            .to_scale_rotation_translation();
        CameraPose {
            eye,
            target: self.target,
            rotation,
        }
    }

    pub fn apply_input(&mut self, input: &InputFrame) {
        if !input.primary_drag.is_zero() {
            self.orbit(input.primary_drag);
        }
        if !input.secondary_drag.is_zero() || !input.middle_drag.is_zero() {
            let delta = if !input.secondary_drag.is_zero() {
                input.secondary_drag
            } else {
                input.middle_drag
            };
            self.pan(delta);
        }
        if input.wheel_lines != 0.0 {
            self.zoom(input.wheel_lines * self.config.zoom_sensitivity);
        }
    }

    pub fn orbit(&mut self, delta: PointerDelta) {
        self.yaw -= delta.dx * self.config.orbit_sensitivity;
        self.pitch = (self.pitch + delta.dy * self.config.orbit_sensitivity)
            .clamp(self.config.min_pitch, self.config.max_pitch);
    }

    pub fn pan(&mut self, delta: PointerDelta) {
        let eye = self.eye();
        let forward = (self.target - eye).try_normalize().unwrap_or(-Vec3::Z);
        let right = forward.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        let up = right.cross(forward).try_normalize().unwrap_or(Vec3::Y);
        let scale = self.distance * self.config.pan_sensitivity;
        self.target += (-right * delta.dx + up * delta.dy) * scale;
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance =
            (self.distance - delta).clamp(self.config.min_distance, self.config.max_distance);
    }

    pub fn reframe(&mut self, center: Vec3, radius: f32, fov_y_deg: f32, aspect: f32) {
        self.target = center;
        self.distance = fit_distance_for_radius(radius, fov_y_deg, aspect)
            .clamp(self.config.min_distance, self.config.max_distance);
    }

    fn clamped(mut self) -> Self {
        self.pitch = self
            .pitch
            .clamp(self.config.min_pitch, self.config.max_pitch);
        self.distance = self
            .distance
            .clamp(self.config.min_distance, self.config.max_distance);
        self
    }
}

pub fn fit_distance_for_radius(radius: f32, fov_y_deg: f32, aspect: f32) -> f32 {
    let half_v = (fov_y_deg.to_radians() * 0.5).clamp(0.05, 1.4);
    let half_h = (half_v.tan() * aspect.max(0.1)).atan();
    let fit_half = half_v.min(half_h).max(0.05);
    (radius.max(0.001) / fit_half.tan()) * 1.05
}

pub fn orbit_from_center_radius(
    center: Vec3,
    radius: f32,
    fov_y_deg: f32,
    aspect: f32,
) -> OrbitController {
    let mut orbit = OrbitController::new(6.0, 0.22, 0.0, center);
    orbit.reframe(center, radius, fov_y_deg, aspect);
    orbit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_drag_updates_yaw_and_pitch() {
        let mut orbit = OrbitController::new(4.0, 0.0, 0.0, Vec3::ZERO);
        orbit.orbit(PointerDelta::new(10.0, -5.0));
        assert!((orbit.yaw + 0.05).abs() < 1e-6);
        assert!((orbit.pitch + 0.025).abs() < 1e-6);
    }

    #[test]
    fn orbit_pitch_is_clamped() {
        let mut orbit = OrbitController::new(4.0, 0.0, 0.0, Vec3::ZERO);
        orbit.orbit(PointerDelta::new(0.0, 100_000.0));
        assert_eq!(orbit.pitch, orbit.config.max_pitch);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut orbit = OrbitController::new(4.0, 0.0, 0.0, Vec3::ZERO);
        orbit.zoom(1000.0);
        assert_eq!(orbit.distance, orbit.config.min_distance);
        orbit.zoom(-1000.0);
        assert_eq!(orbit.distance, orbit.config.max_distance);
    }

    #[test]
    fn pan_moves_target() {
        let mut orbit = OrbitController::new(10.0, 0.0, 0.0, Vec3::ZERO);
        orbit.pan(PointerDelta::new(10.0, 0.0));
        assert!(orbit.target.x < 0.0);
        assert!(orbit.target.y.abs() < 1e-6);
    }

    #[test]
    fn fit_distance_increases_with_radius() {
        let small = fit_distance_for_radius(1.0, 60.0, 16.0 / 9.0);
        let large = fit_distance_for_radius(4.0, 60.0, 16.0 / 9.0);
        assert!(large > small);
    }

    #[test]
    fn reframe_sets_target_and_distance() {
        let mut orbit = OrbitController::new(1.0, 0.0, 0.0, Vec3::ZERO);
        orbit.reframe(Vec3::new(1.0, 2.0, 3.0), 2.0, 60.0, 1.0);
        assert_eq!(orbit.target, Vec3::new(1.0, 2.0, 3.0));
        assert!(orbit.distance > 2.0);
    }

    #[test]
    fn pose_writes_transform_position() {
        let orbit = OrbitController::new(2.0, 0.0, 0.0, Vec3::ZERO);
        let pose = orbit.pose();
        let mut transform = TransformComponent::default();
        pose.write_transform(&mut transform);
        assert!((transform.position.z - 2.0).abs() < 1e-6);
    }
}
