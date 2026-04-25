//! Paramètres de lumière directionnelle + frustum ombre, partagés par le viewer web (WASM) et le client natif.

use glam::Vec3;

/// État éditable pour lumière directionnelle + paramètres d’ombre.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewerLightState {
    /// Direction (espace monde) *du point vers la lumière*, normalisée côté GPU — même convention que l’ancien `Vec3::new(-0.5, -1.0, -0.5)`.
    pub light_direction: [f32; 3],
    /// Teinte linaire de la lumière directionnelle.
    pub light_color: [f32; 3],
    /// Facteur appliqué à `light_color` dans les uniformes (≥ 0).
    pub directional_intensity: f32,
    /// Intensité ambiante (0..typiquement 0.5+).
    pub ambient_intensity: f32,
    /// Bias d’ombre côté fragment.
    pub shadow_bias: f32,
    /// Caméra ombre = regarde l’origine ; position = `-light_dir * shadow_distance`.
    pub shadow_light_distance: f32,
    /// Moitié de la largeur / hauteur de l’ortho ombre.
    pub shadow_ortho_half_extent: f32,
    /// Plans near / far ombre.
    pub shadow_z_near: f32,
    pub shadow_z_far: f32,
}

impl Default for ViewerLightState {
    /// Valeurs proches de `examples/khronos-pbr-sample` (unifié avec l’exemple de référence).
    fn default() -> Self {
        let d = Vec3::new(-0.5, -1.0, -0.5).normalize();
        Self {
            light_direction: d.to_array(),
            light_color: [1.0, 0.95, 0.9],
            directional_intensity: 1.0,
            ambient_intensity: 0.12,
            shadow_bias: 0.001,
            shadow_light_distance: 30.0,
            shadow_ortho_half_extent: 25.0,
            shadow_z_near: 0.1,
            shadow_z_far: 80.0,
        }
    }
}

impl ViewerLightState {
    /// Vecteur direction lumière normalisé (même sémantique que `light_direction`).
    pub fn normalized_light_dir(&self) -> Vec3 {
        let d = Vec3::from_array(self.light_direction);
        if d.length_squared() < 1e-20 {
            Vec3::Y
        } else {
            d.normalize()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_normalized() {
        let s = ViewerLightState::default();
        let d = s.normalized_light_dir();
        assert!((d.length() - 1.0).abs() < 1e-4);
    }
}
