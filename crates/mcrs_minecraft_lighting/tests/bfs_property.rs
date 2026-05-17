//! Byte-equality safety net for the four `propagate_*` BFS bodies.
//!
//! Re-runs each fixture in `tests/bfs_property/snapshot.json` against the
//! current `propagate_increase`, `propagate_decrease`,
//! `propagate_increase_sky`, `propagate_decrease_sky` and asserts the
//! post-pass `BlockLight` and `SkyLight` storage is bitwise-identical to
//! the committed baseline. Any refactor of the BFS bodies that changes a
//! single nibble on any fixture fails this test.
//!
//! The snapshot baseline is captured by the (untracked) example
//! `examples/capture_bfs_snapshot.rs`. Re-running that binary against a
//! changed tree regenerates the snapshot; do that only when the change
//! is an intentional behavior shift and the baseline is being moved on
//! purpose.

#[path = "bfs_property/fixture.rs"]
mod fixture;

use mcrs_minecraft_lighting::bfs::{
    pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL,
};
use serde::Deserialize;

const SNAPSHOT_JSON: &str = include_str!("bfs_property/snapshot.json");

#[derive(Deserialize)]
struct Record {
    seed: u64,
    post_block: String,
    post_sky: String,
}

#[test]
fn snapshot_size_within_budget() {
    assert!(
        SNAPSHOT_JSON.len() <= 256 * 1024,
        "snapshot.json grew past the 256 KB budget: {} bytes",
        SNAPSHOT_JSON.len()
    );
}

#[test]
fn snapshot_record_count_meets_floor() {
    let records: Vec<Record> = serde_json::from_str(SNAPSHOT_JSON).expect("parse snapshot.json");
    assert!(
        records.len() >= 32,
        "snapshot must hold at least 32 records, got {}",
        records.len()
    );
}

#[test]
fn snapshot_replay_byte_identical_per_fixture() {
    let records: Vec<Record> = serde_json::from_str(SNAPSHOT_JSON).expect("parse snapshot.json");

    for record in &records {
        let mut f = fixture::build_fixture(record.seed);
        let (post_block, post_sky) = fixture::run_propagation_and_serialize(&mut f);

        let expected_block = fixture::b64::decode(&record.post_block);
        let expected_sky = fixture::b64::decode(&record.post_sky);

        assert_eq!(
            expected_block.len(),
            2048,
            "seed {}: baseline block payload must be 2048 bytes, got {}",
            record.seed,
            expected_block.len()
        );
        assert_eq!(
            expected_sky.len(),
            2048,
            "seed {}: baseline sky payload must be 2048 bytes, got {}",
            record.seed,
            expected_sky.len()
        );

        if post_block != expected_block.as_slice() {
            let mismatch = first_byte_mismatch(&post_block, &expected_block);
            panic!(
                "seed {}: post-pass BlockLight diverged from baseline at byte {}: actual=0x{:02X} expected=0x{:02X}",
                record.seed, mismatch.index, mismatch.actual, mismatch.expected
            );
        }
        if post_sky != expected_sky.as_slice() {
            let mismatch = first_byte_mismatch(&post_sky, &expected_sky);
            panic!(
                "seed {}: post-pass SkyLight diverged from baseline at byte {}: actual=0x{:02X} expected=0x{:02X}",
                record.seed, mismatch.index, mismatch.actual, mismatch.expected
            );
        }
    }
}

struct Mismatch {
    index: usize,
    actual: u8,
    expected: u8,
}

fn first_byte_mismatch(actual: &[u8], expected: &[u8]) -> Mismatch {
    for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
        if a != e {
            return Mismatch {
                index: i,
                actual: a,
                expected: e,
            };
        }
    }
    Mismatch {
        index: actual.len().min(expected.len()),
        actual: 0,
        expected: 0,
    }
}

use proptest::prelude::*;

