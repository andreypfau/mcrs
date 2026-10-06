use std::cell::Cell;
use std::rc::Rc;

use mcrs_minecraft_chunk::{Blocks, BoxVolume, VoxelId};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use rand_xoshiro::rand_core::{Infallible, TryRng};

use crate::{BetaOreBlockIds, place_all_ores};

// ── Counting RNG: pins total LegacyRandom advances for the ore stream ───────────

#[derive(Clone)]
struct CountingRng {
    inner: LegacyRandom,
    draws: Rc<Cell<u64>>,
}

impl CountingRng {
    fn new(seed: u64, draws: Rc<Cell<u64>>) -> Self {
        CountingRng {
            inner: LegacyRandom::new(seed),
            draws,
        }
    }
    fn inc(&self) {
        self.draws.set(self.draws.get() + 1);
    }
}

impl TryRng for CountingRng {
    type Error = Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        self.inc();
        self.inner.try_next_u32()
    }
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        self.inc();
        self.inner.try_next_u64()
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        for _ in 0..dst.len().div_ceil(8) {
            self.inc();
        }
        self.inner.try_fill_bytes(dst)
    }
}

impl Random for CountingRng {
    fn is_legacy(&self) -> bool {
        true
    }
    fn next_bool(&mut self) -> bool {
        self.inc();
        self.inner.next_bool()
    }
    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        self.inc();
        self.inner.next_u32_bound(bound)
    }
    fn next_f32(&mut self) -> f32 {
        self.inc();
        self.inner.next_f32()
    }
    fn next_f64(&mut self) -> f64 {
        self.inc();
        self.inc();
        self.inner.next_f64()
    }

    fn next_gaussian(&mut self) -> f64 {
        self.inner.next_gaussian()
    }
    fn fork(&mut self) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork(),
            draws: self.draws.clone(),
        }
    }
    fn fork_at<T: Into<bevy_math::IVec3>>(&mut self, pos: T) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork_at(pos),
            draws: self.draws.clone(),
        }
    }
    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork_hash(seed),
            draws: self.draws.clone(),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Stone over the 3×3 of columns around chunk (0, 0), in world coordinates: the
/// region a `Run` sees, so a vein that leaves its own column still lands.
fn stone_volume(stone: VoxelId) -> BoxVolume {
    BoxVolume::filled(
        BlockPos::new(-16, 0, -16),
        BlockPos::new(31, 127, 31),
        stone,
    )
}

/// Every placed block as (column offset from the centre, state), so a census
/// can tell the half of a vein that stayed home from the half that crossed.
fn placed(volume: &BoxVolume, stone: VoxelId) -> Vec<((i32, i32), VoxelId)> {
    volume
        .iter()
        .filter(|(_, state)| *state != stone)
        .map(|(at, state)| ((at.x.div_euclid(16), at.z.div_euclid(16)), state))
        .collect()
}

/// Beta populate seed for a chunk (matches apply_beta_ores_in's derivation).
fn populate_seed(chunk_x: i32, chunk_z: i32, world_seed: i64) -> i64 {
    let mut s = LegacyRandom::new(world_seed as u64);
    let i1 = s.next_java_long() / 2 * 2 + 1;
    let j1 = s.next_java_long() / 2 * 2 + 1;
    (chunk_x as i64)
        .wrapping_mul(i1)
        .wrapping_add((chunk_z as i64).wrapping_mul(j1))
        ^ world_seed
}

/// Pinned total LegacyRandom advances for the full ore-placement stream on chunk
/// (0,0) at seed 12345. Recorded on the first green run, asserted thereafter; any
/// change to vein counts/sizes/order shifts this value.
const ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345: u64 = 3737;

/// Blocks the driver places on the 3×3 stone region from chunk (0,0), seed 12345,
/// as (own column, columns it crossed into). Re-recorded when the pass moved onto
/// the ladder's `Run`: it used to write only the first number, the rest of every
/// bordering vein having been clipped away.
const ORE_BLOCKS_CHUNK_0_0_SEED_12345: (usize, usize) = (964, 2580);

fn drive(seed: i64, ids: &BetaOreBlockIds) -> (BoxVolume, u64) {
    let draws = Rc::new(Cell::new(0u64));
    let mut rng = CountingRng::new(seed as u64, draws.clone());
    let mut volume = stone_volume(ids.stone.0.into());
    place_all_ores(&mut volume, 0, 0, &mut rng, ids);
    (volume, draws.get())
}

#[test]
fn veins_cross_the_column_border_on_the_pinned_draw_count() {
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let (volume, draws) = drive(populate_seed(0, 0, 12345), &ids);
    assert_eq!(draws, ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345);
    let placed = placed(&volume, ids.stone.0.into());

    let own = placed.iter().filter(|(col, _)| *col == (0, 0)).count();
    let crossed = placed.len() - own;

    assert_eq!((own, crossed), ORE_BLOCKS_CHUNK_0_0_SEED_12345);

    // A vein reaches at most fourteen blocks past an origin inside its own
    // column, so the writes land in the 3×3 and never beyond it.
    for (col, _) in &placed {
        assert!(
            (-1..=1).contains(&col.0) && (-1..=1).contains(&col.1),
            "a write reached column {col:?}"
        );
    }

    // Every resource of the table places at least one block somewhere.
    for state in [
        ids.dirt,
        ids.gravel,
        ids.coal,
        ids.iron,
        ids.gold,
        ids.redstone,
        ids.diamond,
        ids.lapis,
    ] {
        assert!(
            placed.iter().any(|(_, got)| *got == state.0.into()),
            "no block of {state:?} was placed"
        );
    }
}

/// Beta's populate step reached as a feature writes what running it on the
/// region directly writes: the source the feature's step hands it plays no part.
#[test]
fn the_populate_feature_is_the_populate_step() {
    use std::sync::Arc;

    use mcrs_minecraft_protocol::ColumnPos;

    use crate::ColumnBlocks;
    use crate::beta_ores::apply_beta_ores_in;
    use crate::stages::{ColumnRegion, run_column};

    let router = super::build_beta_router();
    let seed = router.world_seed as i64;
    let (_, registry) = super::beta_surface::build_beta_biome_source();
    let program = Arc::new(super::beta_populate_program(&registry, seed));
    let ctx = super::fill_context_with(router, Some(program));
    let stone: VoxelId = super::corpus().default_state("minecraft:stone").0.into();
    let center = ColumnPos::new(3, -2);
    let snapshots = super::region_of(center, |col| {
        super::flat_snapshot(
            col,
            &ctx.y_sections,
            |section_y| (0..8).contains(&section_y).then_some(stone),
            None,
        )
    });

    let writes = |populate: &dyn Fn(&mut ColumnRegion<'_>)| {
        let column = ColumnBlocks::new(&ctx.y_sections);
        column.unpack(&snapshots[4].sections);
        let mut region = ColumnRegion::new(&snapshots, &column, &ctx);
        populate(&mut region);
        region
            .finish()
            .into_iter()
            .map(|(col, delta)| (col, delta.writes))
            .collect::<Vec<_>>()
    };
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let through_program = writes(&|region| run_column(&ctx, region, 0));
    let direct = writes(&|region| apply_beta_ores_in(region, center.x, center.z, seed, &ids));

    assert!(
        direct.iter().map(|(_, writes)| writes.len()).sum::<usize>() > 0,
        "the populate step wrote nothing"
    );
    assert_eq!(through_program, direct);
}
