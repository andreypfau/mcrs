//! Wall-clock-bounded observational baselines for the two scale profiles.
//! No perf thresholds are enforced; each run emits a JSON snapshot of per-tick
//! timing and bus saturation. Duration comes from `TR07_DURATION_SECS`.

#[path = "common/scale_bots.rs"]
mod scale_bots;

use scale_bots::{ScaleReport, profile_duration_secs, run_profile, write_baseline_json};

fn main() {
    let duration = profile_duration_secs();
    baseline(run_profile("tr07-vanilla-long", 2, 100, 0.1, duration));
    baseline(run_profile("tr07-minigame-long", 20, 320, 0.1, duration));
}

fn baseline(report: ScaleReport) {
    assert!(
        report.entity_delta() <= 0,
        "entity_delta should be non-positive, got {}",
        report.entity_delta()
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tr07-baselines")
        .join(format!("{}.json", report.profile_name));
    if let Err(e) = write_baseline_json(&report, &path) {
        println!("baseline write skipped ({e})");
    }
}