const fn proptest_cases() -> u32 {
    #[cfg(feature = "long-prop-tests")]
    {
        4096
    }
    #[cfg(not(feature = "long-prop-tests"))]
    {
        256
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: proptest_cases(),
        .. ProptestConfig::default()
    })]

    #[test]
    fn bfs_is_deterministic_across_runs(seed in any::<u64>()) {
        let mut fixture_a = fixture::build_fixture(seed);
        let mut fixture_b = fixture::build_fixture(seed);
        let (a_block, a_sky) = fixture::run_propagation_and_serialize(&mut fixture_a);
        let (b_block, b_sky) = fixture::run_propagation_and_serialize(&mut fixture_b);
        prop_assert_eq!(a_block, b_block);
        prop_assert_eq!(a_sky, b_sky);
    }

    #[test]
    fn bfs_is_idempotent_at_convergence(seed in any::<u64>()) {
        let mut fixture = fixture::build_fixture(seed);
        // Drive to the fixed-point: a single canonical sequence may leave
        // RECHECK_LEVEL entries on the increase queue (decrease pushes them
        // for re-propagation). Loop until a pass leaves all queues empty.
        let (block_converged, sky_converged) =
            drive_to_fixed_point(&mut fixture).expect("BFS should converge within bound");
        // Re-seed every lit cell with a WRITE_LEVEL entry at its converged
        // level. Each entry must hit the propagated_level <= current early
        // exit in propagate_core, so the second canonical sweep makes no
        // change to storage. Without re-seeding the test only proves "BFS
        // on empty queues is a no-op", which is a much weaker claim than
        // idempotency.
        reseed_write_level_for_every_lit_cell(&mut fixture);
        let (block_after, sky_after) = fixture::run_propagation_and_serialize(&mut fixture);
        prop_assert_eq!(block_converged, block_after);
        prop_assert_eq!(sky_converged, sky_after);
    }
}

/// Walk all 16^3 cells in each light storage and push a `FLAG_WRITE_LEVEL`
/// entry at the cell's stored level onto the matching increase queue. Cells
/// with stored level 0 are skipped (a zero-level WRITE_LEVEL entry would
/// trigger the same early exit but adds queue churn for nothing).
fn reseed_write_level_for_every_lit_cell(fixture: &mut fixture::Fixture) {
    for y in 0..16u8 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                let block_level = fixture.block_light.get(x as usize, y as usize, z as usize);
                if block_level != 0 {
                    fixture.block_workspace.increase_queue.push(pack_bfs_entry(
                        x,
                        z,
                        y,
                        block_level,
                        ALL_DIRECTIONS_BITSET,
                        FLAG_WRITE_LEVEL,
                    ));
                }
                let sky_level = fixture.sky_light.get(x as usize, y as usize, z as usize);
                if sky_level != 0 {
                    fixture.sky_workspace.increase_queue.push(pack_bfs_entry(
                        x,
                        z,
                        y,
                        sky_level,
                        ALL_DIRECTIONS_BITSET,
                        FLAG_WRITE_LEVEL,
                    ));
                }
            }
        }
    }
}

/// Run the canonical propagate sequence repeatedly until all four queues
/// are empty at the end of a sweep. Returns the post-pass byte arrays from
/// the final sweep, or `None` if the loop exceeds `MAX_SWEEPS` (which
/// would indicate a divergent or oscillating state — unexpected for a
/// finite 16^3 grid).
fn drive_to_fixed_point(fixture: &mut fixture::Fixture) -> Option<([u8; 2048], [u8; 2048])> {
    const MAX_SWEEPS: usize = 8;
    for _ in 0..MAX_SWEEPS {
        let snap = fixture::run_propagation_and_serialize(fixture);
        if fixture.block_workspace.increase_queue.is_empty()
            && fixture.block_workspace.decrease_queue.is_empty()
            && fixture.sky_workspace.increase_queue.is_empty()
            && fixture.sky_workspace.decrease_queue.is_empty()
        {
            return Some(snap);
        }
    }
    None
}
