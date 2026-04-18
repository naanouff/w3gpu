use glam::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self::default();
        }
        let center = points.iter().copied().fold(Vec3::ZERO, |acc, p| acc + p) / points.len() as f32;
        let radius = points.iter().map(|&p| p.distance(center)).fold(0.0_f32, f32::max);
        Self { center, radius }
    }
}
