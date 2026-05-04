//! Shared sampling utilities — Shoemake's uniform unit-quaternion sampler.

use rand::Rng;
use rupert_core::Quat;

/// Uniformly random unit quaternion via Shoemake's method.
///
/// Reference: K. Shoemake, "Uniform Random Rotations", Graphics Gems III.
pub fn random_unit_quat<R: Rng + ?Sized>(rng: &mut R) -> Quat {
    let u1: f64 = rng.gen_range(0.0..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    let u3: f64 = rng.gen_range(0.0..1.0);
    let s1 = (1.0 - u1).sqrt();
    let s2 = u1.sqrt();
    let two_pi = 2.0 * std::f64::consts::PI;
    Quat::new(
        s2 * (two_pi * u3).cos(),
        s1 * (two_pi * u2).sin(),
        s1 * (two_pi * u2).cos(),
        s2 * (two_pi * u3).sin(),
    )
}

/// Random translation in `[-r, r]^2`.
pub fn random_translation<R: Rng + ?Sized>(rng: &mut R, r: f64) -> [f64; 2] {
    [rng.gen_range(-r..r), rng.gen_range(-r..r)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_xoshiro::Xoshiro256PlusPlus;
    use rand_xoshiro::rand_core::SeedableRng;

    #[test]
    fn random_quats_are_unit_norm() {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
        for _ in 0..200 {
            let q = random_unit_quat(&mut rng);
            assert!((q.norm() - 1.0).abs() < 1e-12, "non-unit: {}", q.norm());
        }
    }

    #[test]
    fn random_quats_distinct_across_seeds() {
        let mut rng_a = Xoshiro256PlusPlus::seed_from_u64(1);
        let mut rng_b = Xoshiro256PlusPlus::seed_from_u64(2);
        let qa = random_unit_quat(&mut rng_a);
        let qb = random_unit_quat(&mut rng_b);
        // Distinct seeds should yield distinct quats with overwhelming probability.
        let same = (qa.w - qb.w).abs() < 1e-12
            && (qa.x - qb.x).abs() < 1e-12
            && (qa.y - qb.y).abs() < 1e-12
            && (qa.z - qb.z).abs() < 1e-12;
        assert!(!same);
    }

    #[test]
    fn random_quats_deterministic_for_same_seed() {
        let mut rng_a = Xoshiro256PlusPlus::seed_from_u64(7);
        let mut rng_b = Xoshiro256PlusPlus::seed_from_u64(7);
        for _ in 0..10 {
            let qa = random_unit_quat(&mut rng_a);
            let qb = random_unit_quat(&mut rng_b);
            assert_eq!(qa, qb);
        }
    }
}
