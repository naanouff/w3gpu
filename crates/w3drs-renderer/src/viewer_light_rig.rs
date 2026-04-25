//! Construction des [`LightUniforms`](crate::LightUniforms) et des champs lumière des [`FrameUniforms`](crate::FrameUniforms) à partir d’un [`ViewerLightState`].
//!
//! Utilisé par le viewer WASM et peut être partagé avec un binaire natif (ex. `pbr-viewer`).

use glam::{Mat4, Vec3};
use w3drs_assets::ViewerLightState;
use w3drs_ecs::{components::CameraComponent, components::TransformComponent, World};

use crate::{FrameUniforms, LightUniforms};

/// Matrice ombre (vue × ortho) + bias — aligné sur l’exemple `khronos-pbr-sample` historique, paramétrable.
pub fn light_uniforms_from_viewer(s: &ViewerLightState) -> LightUniforms {
    let d = s.normalized_light_dir();
    let light_pos = -d * s.shadow_light_distance;
    let light_view = Mat4::look_at_rh(light_pos, Vec3::ZERO, Vec3::Y);
    let h = s.shadow_ortho_half_extent;
    let light_proj = Mat4::orthographic_rh(-h, h, -h, h, s.shadow_z_near, s.shadow_z_far);
    LightUniforms {
        view_proj: (light_proj * light_view).to_cols_array_2d(),
        shadow_bias: s.shadow_bias,
        _pad: [0.0; 3],
    }
}

/// Remplit les champs d’une trame lumière / ombre (caméra, temps, IBL) pour une frame.
pub fn build_frame_uniforms_for_viewer(
    view: Mat4,
    projection: Mat4,
    cam_pos: Vec3,
    total_time: f32,
    ibl_diffuse_scale: f32,
    viewer: &ViewerLightState,
) -> FrameUniforms {
    let inv_vp = (projection * view).inverse();
    let l = light_uniforms_from_viewer(viewer);
    let lc = Vec3::from_array(viewer.light_color) * viewer.directional_intensity;
    let dir = viewer.normalized_light_dir();
    FrameUniforms {
        projection: projection.to_cols_array_2d(),
        view: view.to_cols_array_2d(),
        inv_view_projection: inv_vp.to_cols_array_2d(),
        camera_position: cam_pos.to_array(),
        _pad0: 0.0,
        light_direction: dir.to_array(),
        _pad1: 0.0,
        light_color: lc.to_array(),
        ambient_intensity: viewer.ambient_intensity,
        total_time,
        _pad2: [0.0; 3],
        light_view_proj: l.view_proj,
        shadow_bias: l.shadow_bias,
        ibl_flags: 0,
        ibl_diffuse_scale,
        _pad3: 0.0,
    }
}

/// Extrait (view, projection, pos caméra) de la **première** caméra active.
pub fn active_camera_vpc(world: &World) -> (Mat4, Mat4, Vec3) {
    world
        .query_entities::<CameraComponent>()
        .into_iter()
        .find_map(|e| {
            let cam = world.get_component::<CameraComponent>(e)?;
            if !cam.is_active {
                return None;
            }
            let pos = world
                .get_component::<TransformComponent>(e)
                .map(|t| {
                    let w = t.world_matrix.w_axis;
                    Vec3::new(w.x, w.y, w.z)
                })
                .unwrap_or(Vec3::ZERO);
            Some((cam.view_matrix, cam.projection_matrix, pos))
        })
        .unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO))
}

/// Helper tout-en-un (Khronos, viewers natifs).
pub fn build_frame_uniforms_for_world(
    world: &World,
    total_time: f32,
    ibl_diffuse_scale: f32,
    viewer: &ViewerLightState,
) -> FrameUniforms {
    let (view, projection, cam_pos) = active_camera_vpc(world);
    build_frame_uniforms_for_viewer(
        view,
        projection,
        cam_pos,
        total_time,
        ibl_diffuse_scale,
        viewer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_sane_light_size() {
        let s = ViewerLightState::default();
        let u = light_uniforms_from_viewer(&s);
        // view_proj non nulle
        assert_ne!(u.view_proj[0][0], 0.0);
    }
}
