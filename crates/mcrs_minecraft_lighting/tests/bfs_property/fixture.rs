//! Deterministic 16x16x16 chunk fixtures for the BFS property snapshot.
//!
//! Used by both the (untracked) `examples/capture_bfs_snapshot.rs`
//! generator and `tests/bfs_property.rs` verifier. The two MUST stay in
//! lockstep; the snapshot's byte-for-byte safety net assumes that
//! re-running the same protocol from the same seed reproduces the same
//! initial conditions. Any non-trivial change here invalidates the
//! committed `snapshot.json` and the generator must be re-run.

use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::bfs::{
    pack_bfs_entry, propagate_decrease, propagate_decrease_sky, propagate_increase,
    propagate_increase_sky, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL,
};
use mcrs_minecraft_lighting::components::{
    BlockEgress, BlockLightWorkspace, SkyEgress, SkyLightWorkspace,
};
use mcrs_minecraft_lighting::nibble::NibbleArray;
use mcrs_minecraft_lighting::storage::LightStorage;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_protocol::BlockStateId;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[allow(dead_code)]
pub const N_FIXTURES: u64 = 32;
pub const OPAQUE_PROBABILITY: f64 = 0.40;

const AIR_ID: BlockStateId = BlockStateId(0);
const STONE_ID: BlockStateId = BlockStateId(1);
const TORCH_ID: BlockStateId = BlockStateId(0x1000);

pub fn build_table() -> BlockLightTable {
    const SIZE: usize = 0x1001;
    let mut emission = vec![0u8; SIZE].into_boxed_slice();
    let mut dampening = vec![0u8; SIZE].into_boxed_slice();
    let mut occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); SIZE].into_boxed_slice();
    let mut flags = vec![0u8; SIZE].into_boxed_slice();

    let air = AIR_ID.0 as usize;
    flags[air] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

    let stone = STONE_ID.0 as usize;
    dampening[stone] = 15;
    occlusion[stone] = VoxelShape::block();
    flags[stone] =
        flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;

    let torch = TORCH_ID.0 as usize;
    emission[torch] = 14;
    flags[torch] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

    BlockLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

pub struct Fixture {
    pub table: BlockLightTable,
    pub palette: BlockPalette,
    pub block_light: LightStorage,
    pub sky_light: LightStorage,
    pub block_workspace: BlockLightWorkspace,
    pub sky_workspace: SkyLightWorkspace,
    pub block_egress: BlockEgress,
    pub sky_egress: SkyEgress,
}

pub fn zero_storage() -> LightStorage {
    LightStorage::Mixed(Box::new(NibbleArray::zeros()))
}

/// Build a fixture from `seed`. The protocol is:
///
/// 1. Fill the 16x16x16 palette: each cell is `STONE_ID` with probability
///    `OPAQUE_PROBABILITY`, else `AIR_ID`. The deterministic order is the
///    canonical y-major / z-mid / x-minor scan.
/// 2. Pick `1..=3` block emitters at random `(x, y, z)`; for each, place
///    `TORCH_ID` at the cell, write the emission level into `block_light`,
///    and push the seed entry onto `block_workspace.increase_queue` with
///    `FLAG_WRITE_LEVEL`.
/// 3. Pick `1..=3` sky source columns at random `(x, z)`; for each, write
///    level 15 at `(x, 15, z)` into `sky_light` and push onto
///    `sky_workspace.increase_queue` with `FLAG_WRITE_LEVEL`.
/// 4. Pick `0..=2` random decrease seeds at random `(x, y, z, prev_level)`;
///    push onto `block_workspace.decrease_queue` (no `sky_workspace` seeds
///    here — sky-decrease is exercised by Step 2 indirectly through the
///    increase-then-decrease passes the caller runs).
pub fn build_fixture(seed: u64) -> Fixture {
    let mut rng = StdRng::seed_from_u64(seed);
    let table = build_table();
    let mut palette = BlockPalette::default();
    palette.fill(AIR_ID);

    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if rng.random_bool(OPAQUE_PROBABILITY) {
                    palette.set(BlockPos::new(x, y, z), STONE_ID);
                }
            }
        }
    }

    let mut block_light = zero_storage();
    let mut sky_light = zero_storage();
    let mut block_workspace = BlockLightWorkspace::default();
    let mut sky_workspace = SkyLightWorkspace::default();

    let n_emitters = rng.random_range(1..=3u32);
    for _ in 0..n_emitters {
        let x = rng.random_range(0..16u8);
        let y = rng.random_range(0..16u8);
        let z = rng.random_range(0..16u8);
        let level = rng.random_range(1..=15u8);
        palette.set(BlockPos::new(x as i32, y as i32, z as i32), TORCH_ID);
        block_light.set(x as usize, y as usize, z as usize, level);
        block_workspace.increase_queue.push(pack_bfs_entry(
            x,
            z,
            y,
            level,
            ALL_DIRECTIONS_BITSET,
            FLAG_WRITE_LEVEL,
        ));
    }

    let n_sky_sources = rng.random_range(1..=3u32);
    for _ in 0..n_sky_sources {
        let x = rng.random_range(0..16u8);
        let z = rng.random_range(0..16u8);
        sky_light.set(x as usize, 15, z as usize, 15);
        sky_workspace.increase_queue.push(pack_bfs_entry(
            x,
            z,
            15,
            15,
            ALL_DIRECTIONS_BITSET,
            FLAG_WRITE_LEVEL,
        ));
    }

    let n_decrease_seeds = rng.random_range(0..=2u32);
    for _ in 0..n_decrease_seeds {
        let x = rng.random_range(0..16u8);
        let y = rng.random_range(0..16u8);
        let z = rng.random_range(0..16u8);
        let level = rng.random_range(1..=15u8);
        block_workspace.decrease_queue.push(pack_bfs_entry(
            x,
            z,
            y,
            level,
            ALL_DIRECTIONS_BITSET,
            0,
        ));
    }

    Fixture {
        table,
        palette,
        block_light,
        sky_light,
        block_workspace,
        sky_workspace,
        block_egress: BlockEgress::default(),
        sky_egress: SkyEgress::default(),
    }
}

