//! Ported from Tom 7 (suckerpinch), `imperts.cc` from his SIGBOVIK 2025
//! Rupert project: <http://tom7.org/ruperts/>.
//!
//! Original source: <https://sourceforge.net/p/tom7misc/svn/HEAD/tree/trunk/ruperts/imperts.cc>
//! License: not declared upstream — algorithm reimplemented per the
//! description in the SIGBOVIK 2025 paper and the source's structure.
//!
//! ## What `imperts` is
//!
//! It is a **refinement** solver — given an existing positive-clearance
//! seed solution, it searches a tight box of `(Δq_outer, Δq_inner, Δt)`
//! deltas around the seed and accepts strictly-improving steps. The
//! upstream binary pulls seeds from a SQLite database produced by
//! `ruperts.cc` (the discovery engine). Our [`Solver`] trait doesn't
//! take a seed, so we self-bootstrap: a random-quaternion search until a
//! first positive-clearance candidate, then the refinement loop.
//!
//! ## v1 ↔ upstream diffs
//!
//! - **Inner DFO.** Tom's source uses `Opt::Minimize` from his cc-lib —
//!   a black-box derivative-free minimizer with `iters=2000, depth=2,
//!   attempts=100`. We don't have that crate. v1 substitutes a simpler
//!   *random search inside the box* (uniform delta sampling). Because
//!   uniform sampling at Tom's wide bounds (`Q=1.0`, `T=0.5`) doesn't
//!   actually concentrate near the seed, we add **adaptive box
//!   shrinking** on flat outer iterations — explicitly NOT in the
//!   upstream source (whose smart DFO doesn't need it). This is the
//!   v1 deviation that earns this solver its keep without cc-lib.
//!   v2 work item: port cc-lib's `Opt::Minimize`, drop the adaptive
//!   shrink, restore Tom's static `Q=1.0, T=0.5`.
//! - **No SQLite seed selection.** Tom's binary picks seeds from a
//!   global solution database (preferring under-improved shapes); we
//!   generate one fresh per `solve()` call via random unit-quaternion
//!   bootstrap.
//! - **No threading.** Upstream uses 8-thread `ParallelFan`; we honor
//!   the harness's per-task single-threaded contract (parallelism lives
//!   in `rupert-bench::sweep` over (shape, solver, seed) triples).
//!
//! ## Constants ported verbatim from `imperts.cc`
//!
//! - `MAX_OUTER_ITERS = 3000` — outer iteration cap
//! - `MIN_FLAT_ITERS = 100` — early-stop after 100 flat iters once any
//!   improvement is found
//!
//! ## Constants we adapted (see v1 deviation above)
//!
//! - `Q_BOX_INIT = 0.3` — initial half-width for quaternion deltas
//!   (upstream: `Q = 1.0` static; we shrink)
//! - `T_BOX_INIT = 0.2` — initial half-width for translation deltas
//!   (upstream: `T = 0.5` static; we shrink)
//! - `BOX_SHRINK = 0.7` — geometric decay on flat outer iterations
//! - `BOX_MIN = 1e-6` — terminate when box shrinks below this
//! - `INNER_EVALS = 500` — DFO evals per outer iter (upstream: 2000;
//!   ours converges faster per outer because shrinking adapts the box)

use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use rupert_core::{
    Budget, Candidate, EvalCounter, Polyhedron, Quat, Solution, Solver, SolverOutcome,
};

use crate::sample::random_unit_quat;

#[derive(Debug, Default)]
pub struct Imperts;

const MAX_OUTER_ITERS: usize = 3000;
const MIN_FLAT_ITERS: usize = 100;
const Q_BOX_INIT: f64 = 0.3;
const T_BOX_INIT: f64 = 0.2;
const BOX_SHRINK: f64 = 0.7;
const BOX_MIN: f64 = 1.0e-6;
const INNER_EVALS: usize = 500;

/// Maximum random-quaternion attempts to find a starting seed before
/// giving up. After this we either have a positive-clearance candidate
/// or we declare the shape too hard for imperts to bootstrap on.
const MAX_SEED_ATTEMPTS: u64 = 50_000;

impl Solver for Imperts {
    fn name(&self) -> &'static str {
        "imperts"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn solve(
        &mut self,
        _poly: &Polyhedron,
        budget: &Budget,
        ec: &mut EvalCounter<'_>,
    ) -> SolverOutcome {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(budget.seed);
        let max = budget.max_evaluations.get();

        // Phase 1 — bootstrap a seed via random unit-quaternion search.
        let Some((mut best_candidate, mut best_clearance)) = bootstrap_seed(ec, &mut rng, max)
        else {
            return SolverOutcome::Exhausted;
        };

        // Phase 2 — Tom 7's outer refinement loop. Each outer iter runs
        // the inner box-DFO over deltas anchored at the current best.
        // The v1 deviation (adaptive box shrinking) keeps the random
        // search concentrated as the seed converges.
        let mut last_improved: usize = 0;
        let mut q_box = Q_BOX_INIT;
        let mut t_box = T_BOX_INIT;
        for outer in 0..MAX_OUTER_ITERS {
            if ec.count() >= max {
                break;
            }
            let trial = inner_refine(ec, &best_candidate, q_box, t_box, &mut rng, max);
            let mut improved = false;
            if let Some((c, cand)) = trial {
                if c > best_clearance {
                    best_candidate = cand;
                    best_clearance = c;
                    last_improved = outer;
                    improved = true;
                }
            }
            if !improved {
                q_box *= BOX_SHRINK;
                t_box *= BOX_SHRINK;
                if q_box < BOX_MIN {
                    break;
                }
            }
            if outer > last_improved + MIN_FLAT_ITERS {
                break;
            }
        }

        if best_clearance.is_finite() && best_clearance > 0.0 {
            SolverOutcome::Found(Solution {
                candidate: best_candidate,
                clearance: best_clearance,
                found_at_eval: ec.count(),
                certification: None,
            })
        } else {
            SolverOutcome::Exhausted
        }
    }
}

