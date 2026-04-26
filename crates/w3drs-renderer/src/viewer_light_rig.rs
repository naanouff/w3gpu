//! Construction des [`LightUniforms`](crate::LightUniforms) et des champs lumière des [`FrameUniforms`](crate::FrameUniforms) à partir d’un [`ViewerLightState`].
//!
//! Utilisé par le viewer WASM et peut être partagé avec un binaire natif (ex. `pbr-viewer`).

use glam::{Mat4, Vec3, Vec4};
use w3drs_assets::ViewerLightState;
use w3drs_ecs::{components::CameraComponent, components::TransformComponent, World};

use crate::{frame_uniforms::SHADOW_CASCADE_COUNT, FrameUniforms, LightUniforms};

const CSM_SPLIT_LAMBDA: f32 = 0.7;

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

fn extract_near_far_from_projection(projection: Mat4) -> (f32, f32) {
    let inv_p = projection.inverse();
    let near_h = inv_p * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let far_h = inv_p * Vec4::new(0.0, 0.0, 1.0, 1.0);
    let near = (near_h.z / near_h.w).abs().max(0.01);
    let far = (far_h.z / far_h.w).abs().max(near + 0.01);
    (near.min(far), far.max(near))
}

/// Map a positive view-space depth `d` to NDC Z in [0,1] using the perspective
/// projection formula for a right-handed depth range.
fn depth_to_ndc_z(d: f32, near: f32, far: f32) -> f32 {
    if d <= 0.0 || far <= near {
        return 0.0;
    }
    (far * (d - near) / (d * (far - near))).clamp(0.0, 1.0)
}

fn frustum_slice_corners_world(inv_vp: Mat4, ndc_z_near: f32, ndc_z_far: f32) -> [Vec3; 8] {
    let mut out = [Vec3::ZERO; 8];
    let mut i = 0;
    for &z in &[ndc_z_near, ndc_z_far] {
        for &y in &[-1.0, 1.0] {
            for &x in &[-1.0, 1.0] {
                let p = inv_vp * Vec4::new(x, y, z, 1.0);
                out[i] = (p / p.w).truncate();
                i += 1;
            }
        }
    }
    out
}

/// Build 4 cascaded light matrices and split distances in view space.
pub fn light_uniforms_for_cascades(
    view: Mat4,
    projection: Mat4,
    cam_pos: Vec3,
    s: &ViewerLightState,
) -> ([LightUniforms; SHADOW_CASCADE_COUNT], [f32; SHADOW_CASCADE_COUNT]) {
    let (cam_near, cam_far) = extract_near_far_from_projection(projection);
    let inv_vp = (projection * view).inverse();
    let light_dir = s.normalized_light_dir();
    let mut splits = [cam_far; SHADOW_CASCADE_COUNT];
    let mut uniforms = [light_uniforms_from_viewer(s); SHADOW_CASCADE_COUNT];
    let range = cam_far - cam_near;

    for (i, split) in splits.iter_mut().enumerate() {
        let p = (i + 1) as f32 / SHADOW_CASCADE_COUNT as f32;
        let log = cam_near * (cam_far / cam_near).powf(p);
        let uni = cam_near + range * p;
        *split = log * CSM_SPLIT_LAMBDA + uni * (1.0 - CSM_SPLIT_LAMBDA);
    }

    let mut prev_split = cam_near;
    for i in 0..SHADOW_CASCADE_COUNT {
        let split_far = splits[i];
        let ndc_near = depth_to_ndc_z(prev_split, cam_near, cam_far);
        let ndc_far = depth_to_ndc_z(split_far, cam_near, cam_far);
        let corners = frustum_slice_corners_world(inv_vp, ndc_near, ndc_far);

        let center = corners.iter().copied().fold(Vec3::ZERO, |acc, p| acc + p) / 8.0;
        let radius = corners
            .iter()
            .map(|&p| p.distance(center))
            .fold(0.0f32, f32::max)
            .max(0.5);
        // Stabilize projected extents by snapping to texel world units.
        let texel_size = (radius * 2.0) / 2048.0;
        let mut snapped_center = center;
        if texel_size > 1e-6 {
            snapped_center.x = (snapped_center.x / texel_size).floor() * texel_size;
            snapped_center.y = (snapped_center.y / texel_size).floor() * texel_size;
            snapped_center.z = (snapped_center.z / texel_size).floor() * texel_size;
        }

        let light_eye = snapped_center - light_dir * (s.shadow_light_distance + radius);
        let light_view = Mat4::look_at_rh(light_eye, snapped_center, Vec3::Y);

        let mut min_ls = Vec3::splat(f32::MAX);
        let mut max_ls = Vec3::splat(f32::MIN);
        for &c in &corners {
            let l = (light_view * c.extend(1.0)).truncate();
            min_ls = min_ls.min(l);
            max_ls = max_ls.max(l);
        }
        // Expand XY slightly to reduce edge swimming.
        let xy_pad = radius * 0.1 + 0.5;
        min_ls.x -= xy_pad;
        min_ls.y -= xy_pad;
        max_ls.x += xy_pad;
        max_ls.y += xy_pad;

        let near = (-max_ls.z).max(s.shadow_z_near.max(0.01));
        let far = (-min_ls.z + radius * 0.5).max(near + 1.0).min(s.shadow_z_far.max(near + 1.0));
        let light_proj = Mat4::orthographic_rh(min_ls.x, max_ls.x, min_ls.y, max_ls.y, near, far);
        uniforms[i] = LightUniforms {
            view_proj: (light_proj * light_view).to_cols_array_2d(),
            shadow_bias: s.shadow_bias,
            _pad: [0.0; 3],
        };
        prev_split = split_far;
    }

    // Keep last split as far plane distance to simplify cascade selection in shader.
    splits[SHADOW_CASCADE_COUNT - 1] = cam_far.max(splits[SHADOW_CASCADE_COUNT - 1]);
    // Ensure camera position is consumed (explicitly used by callers to keep intent clear).
    let _ = cam_pos;
    (uniforms, splits)
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
    let (cascades, splits) = light_uniforms_for_cascades(view, projection, cam_pos, viewer);
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
        light_view_proj_cascades: cascades.map(|c| c.view_proj),
        shadow_cascade_splits: splits,
        shadow_bias: viewer.shadow_bias,
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
        let (u, splits) =
            light_uniforms_for_cascades(Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO, &s);
        assert_ne!(u[0].view_proj[0][0], 0.0);
        assert!(splits[0] > 0.0);
        assert!(splits[3] >= splits[2]);
    }
}