/// Run the four `propagate_*` calls in the canonical order:
/// `increase`, `decrease`, `increase_sky`, `decrease_sky`. Returns the
/// post-pass `(block_light_bytes, sky_light_bytes)` as 2048-byte arrays.
/// A `LightStorage` that drained to `Uniform(v)` or `Null` is expanded to
/// the equivalent 2048-byte buffer so the snapshot comparison is always
/// byte-level.
pub fn run_propagation_and_serialize(fixture: &mut Fixture) -> ([u8; 2048], [u8; 2048]) {
    propagate_increase(
        &fixture.table,
        &fixture.palette,
        &mut fixture.block_light,
        &mut fixture.block_workspace,
        &mut fixture.block_egress,
    );
    propagate_decrease(
        &fixture.table,
        &fixture.palette,
        &mut fixture.block_light,
        &mut fixture.block_workspace,
        &mut fixture.block_egress,
    );
    propagate_increase_sky(
        &fixture.table,
        &fixture.palette,
        &mut fixture.sky_light,
        &mut fixture.sky_workspace,
        &mut fixture.sky_egress,
    );
    propagate_decrease_sky(
        &fixture.table,
        &fixture.palette,
        &mut fixture.sky_light,
        &mut fixture.sky_workspace,
        &mut fixture.sky_egress,
    );
    (
        storage_to_bytes(&fixture.block_light),
        storage_to_bytes(&fixture.sky_light),
    )
}

fn storage_to_bytes(s: &LightStorage) -> [u8; 2048] {
    match s {
        LightStorage::Null => [0u8; 2048],
        LightStorage::Uniform(v) => {
            let packed = (*v & 0x0F) | ((*v & 0x0F) << 4);
            [packed; 2048]
        }
        LightStorage::Mixed(arr) => *arr.0,
    }
}

pub mod b64 {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
        let mut iter = bytes.chunks_exact(3);
        for chunk in iter.by_ref() {
            let b0 = chunk[0];
            let b1 = chunk[1];
            let b2 = chunk[2];
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            out.push(ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            out.push(ALPHABET[(b2 & 0x3F) as usize] as char);
        }
        let rem = iter.remainder();
        match rem.len() {
            0 => {}
            1 => {
                let b0 = rem[0];
                out.push(ALPHABET[(b0 >> 2) as usize] as char);
                out.push(ALPHABET[((b0 & 0x03) << 4) as usize] as char);
                out.push('=');
                out.push('=');
            }
            2 => {
                let b0 = rem[0];
                let b1 = rem[1];
                out.push(ALPHABET[(b0 >> 2) as usize] as char);
                out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
                out.push(ALPHABET[((b1 & 0x0F) << 2) as usize] as char);
                out.push('=');
            }
            _ => unreachable!(),
        }
        out
    }

    pub fn decode(s: &str) -> Vec<u8> {
        let bytes = s.as_bytes();
        let mut lookup = [255u8; 256];
        for (i, &c) in ALPHABET.iter().enumerate() {
            lookup[c as usize] = i as u8;
        }
        let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
        let mut i = 0;
        while i + 4 <= bytes.len() {
            let c0 = lookup[bytes[i] as usize];
            let c1 = lookup[bytes[i + 1] as usize];
            let c2 = bytes[i + 2];
            let c3 = bytes[i + 3];
            let v0 = (c0 << 2) | (c1 >> 4);
            out.push(v0);
            if c2 != b'=' {
                let c2v = lookup[c2 as usize];
                let v1 = ((c1 & 0x0F) << 4) | (c2v >> 2);
                out.push(v1);
                if c3 != b'=' {
                    let c3v = lookup[c3 as usize];
                    let v2 = ((c2v & 0x03) << 6) | c3v;
                    out.push(v2);
                }
            }
            i += 4;
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn round_trip_empty() {
            let s = encode(&[]);
            assert_eq!(s, "");
            assert_eq!(decode(&s), Vec::<u8>::new());
        }

        #[test]
        fn round_trip_one_byte() {
            let s = encode(&[0xAB]);
            assert_eq!(decode(&s), vec![0xAB]);
        }

        #[test]
        fn round_trip_two_bytes() {
            let s = encode(&[0xAB, 0xCD]);
            assert_eq!(decode(&s), vec![0xAB, 0xCD]);
        }

        #[test]
        fn round_trip_three_bytes() {
            let s = encode(&[0xAB, 0xCD, 0xEF]);
            assert_eq!(decode(&s), vec![0xAB, 0xCD, 0xEF]);
        }

        #[test]
        fn round_trip_2048_random() {
            let bytes: Vec<u8> = (0..2048u32).map(|i| (i * 31 ^ 0x5A) as u8).collect();
            let encoded = encode(&bytes);
            let decoded = decode(&encoded);
            assert_eq!(decoded, bytes);
        }
    }
}
