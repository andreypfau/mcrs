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
