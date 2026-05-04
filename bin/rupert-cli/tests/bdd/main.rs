//! BDD harness for rupert-cli. Drives the built binary as a subprocess to
//! exercise the actual `rupert` entry point.

use std::path::PathBuf;
use std::process::Command;

use cucumber::{World, given, then, when};

#[derive(Debug, Default, World)]
struct CliWorld {
    last_stdout: String,
    last_stderr: String,
    last_status: i32,
    work_dir: Option<tempfile::TempDir>,
}

fn rupert_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rupert"))
}

#[given(regex = r"^the rupert CLI is built$")]
async fn rupert_built(_w: &mut CliWorld) {
    assert!(rupert_bin().exists(), "rupert binary missing");
}

#[given(regex = r"^a fresh working directory$")]
async fn fresh_working_dir(w: &mut CliWorld) {
    w.work_dir = Some(tempfile::tempdir().expect("tempdir"));
}

#[given(regex = r"^a fabricated noperthedron-passage result on disk$")]
async fn fabricate_noperthedron_lie(w: &mut CliWorld) {
    use serde_json::json;
    let dir = w.work_dir.as_ref().expect("fresh_working_dir first");
    let path = dir.path().join("results").join("test_noperthedron_lie.jsonl");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    // Identity candidate ⇒ clearance 0 (touching shadows). The solver
    // claims clearance 1.0 with a fake certification — `rupert verify`
    // must catch the lie.
    let line = json!({
        "schema_version": 1,
        "timestamp_utc": "2026-05-04T00:00:00Z",
        "poly_id": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
                    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31],
        "poly_name": "noperthedron",
        "solver_name": "fabricated_lie",
        "solver_version": "0.0.0",
        "seed": 0,
        "budget": { "max_evaluations": 1, "max_wall_time_ms": null },
        "outcome": { "kind": "solved" },
        "eval_count": 1,
        "wall_time_ms": 0,
        "solution": {
            "candidate": {
                "outer": { "w": 1.0, "x": 0.0, "y": 0.0, "z": 0.0 },
                "inner": { "w": 1.0, "x": 0.0, "y": 0.0, "z": 0.0 },
                "translation": [0.0, 0.0]
            },
            "clearance": 1.0,
            "found_at_eval": 1,
            "certification": null
        },
        "host": { "rustc": "x", "target": "x", "git_rev": "x" }
    });
    std::fs::write(&path, serde_json::to_string(&line).expect("ser") + "\n")
        .expect("write fabricated result");
}

#[when(regex = r#"^I run "rupert (.*)"$"#)]
async fn run_rupert(w: &mut CliWorld, raw_args: String) {
    let args: Vec<String> = raw_args.split_whitespace().map(String::from).collect();
    let mut cmd = Command::new(rupert_bin());
    if let Some(dir) = w.work_dir.as_ref() {
        cmd.current_dir(dir.path());
    }
    let out = cmd.args(&args).output().expect("spawn rupert");
    w.last_stdout = String::from_utf8_lossy(&out.stdout).to_string();
    w.last_stderr = String::from_utf8_lossy(&out.stderr).to_string();
    w.last_status = out.status.code().unwrap_or(-1);
}

#[then(regex = r"^the exit code is 0$")]
async fn exit_zero(w: &mut CliWorld) {
    assert_eq!(w.last_status, 0, "stderr={}", w.last_stderr);
}

#[then(regex = r"^the exit code is non-zero$")]
async fn exit_nonzero(w: &mut CliWorld) {
    assert_ne!(w.last_status, 0, "expected non-zero exit; stderr={}", w.last_stderr);
}

#[then(regex = r#"^stdout contains "(.*)"$"#)]
async fn stdout_contains(w: &mut CliWorld, needle: String) {
    assert!(
        w.last_stdout.contains(&needle),
        "stdout did not contain {needle:?}; full stdout: {}",
        w.last_stdout
    );
}

#[then(regex = r#"^stderr contains "(.*)"$"#)]
async fn stderr_contains(w: &mut CliWorld, needle: String) {
    assert!(
        w.last_stderr.contains(&needle),
        "stderr did not contain {needle:?}; full stderr: {}",
        w.last_stderr
    );
}

#[then(regex = r"^a result file exists with at least one Solved certified record$")]
async fn certified_record_exists(w: &mut CliWorld) {
    let dir = w.work_dir.as_ref().expect("work dir").path().join("results");
    let mut found = false;
    for entry in std::fs::read_dir(&dir).expect("read results") {
        let entry = entry.expect("entry");
        let bytes = std::fs::read(entry.path()).expect("read");
        for line in bytes.split(|b| *b == b'\n') {
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(line).expect("json");
            let outcome_kind = v["outcome"]["kind"].as_str().unwrap_or("");
            let cert = &v["solution"]["certification"];
            if outcome_kind == "solved" && !cert.is_null() {
                found = true;
            }
        }
    }
    assert!(found, "no Solved+certified record in {}", dir.display());
}

#[then(regex = r"^the fabricated record's outcome is now Disqualified$")]
async fn record_disqualified(w: &mut CliWorld) {
    let path = w
        .work_dir
        .as_ref()
        .expect("work dir")
        .path()
        .join("results/test_noperthedron_lie.jsonl");
    let bytes = std::fs::read(&path).expect("read fabricated");
    let line = bytes
        .split(|b| *b == b'\n')
        .find(|l| !l.is_empty())
        .expect("at least one line");
    let v: serde_json::Value = serde_json::from_slice(line).expect("json");
    assert_eq!(
        v["outcome"]["kind"].as_str().unwrap_or(""),
        "disqualified",
        "outcome was {:?}",
        v["outcome"]
    );
}

fn main() {
    futures::executor::block_on(CliWorld::run("tests/bdd/features"));
}
