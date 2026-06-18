//! TR-07 scale scenario.
//!
//! Two synthetic-bot-load profiles drive the `bridge_outbound` routing path
//! and the `SessionRegistry` cross-dim + teardown path. They exist in two
//! shapes:
//!
//! - Smoke variants (always run): tick-bounded and deterministic. They finish
//!   in milliseconds and assert hard functional invariants — every injected
//!   packet is routed to its bot's queue, the cross-dim reassignment path runs
//!   the expected number of times, no orphan entities leak, and teardown
//!   empties the registry.
//! - Long variants (`#[ignore]`, opt-in): wall-clock-bounded observational
//!   baselines that emit a JSON snapshot of per-tick timing and bus
//!   saturation. No perf thresholds are enforced; they are run manually:
//!
//!     TR07_DURATION_SECS=300 cargo test -p mcrs_minecraft tr07 -- --ignored
//!
//! Ship-gate decision (LOCKED): the hard gate for shipping is the functional
//! end-to-end test (vanilla 26.1.2 client portal walk + AoI update). These
//! TR-07 baselines are SOFT observational data only — no perf thresholds are
//! enforced here.

#[path = "harness/mod.rs"]
mod harness;

use harness::scale_bots::{profile_duration_secs, run_profile, run_profile_ticks, write_baseline_json};

/// Deterministic tick budget for the smoke variants. Even, so exactly
/// `SMOKE_TICKS / 2` injection ticks occur (one BlockUpdate per bot on every
/// even tick), and well past the halfway mark where the cross-dim transfer
/// fires.
const SMOKE_TICKS: u64 = 24;

/// Packets injected per bot over a `SMOKE_TICKS` run (one per even tick).
const INJECTIONS_PER_BOT: u64 = SMOKE_TICKS / 2;

// ---------------------------------------------------------------------------
// Profile A: vanilla shape (~100 bots × 2 dims)
// ---------------------------------------------------------------------------

#[test]
fn tr07_profile_a_vanilla() {
    let dims = 2;
    let bots = 100;
    let cross_dim_rate = 0.1;
    let report = run_profile_ticks("tr07-vanilla", dims, bots, cross_dim_rate, SMOKE_TICKS);

    assert_eq!(
        report.tick_count, SMOKE_TICKS,
        "smoke run should execute exactly the tick budget"
    );

    // Load actually ran: one packet per bot per injection tick.
    let expected_injected = bots as u64 * INJECTIONS_PER_BOT;
    assert_eq!(
        report.packets_injected, expected_injected,
        "expected {expected_injected} injected packets",
    );

    // Routing invariant: bridge_outbound resolved every packet's session and
    // pushed it onto the addressed bot's OutboundQueue — nothing dropped.
    assert_eq!(
        report.total_queued, report.packets_injected,
        "every injected packet should route to a bot queue (queued={}, injected={})",
        report.total_queued, report.packets_injected,
    );

    // Cross-dim reassignment path ran for exactly 10 % of bots.
    let expected_transfers = (bots as f32 * cross_dim_rate) as u64;
    assert_eq!(
        report.cross_dim_transfers, expected_transfers,
        "expected {expected_transfers} cross-dim transfers",
    );

    // No orphan-entity leak across the run.
    assert!(
        report.entity_delta() <= 0,
        "entity_delta should be non-positive (no orphan leak), got {} (start={}, end={})",
        report.entity_delta(),
        report.entity_count_start,
        report.entity_count_end,
    );

    // Teardown removed every session from the registry.
    assert_eq!(
        report.sessions_remaining_after_teardown, 0,
        "teardown should leave no sessions in the registry",
    );
}

/// Full wall-clock run of the vanilla-shape profile. Marked `#[ignore]` so it
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

#[test]
fn tr07_profile_b_minigame() {
    let dims = 20;
    let bots = 320;
    let cross_dim_rate = 0.1;
    let report = run_profile_ticks("tr07-minigame", dims, bots, cross_dim_rate, SMOKE_TICKS);

    assert_eq!(report.tick_count, SMOKE_TICKS);

    let expected_injected = bots as u64 * INJECTIONS_PER_BOT;
    assert_eq!(
        report.packets_injected, expected_injected,
        "expected {expected_injected} injected packets",
    );

    assert_eq!(
        report.total_queued, report.packets_injected,
        "every injected packet should route to a bot queue (queued={}, injected={})",
        report.total_queued, report.packets_injected,
    );

    let expected_transfers = (bots as f32 * cross_dim_rate) as u64;
    assert_eq!(
        report.cross_dim_transfers, expected_transfers,
        "expected {expected_transfers} cross-dim transfers",
    );

    assert!(
        report.entity_delta() <= 0,
        "entity_delta should be non-positive (no orphan leak), got {} (start={}, end={})",
        report.entity_delta(),
        report.entity_count_start,
        report.entity_count_end,
    );

    assert_eq!(
        report.sessions_remaining_after_teardown, 0,
        "teardown should leave no sessions in the registry",
    );
}

/// Full wall-clock run of the mini-game-shape profile. Marked `#[ignore]`.
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
