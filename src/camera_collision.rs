//! Static rendered-geometry segment queries for the recovery follow camera.
//! Not original physics, navigation collision, alpha-tested visibility or a sphere sweep.
use crate::Result;
use bevy::prelude::*;
#[derive(Clone, Copy)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}
impl Bounds {
    fn hit(&self, start: Vec3, delta: Vec3) -> bool {
        let mut lo = 0.0f32;
        let mut hi = 1.0f32;
        for i in 0..3 {
            if delta[i].abs() < 1e-8 {
                if start[i] < self.min[i] || start[i] > self.max[i] {
                    return false;
                }
            } else {
                let a = (self.min[i] - start[i]) / delta[i];
                let b = (self.max[i] - start[i]) / delta[i];
                lo = lo.max(a.min(b));
                hi = hi.min(a.max(b));
                if hi < lo {
                    return false;
                }
            }
        }
        true
    }
}
struct Shape {
    triangles: Vec<[Vec3; 3]>,
    bounds: Bounds,
}
struct Instance {
    shape: usize,
    inverse: Mat4,
    bounds: Bounds,
}
#[derive(Resource, Default)]
pub struct CameraCollision {
    shapes: Vec<Shape>,
    instances: Vec<Instance>,
}
fn triangle_fraction(start: Vec3, delta: Vec3, t: [Vec3; 3]) -> Option<f32> {
    let e1 = t[1] - t[0];
    let e2 = t[2] - t[0];
    let h = delta.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = det.recip();
    let s = start - t[0];
    let u = s.dot(h) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = delta.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let fraction = e2.dot(q) * inv;
    (0.0..=1.0).contains(&fraction).then_some(fraction)
}
impl CameraCollision {
    pub fn add_shape(&mut self, triangles: Vec<[Vec3; 3]>) -> Result<usize> {
        if triangles.is_empty() || triangles.iter().flatten().any(|p| !p.is_finite()) {
            return Err("Invalid camera collision mesh".into());
        }
        let min = triangles
            .iter()
            .flatten()
            .fold(Vec3::splat(f32::INFINITY), |a, b| a.min(*b));
        let max = triangles
            .iter()
            .flatten()
            .fold(Vec3::splat(f32::NEG_INFINITY), |a, b| a.max(*b));
        let id = self.shapes.len();
        self.shapes.push(Shape {
            triangles,
            bounds: Bounds { min, max },
        });
        Ok(id)
    }
    pub fn add_instance(&mut self, shape: usize, root: Mat4) -> Result<()> {
        let local = self
            .shapes
            .get(shape)
            .ok_or("Unknown collision mesh")?
            .bounds;
        if !root.is_finite()
            || root.determinant().abs() < 1e-10
            || root.x_axis.w != 0.0
            || root.y_axis.w != 0.0
            || root.z_axis.w != 0.0
            || root.w_axis.w != 1.0
        {
            return Err("Invalid collision transform".into());
        }
        let inverse = root.inverse();
        if !inverse.is_finite() {
            return Err("Invalid inverse transform".into());
        }
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for mask in 0..8 {
            let p = Vec3::new(
                if mask & 1 == 0 {
                    local.min.x
                } else {
                    local.max.x
                },
                if mask & 2 == 0 {
                    local.min.y
                } else {
                    local.max.y
                },
                if mask & 4 == 0 {
                    local.min.z
                } else {
                    local.max.z
                },
            );
            let world = root.transform_point3(p);
            if !world.is_finite() {
                return Err("Collision bounds overflow".into());
            }
            min = min.min(world);
            max = max.max(world);
        }
        self.instances.push(Instance {
            shape,
            inverse,
            bounds: Bounds { min, max },
        });
        Ok(())
    }
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
    pub fn nearest_fraction(&self, start: Vec3, end: Vec3) -> Result<Option<f32>> {
        if !start.is_finite() || !end.is_finite() {
            return Err("Invalid camera segment".into());
        }
        let delta = end - start;
        if !delta.is_finite() {
            return Err("Camera segment overflow".into());
        }
        if delta.length_squared() < 1e-12 {
            return Ok(None);
        }
        let mut nearest = 1.0;
        let mut found = false;
        for instance in &self.instances {
            if !instance.bounds.hit(start, delta) {
                continue;
            }
            let a = instance.inverse.transform_point3(start);
            let d = instance.inverse.transform_vector3(delta);
            for triangle in &self.shapes[instance.shape].triangles {
                if let Some(t) = triangle_fraction(a, d, *triangle)
                    && t <= nearest
                {
                    nearest = t;
                    found = true;
                }
            }
        }
        Ok(found.then_some(nearest))
    }
    /// When direct retraction leaves little room, try bounded alternate views.
    /// This is replacement camera policy, not a recovered original algorithm.
    pub fn follow_position(&self, focus: Vec3, desired: Vec3, padding: f32) -> Result<Vec3> {
        let primary = self.camera_position(focus, desired, padding)?;
        let offset = desired - focus;
        let distance = offset.length();
        let minimum = 3.0f32.min(distance);
        if primary.distance(focus) >= minimum {
            return Ok(primary);
        }
        let direction = offset.normalize();
        let mut best = primary;
        let mut available = primary.distance(focus);
        for candidate in [
            (direction + Vec3::Y * 0.7).normalize(),
            Quat::from_rotation_y(std::f32::consts::PI) * direction,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * direction,
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * direction,
        ] {
            let position = self.camera_position(focus, focus + candidate * distance, padding)?;
            let room = position.distance(focus);
            if room > available {
                best = position;
                available = room;
            }
            if available >= minimum {
                break;
            }
        }
        Ok(best)
    }
    pub fn camera_position(&self, focus: Vec3, desired: Vec3, padding: f32) -> Result<Vec3> {
        if !padding.is_finite() || padding < 0.0 {
            return Err("Invalid camera padding".into());
        }
        let hit = self.nearest_fraction(focus, desired)?;
        let delta = desired - focus;
        let length = delta.length();
        if !length.is_finite() {
            return Err("Camera distance overflow".into());
        }
        Ok(if let Some(fraction) = hit {
            focus + delta * (fraction - padding / length).max(0.0)
        } else {
            desired
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wall() -> CameraCollision {
        let mut c = CameraCollision::default();
        let s = c
            .add_shape(vec![[
                Vec3::new(-5.0, -5.0, 4.0),
                Vec3::new(5.0, -5.0, 4.0),
                Vec3::new(0.0, 5.0, 4.0),
            ]])
            .unwrap();
        c.add_instance(s, Mat4::IDENTITY).unwrap();
        c
    }
    #[test]
    fn camera_retracts_before_wall() {
        let c = wall();
        let p = c.camera_position(Vec3::ZERO, Vec3::Z * 10.0, 0.3).unwrap();
        assert!((p.z - 3.7).abs() < 0.0001);
        assert_eq!(c.nearest_fraction(Vec3::ZERO, p).unwrap(), None);
    }
    #[test]
    fn close_wall_uses_clear_alternate_view() {
        let mut c = CameraCollision::default();
        let shape = c
            .add_shape(vec![[
                Vec3::new(-5.0, -5.0, 0.1),
                Vec3::new(5.0, -5.0, 0.1),
                Vec3::new(0.0, 5.0, 0.1),
            ]])
            .unwrap();
        c.add_instance(shape, Mat4::IDENTITY).unwrap();
        let p = c.follow_position(Vec3::ZERO, Vec3::Z * 10.0, 0.35).unwrap();
        assert!(p.length() >= 3.0);
        assert_eq!(c.nearest_fraction(Vec3::ZERO, p).unwrap(), None);
        assert_eq!(
            c.follow_position(Vec3::ZERO, Vec3::ZERO, 0.35).unwrap(),
            Vec3::ZERO
        );
    }
    #[test]
    fn nearest_surface_and_endpoint() {
        let mut c = wall();
        let shape = c
            .add_shape(vec![[
                Vec3::new(-5.0, -5.0, 0.0),
                Vec3::new(5.0, -5.0, 0.0),
                Vec3::new(0.0, 5.0, 0.0),
            ]])
            .unwrap();
        c.add_instance(shape, Mat4::from_translation(Vec3::Z * 2.0))
            .unwrap();
        assert!(
            (c.nearest_fraction(Vec3::ZERO, Vec3::Z * 10.0)
                .unwrap()
                .unwrap()
                - 0.2)
                .abs()
                < 1e-6
        );
        assert_eq!(
            c.nearest_fraction(Vec3::ZERO, Vec3::Z * 2.0).unwrap(),
            Some(1.0)
        );
        assert!(c.camera_position(Vec3::ZERO, Vec3::Z * 10.0, -1.0).is_err());
        assert_eq!(
            c.camera_position(Vec3::ZERO, Vec3::ZERO, 0.35).unwrap(),
            Vec3::ZERO
        );
    }
    #[test]
    fn rotated_nonuniform_instance_and_degenerate_triangle() {
        let mut c = CameraCollision::default();
        let shape = c
            .add_shape(vec![
                [
                    Vec3::new(-1.0, -1.0, 0.0),
                    Vec3::new(1.0, -1.0, 0.0),
                    Vec3::Y,
                ],
                [Vec3::ZERO; 3],
            ])
            .unwrap();
        c.add_instance(
            shape,
            Mat4::from_scale_rotation_translation(
                Vec3::new(2.0, 3.0, 1.0),
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                Vec3::X * 6.0,
            ),
        )
        .unwrap();
        assert!(
            (c.nearest_fraction(Vec3::ZERO, Vec3::X * 10.0)
                .unwrap()
                .unwrap()
                - 0.6)
                .abs()
                < 1e-5
        );
        assert!(c.add_shape(vec![]).is_err());
        assert!(c.add_shape(vec![[Vec3::splat(f32::NAN); 3]]).is_err());
    }
    #[test]
    fn triangle_not_only_bounds() {
        let c = wall();
        assert_eq!(
            c.nearest_fraction(Vec3::new(4.0, 4.0, 0.0), Vec3::new(4.0, 4.0, 10.0))
                .unwrap(),
            None
        );
        assert!(
            (c.nearest_fraction(Vec3::Z * 10.0, Vec3::ZERO)
                .unwrap()
                .unwrap()
                - 0.6)
                .abs()
                < 1e-6
        );
    }
    #[test]
    fn transformed_mesh_and_invalid_input() {
        let mut c = CameraCollision::default();
        let s = c
            .add_shape(vec![[
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ]])
            .unwrap();
        c.add_instance(
            s,
            Mat4::from_scale_rotation_translation(Vec3::splat(2.0), Quat::IDENTITY, Vec3::Z * 6.0),
        )
        .unwrap();
        assert!(
            (c.nearest_fraction(Vec3::ZERO, Vec3::Z * 10.0)
                .unwrap()
                .unwrap()
                - 0.6)
                .abs()
                < 1e-6
        );
        assert!(c.add_instance(s, Mat4::ZERO).is_err());
        assert!(
            c.nearest_fraction(Vec3::splat(f32::NAN), Vec3::ZERO)
                .is_err()
        );
        assert_eq!(c.nearest_fraction(Vec3::ZERO, Vec3::ZERO).unwrap(), None);
    }
}
