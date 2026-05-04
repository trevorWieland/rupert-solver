//! Unit quaternions for SO(3).
//!
//! Convention: `w + x i + y j + z k`. Multiplication is Hamilton-product order
//! `q * v` rotates `v` if `q` is a unit quaternion (via the sandwich product).

use std::ops::Mul;

use serde::{Deserialize, Serialize};

use crate::vec3::Vec3;

/// Quaternion. Should be unit-norm for rotation use; call [`Quat::normalized`]
/// after any operation that might drift.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Quat {
    pub const IDENTITY: Self = Self { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    #[inline]
    pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    /// Build a unit quaternion from an axis (need not be unit) and an angle in
    /// radians. Returns identity if the axis is the zero vector.
    pub fn from_axis_angle(axis: Vec3, angle_rad: f64) -> Self {
        let n = axis.norm();
        if n == 0.0 {
            return Self::IDENTITY;
        }
        let half = angle_rad * 0.5;
        let s = half.sin() / n;
        Self::new(half.cos(), axis.x * s, axis.y * s, axis.z * s)
    }

    #[inline]
    pub fn norm_sq(self) -> f64 {
        self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Returns a unit-norm quaternion. Zero-quat returns identity.
    pub fn normalized(self) -> Self {
        let n = self.norm();
        if n == 0.0 {
            return Self::IDENTITY;
        }
        let s = 1.0 / n;
        Self::new(self.w * s, self.x * s, self.y * s, self.z * s)
    }

    #[inline]
    pub fn conj(self) -> Self {
        Self::new(self.w, -self.x, -self.y, -self.z)
    }

    /// Rotate a 3D vector by this unit quaternion.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        // Use the optimized form: t = 2 * (qv x v); v' = v + q.w*t + qv x t.
        let qv = Vec3::new(self.x, self.y, self.z);
        let t = qv.cross(v) * 2.0;
        v + t * self.w + qv.cross(t)
    }

    /// Spherical linear interpolation. Both inputs assumed unit-norm. The
    /// shorter arc is chosen.
    pub fn slerp(self, other: Self, t: f64) -> Self {
        let mut cos_theta = self.w * other.w + self.x * other.x + self.y * other.y + self.z * other.z;
        let mut o = other;
        if cos_theta < 0.0 {
            cos_theta = -cos_theta;
            o = Self::new(-other.w, -other.x, -other.y, -other.z);
        }
        if cos_theta > 0.999_999_9 {
            // Numerical regime where slerp degenerates to lerp.
            let r = Self::new(
                self.w * (1.0 - t) + o.w * t,
                self.x * (1.0 - t) + o.x * t,
                self.y * (1.0 - t) + o.y * t,
                self.z * (1.0 - t) + o.z * t,
            );
            return r.normalized();
        }
        let theta = cos_theta.acos();
        let sin_theta = theta.sin();
        let s0 = ((1.0 - t) * theta).sin() / sin_theta;
        let s1 = (t * theta).sin() / sin_theta;
        Self::new(
            self.w * s0 + o.w * s1,
            self.x * s0 + o.x * s1,
            self.y * s0 + o.y * s1,
            self.z * s0 + o.z * s1,
        )
    }
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Quat {
    type Output = Self;

    /// Hamilton product `self * other`. The composition reads left-to-right:
    /// `(a * b).rotate(v)` is `a.rotate(b.rotate(v))`.
    fn mul(self, other: Self) -> Self {
        Self::new(
            self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
            self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn approx_eq_v(a: Vec3, b: Vec3, tol: f64) -> bool {
        (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol && (a.z - b.z).abs() < tol
    }

    #[test]
    fn identity_rotates_to_self() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!(approx_eq_v(Quat::IDENTITY.rotate(v), v, 1e-12));
    }

    #[test]
    fn ninety_around_z_sends_x_to_y() {
        let q = Quat::from_axis_angle(Vec3::Z, PI / 2.0);
        let r = q.rotate(Vec3::X);
        assert!(approx_eq_v(r, Vec3::Y, 1e-12), "{r:?}");
    }

    #[test]
    fn ninety_around_x_sends_y_to_z() {
        let q = Quat::from_axis_angle(Vec3::X, PI / 2.0);
        let r = q.rotate(Vec3::Y);
        assert!(approx_eq_v(r, Vec3::Z, 1e-12), "{r:?}");
    }

    #[test]
    fn conj_is_inverse_for_unit() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 1.0, 1.0), 1.234);
        let p = (q * q.conj()).normalized();
        let id = Quat::IDENTITY;
        assert!((p.w - id.w).abs() < 1e-12);
        assert!(p.x.abs() < 1e-12);
        assert!(p.y.abs() < 1e-12);
        assert!(p.z.abs() < 1e-12);
    }

    #[test]
    fn slerp_endpoints() {
        let q = Quat::from_axis_angle(Vec3::Z, PI / 3.0);
        let r = Quat::IDENTITY.slerp(q, 0.0);
        assert!((r.w - 1.0).abs() < 1e-12);
        let s = Quat::IDENTITY.slerp(q, 1.0);
        assert!((s.w - q.w).abs() < 1e-12);
    }

    #[test]
    fn slerp_self_is_self() {
        let q = Quat::from_axis_angle(Vec3::new(0.3, 0.4, 0.5), 0.7);
        let r = q.slerp(q, 0.5);
        assert!((r.w - q.w).abs() < 1e-9);
        assert!((r.x - q.x).abs() < 1e-9);
        assert!((r.y - q.y).abs() < 1e-9);
        assert!((r.z - q.z).abs() < 1e-9);
    }

    #[test]
    fn axis_angle_roundtrip_via_rotation() {
        // Rotate a known vector and check the axis is preserved.
        let axis = Vec3::new(0.5, 0.25, 0.75).normalized();
        let q = Quat::from_axis_angle(axis, 0.9);
        // The axis of rotation is fixed under the rotation.
        let r = q.rotate(axis);
        assert!(approx_eq_v(r, axis, 1e-12), "{r:?} vs {axis:?}");
    }

    #[test]
    fn unit_norm_after_normalize() {
        let q = Quat::new(2.0, 3.0, 4.0, 5.0).normalized();
        assert!((q.norm() - 1.0).abs() < 1e-12);
    }
}