fn bootstrap_seed<R: Rng + ?Sized>(
    ec: &mut EvalCounter<'_>,
    rng: &mut R,
    max_evals: u64,
) -> Option<(Candidate, f64)> {
    let bootstrap_cap = ec.count().saturating_add(MAX_SEED_ATTEMPTS).min(max_evals);
    while ec.count() < bootstrap_cap {
        let candidate = Candidate {
            outer: random_unit_quat(rng),
            inner: random_unit_quat(rng),
            translation: [0.0, 0.0],
        };
        let c = ec.evaluate(&candidate);
        if c.is_finite() && c > 0.0 {
            return Some((candidate, c));
        }
    }
    None
}

/// Inner DFO: uniform-random-delta sampling in a box around the current
/// seed. Returns `Some((clearance, candidate))` if any sample strictly
/// improves on the seed.
fn inner_refine<R: Rng + ?Sized>(
    ec: &mut EvalCounter<'_>,
    seed: &Candidate,
    q_box: f64,
    t_box: f64,
    rng: &mut R,
    max_evals: u64,
) -> Option<(f64, Candidate)> {
    let mut best: Option<(f64, Candidate)> = None;
    for _ in 0..INNER_EVALS {
        if ec.count() >= max_evals {
            return best;
        }
        let candidate = sample_delta(seed, q_box, t_box, rng);
        let c = ec.evaluate(&candidate);
        if !c.is_finite() {
            continue;
        }
        match best.as_ref() {
            None => best = Some((c, candidate)),
            Some((bc, _)) if c > *bc => best = Some((c, candidate)),
            _ => {}
        }
    }
    best
}

fn sample_delta<R: Rng + ?Sized>(
    seed: &Candidate,
    q_box: f64,
    t_box: f64,
    rng: &mut R,
) -> Candidate {
    let dq_o = quat_delta(rng, q_box);
    let dq_i = quat_delta(rng, q_box);
    let outer = (Quat::new(
        seed.outer.w + dq_o.0,
        seed.outer.x + dq_o.1,
        seed.outer.y + dq_o.2,
        seed.outer.z + dq_o.3,
    ))
    .normalized();
    let inner = (Quat::new(
        seed.inner.w + dq_i.0,
        seed.inner.x + dq_i.1,
        seed.inner.y + dq_i.2,
        seed.inner.z + dq_i.3,
    ))
    .normalized();
    let outer = if outer.norm_sq() < 1e-30 {
        Quat::IDENTITY
    } else {
        outer
    };
    let inner = if inner.norm_sq() < 1e-30 {
        Quat::IDENTITY
    } else {
        inner
    };
    let dx = rng.gen_range(-t_box..t_box);
    let dy = rng.gen_range(-t_box..t_box);
    Candidate {
        outer,
        inner,
        translation: [seed.translation[0] + dx, seed.translation[1] + dy],
    }
}

fn quat_delta<R: Rng + ?Sized>(rng: &mut R, q_box: f64) -> (f64, f64, f64, f64) {
    (
        rng.gen_range(-q_box..q_box),
        rng.gen_range(-q_box..q_box),
        rng.gen_range(-q_box..q_box),
        rng.gen_range(-q_box..q_box),
    )
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn budget(max_evals: u64, seed: u64) -> Budget {
        Budget {
            max_evaluations: NonZeroU64::new(max_evals).expect("nonzero"),
            max_wall_time: None,
            seed,
        }
    }

    #[test]
    fn solves_cube() {
        let mut solver = Imperts;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(150_000, 0), &mut ec);
        assert!(
            matches!(outcome, SolverOutcome::Found(_)),
            "got {outcome:?}"
        );
    }

    #[test]
    fn refines_above_random_quat_clearance() {
        // Imperts should find a passage with a *larger* clearance margin
        // than a single random_quat draw — that's the whole point of the
        // refiner. Run on cube; assert clearance > 0.05 (random_quat
        // typical cube clearance is ~0.02).
        let mut solver = Imperts;
        let p = rupert_shapes::cube();
        let mut ec = EvalCounter::new(&p);
        let outcome = solver.solve(&p, &budget(200_000, 17), &mut ec);
        match outcome {
            SolverOutcome::Found(sol) => {
                assert!(
                    sol.clearance > 0.05,
                    "imperts cube clearance {} ≤ refinement target",
                    sol.clearance
                );
            }
            _ => unreachable!("imperts should find cube passage"),
        }
    }

    #[test]
    fn deterministic_for_same_seed() {
        let p = rupert_shapes::cube();
        let mut a = Imperts;
        let mut b = Imperts;
        let mut ec_a = EvalCounter::new(&p);
        let mut ec_b = EvalCounter::new(&p);
        let _ = a.solve(&p, &budget(50_000, 9), &mut ec_a);
        let _ = b.solve(&p, &budget(50_000, 9), &mut ec_b);
        assert_eq!(ec_a.count(), ec_b.count());
    }
}
