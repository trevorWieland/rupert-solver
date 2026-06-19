//! BDD harness for rupert-leaderboard.

use cucumber::{World, given, then, when};
use rupert_core::{
    BudgetSnapshot, Candidate, CertMethod, Certification, HostInfo, RunOutcome, RunResult,
    SCHEMA_VERSION, Solution,
};
use rupert_leaderboard::{AggregatedView, aggregate, render};

#[derive(Debug, Default, World)]
struct LeadWorld {
    rendered: Option<String>,
    results: Vec<RunResult>,
    view: Option<AggregatedView>,
}

fn fake_run(
    shape: &str,
    solver: &str,
    seed: u64,
    evals: u64,
    clearance: f64,
    certified: bool,
    outcome: RunOutcome,
) -> RunResult {
    let cert = if certified {
        Some(Certification {
            method: CertMethod::F64Epsilon,
            clearance_lo: clearance,
            clearance_hi: clearance,
        })
    } else {
        None
    };
    RunResult {
        schema_version: SCHEMA_VERSION,
        timestamp_utc: "2026-05-04T00:00:00Z".into(),
        poly_id: *rupert_shapes::cube().id(),
        poly_name: shape.into(),
        solver_name: solver.into(),
        solver_version: "0.1.0".into(),
        seed,
        budget: BudgetSnapshot {
            max_evaluations: 1000,
            max_wall_time_ms: None,
        },
        outcome,
        eval_count: evals,
        wall_time_ms: 1,
        best_positive: None,
        best_near_miss: None,
        best_boundary: None,
        telemetry: None,
        solution: Some(Solution {
            candidate: Candidate::IDENTITY,
            clearance,
            found_at_eval: evals,
            certification: cert,
        }),
        host: HostInfo::collect(),
    }
}

#[given(regex = r"^an empty aggregated view$")]
async fn empty_view(_w: &mut LeadWorld) {}

#[when(regex = r"^I render it$")]
async fn render_step(w: &mut LeadWorld) {
    w.rendered = Some(render(&AggregatedView::default()));
}

#[then(
    regex = r"^the output mentions Headline, Highest clearance, Uncertified, and Open problems$"
)]
async fn four_sections(w: &mut LeadWorld) {
    let s = w.rendered.as_deref().expect("rendered");
    assert!(s.contains("## Headline"));
    assert!(s.contains("## Highest clearance"));
    assert!(s.contains("## Uncertified candidates"));
    assert!(s.contains("## Open best observations"));
    assert!(s.contains("## Open problems"));
}

#[given(regex = r"^three certified runs for cube with non-overlapping seeds$")]
async fn three_certified_cube(w: &mut LeadWorld) {
    w.results.push(fake_run(
        "cube",
        "rq",
        0,
        100,
        0.10,
        true,
        RunOutcome::Solved,
    ));
    w.results.push(fake_run(
        "cube",
        "rq",
        1,
        50,
        0.05,
        true,
        RunOutcome::Solved,
    ));
    w.results.push(fake_run(
        "cube",
        "rq",
        2,
        80,
        0.20,
        true,
        RunOutcome::Solved,
    ));
}

#[when(regex = r"^I aggregate them$")]
async fn aggregate_them(w: &mut LeadWorld) {
    w.view = Some(aggregate(&w.results));
}

#[then(regex = r"^the headline has one row with best evals 50$")]
async fn one_row_best_50(w: &mut LeadWorld) {
    let v = w.view.as_ref().expect("agg");
    assert_eq!(v.headline.len(), 1);
    assert_eq!(v.headline[0].best_eval_count, 50);
    assert_eq!(v.headline[0].samples, 3);
}

#[then(regex = r"^the highest_clearance row has clearance 0\.20$")]
async fn highest_clearance_check(w: &mut LeadWorld) {
    let v = w.view.as_ref().expect("agg");
    assert_eq!(v.highest_clearance.len(), 1);
    assert!((v.highest_clearance[0].best_clearance - 0.20).abs() < 1e-12);
}

#[given(regex = r"^one Solved run for cube without certification$")]
async fn solved_uncert(w: &mut LeadWorld) {
    w.results.push(fake_run(
        "cube",
        "rq",
        0,
        100,
        0.10,
        false,
        RunOutcome::Solved,
    ));
}

#[then(regex = r"^the headline is empty and uncertified has one row$")]
async fn headline_empty_uncert_one(w: &mut LeadWorld) {
    let v = w.view.as_ref().expect("agg");
    assert_eq!(v.headline.len(), 0);
    assert_eq!(v.uncertified.len(), 1);
}

#[given(regex = r"^a thousand exhausted runs for noperthedron$")]
async fn thousand_exhausted(w: &mut LeadWorld) {
    for s in 0..1000u64 {
        w.results.push(fake_run(
            "noperthedron",
            "rq",
            s,
            50_000,
            0.0,
            false,
            RunOutcome::Exhausted,
        ));
    }
}

#[then(regex = r"^noperthedron is in open problems and not in the headline$")]
async fn noperthedron_open(w: &mut LeadWorld) {
    let v = w.view.as_ref().expect("agg");
    assert!(v.open_problems.iter().any(|s| s == "noperthedron"));
    assert!(!v.headline.iter().any(|r| r.shape == "noperthedron"));
}

fn main() {
    futures::executor::block_on(LeadWorld::run("tests/bdd/features"));
}
