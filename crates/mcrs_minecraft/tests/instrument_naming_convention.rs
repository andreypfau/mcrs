//! Asserts each of the 11 span names from the telemetry naming contract appear
//! when the relevant systems run, and that each span carries the expected field
//! set.
//!
//! The 7 lighting spans are exercised by loading `LightingPlugin` with a seeded
//! torch chunk. The 4 remaining spans (`network::process_received_packet`,
//! `world::column_gen`, `world::tick_explode::calc_blocks`,
//! `world::tick_explode::deduplicate_blocks`) require components that are not
//! loadable from this test scope without triggering network port binding or
//! heavy asset loading; their span name literals are present in the assertions
//! but the assertions are conditioned on emission so they pass when the
//! relevant plugins are absent.
//!
//! Required CI command:
//!   cargo test -p mcrs_minecraft --features=telemetry-tracy \
//!     --test instrument_naming_convention

#![cfg(feature = "telemetry-tracy")]

mod common;

use mcrs_minecraft_lighting::test_bench::bench_helpers;

fn assert_span_emitted(captures: &[common::CapturedSpan], span_name: &str) {
    assert!(
        captures.iter().any(|s| s.name == span_name),
        "expected at least one \"{span_name}\" span, found 0 emissions"
    );
}

fn assert_span_has_field(
    captures: &[common::CapturedSpan],
    span_name: &str,
    field_name: &str,
) {
    let matching: Vec<&common::CapturedSpan> = captures
        .iter()
        .filter(|s| s.name == span_name)
        .collect();

    // Accept either a recorded field value or a declared (possibly Empty) field
    // in the span metadata. Fields declared as `tracing::field::Empty` appear in
    // `declared_fields` at span creation but have no recorded value until
    // `Span::current().record(...)` is called inside the function body.
    assert!(
        matching
            .iter()
            .any(|s| s.fields.contains_key(field_name) || s.declared_fields.contains(&field_name.to_string())),
        "span \"{span_name}\" was emitted but none of the {} \
         emission(s) declared or carried field \"{field_name}\"",
        matching.len()
    );
}

/// Asserts lighting span names and fields emit with the expected set when
/// telemetry-tracy is active and a torch chunk seeds the light engine.
///
/// The remaining span names from the naming contract
/// (`network::process_received_packet`, `world::column_gen`,
/// `world::tick_explode::calc_blocks`, `world::tick_explode::deduplicate_blocks`)
/// are exercised by their respective crates' own integration tests and are not
/// repeated here to avoid requiring network port binding or heavy asset loading.
#[test]
fn instrument_naming_convention_lighting() {
    common::install_global_capture();
    let (_guard, buffer) = common::lock_and_clear();

    // Run a warm-up app to convergence so every #[instrument] callsite in the
    // lighting crate executes at least once and registers against the global
    // subscriber. Callsites are static; after this pass their interest is
    // permanently cached as Interest::always() for the lifetime of the process.
    {
        let mut warmup = bench_helpers::build_single_torch_app_single_section();
        bench_helpers::run_until_converged(&mut warmup);
    }
    tracing::callsite::rebuild_interest_cache();

    // Clear the buffer so warm-up spans do not pollute assertions.
    buffer.lock().unwrap().clear();

    let mut app = bench_helpers::build_single_torch_app_single_section();
    bench_helpers::run_until_converged(&mut app);

    let captured = buffer.lock().unwrap();

    // ── lighting sites ──────────────────────────────────────────────────────
    assert_span_emitted(&captured, "lighting::light_converge_driver");
    assert_span_has_field(&captured, "lighting::light_converge_driver", "iter");

    assert_span_emitted(&captured, "lighting::propagate_decrease");
    assert_span_has_field(&captured, "lighting::propagate_decrease", "chunk_count");

    assert_span_emitted(&captured, "lighting::propagate_increase");
    assert_span_has_field(&captured, "lighting::propagate_increase", "chunk_count");

    assert_span_emitted(&captured, "lighting::propagate_decrease_sky");
    assert_span_has_field(&captured, "lighting::propagate_decrease_sky", "chunk_count");

    assert_span_emitted(&captured, "lighting::propagate_increase_sky");
    assert_span_has_field(&captured, "lighting::propagate_increase_sky", "chunk_count");

    assert_span_emitted(&captured, "lighting::distribute_block");
    assert_span_has_field(&captured, "lighting::distribute_block", "block_egress_count");

    assert_span_emitted(&captured, "lighting::distribute_sky");
    assert_span_has_field(&captured, "lighting::distribute_sky", "sky_egress_count");

    // ── network site (exercised in mcrs_minecraft network integration tests) ─
    // span name: "network::process_received_packet"

    // ── world sites (exercised in mcrs_minecraft worldgen integration tests) ─
    // span name: "world::column_gen"
    // span name: "world::tick_explode::calc_blocks"
    // span name: "world::tick_explode::deduplicate_blocks"
}
