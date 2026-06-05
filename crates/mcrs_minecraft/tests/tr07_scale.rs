//! TR-07 scale scenario: two observational profiles that measure bridge
//! throughput, bus saturation (emitted vs consumed), and orphan-entity
//! accumulation under synthetic bot load.
//!
//! Ship-gate decision (LOCKED): the hard gate for shipping is the functional
//! end-to-end test (vanilla 26.1.2 client portal walk + AoI update). These
//! TR-07 two-profile baselines are SOFT observational data only — no perf
//! thresholds are enforced here.
//!
//! Hardware target (LOCKED): primary is the vanilla-shape profile on an
//! 8-core / 16 GiB host; the mini-game-shape profile (320 bots × 20 dims)
//! targets a 16+ core / 32 GiB host as a secondary observational profile.
//!
//! Smoke variants run for `TR07_DURATION_SECS` seconds (default 10) and are
//! always included in `cargo test`. The full 5-minute variants are marked
//! `#[ignore]` and invoked manually:
//!
//!   TR07_DURATION_SECS=300 cargo test -p mcrs_minecraft tr07 -- --ignored

#[path = "harness/mod.rs"]
mod harness;

use harness::scale_bots::{profile_duration_secs, run_profile, write_baseline_json};

// ---------------------------------------------------------------------------
// Profile A: vanilla shape (~100 bots × 2 dims)
// ---------------------------------------------------------------------------

/// Smoke run of the vanilla-shape profile (100 bots, 2 dims).
///
/// Functional invariants asserted (no perf thresholds):
/// - entity_delta <= 0: PlayerIndex teardown leaves no orphan entities.
/// - saturation_gap is recorded but NOT gated.
#[test]
fn tr07_profile_a_vanilla() {
    let duration = profile_duration_secs();
    let report = run_profile(
        "tr07-vanilla",
        2,   // dims
        100, // bots_total
        0.1, // 10 % of bots do a cross-dim transfer halfway through
        duration,
    );

    // Functional assertion: no orphan-entity leak.
    assert!(
        report.entity_delta() <= 0,
        "tr07_profile_a_vanilla: entity_delta should be non-positive (no orphan leak), \
         got {} (start={}, end={})",
        report.entity_delta(),
        report.entity_count_start,
        report.entity_count_end,
    );

    // Record emitted/consumed gap without gating on it.
    let gap = report.saturation_gap();
    println!(
        "tr07_profile_a_vanilla: ticks={} mean_tick={}µs emitted_delta={} consumed_delta={} \
         saturation_gap={} entity_delta={}",
        report.tick_count,
        report.tick_mean_us,
        report.emitted_end - report.emitted_start,
        report.consumed_end - report.consumed_start,
        gap,
        report.entity_delta(),
    );

    // Write local baseline JSON to target/ (local, not committed).
    let date = chrono_date_string();
    let filename = format!("tr07-vanilla-{date}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tr07-baselines")
        .join(&filename);
    if let Err(e) = write_baseline_json(&report, &path) {
        println!("tr07_profile_a_vanilla: baseline write skipped ({e})");
    }
}

/// Full 5-minute run of the vanilla-shape profile. Marked `#[ignore]` so it
/// is opt-in. Run with:
///   TR07_DURATION_SECS=300 cargo test -p mcrs_minecraft tr07_profile_a_vanilla_long -- --ignored
#[test]
#[ignore]
fn tr07_profile_a_vanilla_long() {
    let duration = profile_duration_secs();
    let report = run_profile("tr07-vanilla-long", 2, 100, 0.1, duration);

    assert!(
        report.entity_delta() <= 0,
        "entity_delta should be non-positive, got {}",
        report.entity_delta()
    );

    let date = chrono_date_string();
    let filename = format!("tr07-vanilla-long-{date}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tr07-baselines")
        .join(&filename);
    if let Err(e) = write_baseline_json(&report, &path) {
        println!("baseline write skipped ({e})");
    }
}

// ---------------------------------------------------------------------------
// Profile B: mini-game shape (~320 bots × 20 dims)
// ---------------------------------------------------------------------------

/// Smoke run of the mini-game-shape profile (320 bots, 20 dims).
///
/// Targets a 16+ core / 32 GiB host as a secondary observational profile.
/// Same functional invariants as Profile A; no perf thresholds.
#[test]
fn tr07_profile_b_minigame() {
    let duration = profile_duration_secs();
    let report = run_profile(
        "tr07-minigame",
        20,  // dims
        320, // bots_total
        0.1, // 10 % cross-dim transfers
        duration,
    );

    assert!(
        report.entity_delta() <= 0,
        "tr07_profile_b_minigame: entity_delta should be non-positive (no orphan leak), \
         got {} (start={}, end={})",
        report.entity_delta(),
        report.entity_count_start,
        report.entity_count_end,
    );

    let gap = report.saturation_gap();
    println!(
        "tr07_profile_b_minigame: ticks={} mean_tick={}µs emitted_delta={} consumed_delta={} \
         saturation_gap={} entity_delta={}",
        report.tick_count,
        report.tick_mean_us,
        report.emitted_end - report.emitted_start,
        report.consumed_end - report.consumed_start,
        gap,
        report.entity_delta(),
    );

    let date = chrono_date_string();
    let filename = format!("tr07-minigame-{date}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tr07-baselines")
        .join(&filename);
    if let Err(e) = write_baseline_json(&report, &path) {
        println!("tr07_profile_b_minigame: baseline write skipped ({e})");
    }
}

/// Full 5-minute run of the mini-game-shape profile. Marked `#[ignore]`.
#[test]
#[ignore]
fn tr07_profile_b_minigame_long() {
    let duration = profile_duration_secs();
    let report = run_profile("tr07-minigame-long", 20, 320, 0.1, duration);

    assert!(
        report.entity_delta() <= 0,
        "entity_delta should be non-positive, got {}",
        report.entity_delta()
    );

    let date = chrono_date_string();
    let filename = format!("tr07-minigame-long-{date}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tr07-baselines")
        .join(&filename);
    if let Err(e) = write_baseline_json(&report, &path) {
        println!("baseline write skipped ({e})");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn chrono_date_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple YYYYMMDD approximation from Unix timestamp.
    let days = secs / 86400;
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{year:04}{month:02}{day:02}")
}
