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
        let center =
            points.iter().copied().fold(Vec3::ZERO, |acc, p| acc + p) / points.len() as f32;
        let radius = points
            .iter()
            .map(|&p| p.distance(center))
            .fold(0.0_f32, f32::max);
        Self { center, radius }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_values() {
        let s = BoundingSphere::new(Vec3::new(1.0, 2.0, 3.0), 5.0);
        assert_eq!(s.center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(s.radius, 5.0);
    }

    #[test]
    fn from_empty_is_default() {
        let s = BoundingSphere::from_points(&[]);
        assert_eq!(s.center, Vec3::ZERO);
        assert_eq!(s.radius, 0.0);
    }

    #[test]
    fn from_single_point_zero_radius() {
        let s = BoundingSphere::from_points(&[Vec3::new(3.0, 0.0, 0.0)]);
        assert_eq!(s.center, Vec3::new(3.0, 0.0, 0.0));
        assert_eq!(s.radius, 0.0);
    }

    #[test]
    fn from_symmetric_points() {
        let pts = vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
        let s = BoundingSphere::from_points(&pts);
        assert!((s.center.x).abs() < 1e-5, "center should be near origin");
        assert!((s.radius - 1.0).abs() < 1e-5);
    }

    #[test]
    fn radius_is_max_distance_from_centroid() {
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
        ];
        let s = BoundingSphere::from_points(&pts);
        // centroid = (2, 0, 0), max dist = 2
        assert!((s.center.x - 2.0).abs() < 1e-5);
        assert!((s.radius - 2.0).abs() < 1e-5);
    }
}
