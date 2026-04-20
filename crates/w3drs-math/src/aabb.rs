use glam::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for &p in points {
            min = min.min(p);
            max = max.max(p);
        }
        Self { min, max }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_min_max() {
        let a = Aabb::new(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(a.min, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(a.max, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn center_is_midpoint() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(a.center(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn half_extents_correct() {
        let a = Aabb::new(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(a.half_extents(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn from_points_finds_bounds() {
        let pts = vec![
            Vec3::new(-2.0, 0.0, 1.0),
            Vec3::new(3.0, -1.0, 0.5),
            Vec3::new(0.0, 4.0, -1.0),
        ];
        let a = Aabb::from_points(&pts);
        assert_eq!(a.min, Vec3::new(-2.0, -1.0, -1.0));
        assert_eq!(a.max, Vec3::new(3.0, 4.0, 1.0));
    }

    #[test]
    fn from_single_point_has_zero_extents() {
        let pts = vec![Vec3::new(1.0, 2.0, 3.0)];
        let a = Aabb::from_points(&pts);
        assert_eq!(a.min, a.max);
        assert_eq!(a.half_extents(), Vec3::ZERO);
    }

    #[test]
    fn default_is_zero() {
        let a = Aabb::default();
        assert_eq!(a.min, Vec3::ZERO);
        assert_eq!(a.max, Vec3::ZERO);
    }
}
