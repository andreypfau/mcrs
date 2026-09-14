//! Columns of open terrain arriving over many ticks, lit through the queue and
//! the epoch budget rather than in one pass. Every surface block of an open
//! world sees the sky, so any cell the engine leaves below fifteen there is a
//! seam the incremental path failed to repair.

mod common;

use std::sync::Arc;

use common::{AIR, Reference, STONE, registry};
use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos, SectionPos};
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_storage::{PalettedContainer, VoxelPalette};

const SECTIONS_Y: i32 = 6;
const COLUMNS: i32 = 5;

fn bounds() -> LightBounds {
    LightBounds::new(0, SECTIONS_Y - 1)
}

/// Terraced ground: each section column sits on its own plateau, so the world
/// is open everywhere and every surface block is a sky source.
fn surface_height(x: i32, z: i32) -> i32 {
    let step = ((x.div_euclid(16) * 3 + z.div_euclid(16) * 5) % 4) as i32;
    20 + step * 13 + (x.rem_euclid(16) / 8) + (z.rem_euclid(16) / 8)
}

fn section_blocks(pos: SectionPos) -> SectionBlocks {
    let base_y = pos.y * 16;
    let mut solid = 0;
    let mut blocks = VoxelPalette(PalettedContainer::Homogeneous(AIR));
    for local_z in 0..SectionPos::SIZE {
        for local_x in 0..SectionPos::SIZE {
            let top = surface_height(pos.x * 16 + local_x as i32, pos.z * 16 + local_z as i32);
            for local_y in 0..SectionPos::SIZE {
                if base_y + local_y as i32 <= top {
                    blocks.set_cell(local_x, local_y, local_z, STONE);
                    solid += 1;
                }
            }
        }
    }
    if solid == 0 {
        return VoxelPalette(PalettedContainer::Homogeneous(AIR));
    }
    blocks
}

fn surface_of(column: ColumnPos) -> Arc<ColumnSurface> {
    let mut surface = ColumnSurface::new((SECTIONS_Y * 16) as u32, 0);
    for z in 0..SectionPos::SIZE {
        for x in 0..SectionPos::SIZE {
            let top = surface_height(column.x * 16 + x as i32, column.z * 16 + z as i32);
            surface.set(x, z, top + 1);
        }
    }
    Arc::new(surface)
}

fn column_edits(column: ColumnPos) -> Vec<Edit> {
    let mut edits: Vec<Edit> = (0..SECTIONS_Y)
        .map(|y| {
            let pos = SectionPos::new(column.x, y, column.z);
            Edit::LoadSection {
                entity: 0,
                pos,
                blocks: Arc::new(section_blocks(pos)),
            }
        })
        .collect();
    edits.push(Edit::SetColumnSurface {
        column,
        surface: surface_of(column),
    });
    edits
}

/// Runs the engine the way the plugin does: intake feeds the queue, the queue
/// hands out batches under a budget, and a batch finishes a tick or more after
/// it was dispatched.
struct Pump {
    world: LightWorld,
    queue: LightQueue,
    in_flight: Vec<(BoundingBox, LightUpdate)>,
    budget_cells: u64,
    epochs: usize,
}

impl Pump {
    fn new(budget_cells: u64, epochs: usize) -> Self {
        Self {
            world: LightWorld::new(registry(), bounds()),
            queue: LightQueue::default(),
            in_flight: Vec::new(),
            budget_cells,
            epochs,
        }
    }

    fn tick(&mut self, edits: Vec<Edit>) {
        // Publish first, exactly like `LightSet::Publish` before `Dispatch`.
        for (_, update) in std::mem::take(&mut self.in_flight) {
            self.world.apply(update);
        }
        for (column, influence) in self.world.apply_edits(edits) {
            self.queue.push(column, influence);
        }
        let occupied: Vec<BoundingBox> = self.in_flight.iter().map(|(area, _)| *area).collect();
        let free = self.epochs - self.in_flight.len();
        for batch in self.queue.drain_batches(self.budget_cells, &occupied, free) {
            let Some(job) = self.world.prepare_batch(batch) else {
                continue;
            };
            let area = job.area();
            self.in_flight.push((area, job.run()));
        }
    }

    fn settle(&mut self) {
        while !self.queue.is_empty() || !self.in_flight.is_empty() {
            self.tick(Vec::new());
        }
    }
}

fn check(pump: &Pump) {
    let min = BlockPos::new(0, 0, 0);
    let max = BlockPos::new(COLUMNS * 16 - 1, SECTIONS_Y * 16 - 1, COLUMNS * 16 - 1);
    let reference = Reference::compute(&pump.world, min, max);
    if let Some(diff) = reference.diff(&pump.world) {
        panic!("engine disagrees with the reference solver: {diff}");
    }
}

/// Every block above the terrain is open to the sky, so the engine owes it
/// fifteen. Checked separately from the reference because it is the property a
/// player actually sees, and it names the position rather than the first
/// disagreement.
fn check_open_sky_is_full(pump: &Pump) {
    for z in 0..COLUMNS * 16 {
        for x in 0..COLUMNS * 16 {
            let top = surface_height(x, z);
            for y in top + 1..SECTIONS_Y * 16 {
                let pos = BlockPos::new(x, y, z);
                let got = pump.world.light_at(pos, Layer::Sky).get();
                assert_eq!(
                    got,
                    15,
                    "open sky at {pos:?} (surface {top}) is {got}, floor {}",
                    pump.world.sky_floor(BlockColumn { x, z })
                );
            }
        }
    }
}

fn columns_in_load_order() -> Vec<ColumnPos> {
    // Distance to the origin, the order the server's priority actually produces.
    let mut columns: Vec<ColumnPos> = (0..COLUMNS)
        .flat_map(|z| (0..COLUMNS).map(move |x| ColumnPos::new(x, z)))
        .collect();
    columns.sort_by_key(|c| c.x * c.x + c.z * c.z);
    columns
}

#[test]
fn terraced_columns_arriving_one_per_tick_are_lit_like_one_pass() {
    let mut pump = Pump::new(8 << 20, 1);
    for column in columns_in_load_order() {
        pump.tick(column_edits(column));
    }
    pump.settle();
    check_open_sky_is_full(&pump);
    check(&pump);
}

#[test]
fn terraced_columns_lit_under_a_budget_that_splits_them() {
    // Small enough that a single column column fills a batch, so every seam
    // between two columns is a seam between two epochs.
    let mut pump = Pump::new(1, 1);
    for column in columns_in_load_order() {
        pump.tick(column_edits(column));
    }
    pump.settle();
    check_open_sky_is_full(&pump);
    check(&pump);
}

#[test]
fn terraced_columns_lit_by_several_epochs_in_flight() {
    let mut pump = Pump::new(1 << 20, 4);
    for column in columns_in_load_order() {
        pump.tick(column_edits(column));
    }
    pump.settle();
    check_open_sky_is_full(&pump);
    check(&pump);
}

/// The whole world in one intake, which is what a bulk load looks like.
#[test]
fn terraced_columns_arriving_together_are_lit_like_one_pass() {
    let mut pump = Pump::new(1 << 20, 4);
    let edits: Vec<Edit> = columns_in_load_order()
        .into_iter()
        .flat_map(column_edits)
        .collect();
    pump.tick(edits);
    pump.settle();
    check_open_sky_is_full(&pump);
    check(&pump);
}

/// A section arriving after its column was already lit, which is how the light
/// world sees a column whose sections land across ticks.
#[test]
fn sections_of_a_column_arriving_across_ticks_are_lit_like_one_pass() {
    let mut pump = Pump::new(8 << 20, 2);
    for column in columns_in_load_order() {
        for y in (0..SECTIONS_Y).rev() {
            let pos = SectionPos::new(column.x, y, column.z);
            pump.tick(vec![Edit::LoadSection {
                entity: 0,
                pos,
                blocks: Arc::new(section_blocks(pos)),
            }]);
        }
        pump.tick(vec![Edit::SetColumnSurface {
            column,
            surface: surface_of(column),
        }]);
    }
    pump.settle();
    check_open_sky_is_full(&pump);
    check(&pump);
}

/// A ticket expiring and coming back, which at a large render distance happens
/// constantly. An unloaded section is opaque, so the seam has to be torn down
/// and rebuilt twice.
#[test]
fn columns_unloaded_and_revived_are_lit_like_one_pass() {
    let mut pump = Pump::new(8 << 20, 2);
    for column in columns_in_load_order() {
        pump.tick(column_edits(column));
    }
    pump.settle();

    let evicted: Vec<ColumnPos> = columns_in_load_order()
        .into_iter()
        .filter(|c| (c.x + c.z) % 2 == 0)
        .collect();
    for column in &evicted {
        let unloads: Vec<Edit> = (0..SECTIONS_Y)
            .map(|y| Edit::UnloadSection {
                pos: SectionPos::new(column.x, y, column.z),
            })
            .collect();
        pump.tick(unloads);
    }
    pump.settle();
    for column in &evicted {
        pump.tick(column_edits(*column));
    }
    pump.settle();

    check_open_sky_is_full(&pump);
    check(&pump);
}

/// A roof over part of the world, so the cells under it are reached only by sky
/// light travelling sideways — the case a per-column sky floor cannot answer on
/// its own.
#[test]
fn a_roof_lit_across_column_seams_matches_one_pass() {
    let roofed = |x: i32, z: i32| (24..56).contains(&x) && (24..56).contains(&z);
    let blocks_of = |pos: SectionPos| {
        let base_y = pos.y * 16;
        let mut blocks = VoxelPalette(PalettedContainer::Homogeneous(AIR));
        let mut solid = 0;
        for local_z in 0..SectionPos::SIZE {
            for local_x in 0..SectionPos::SIZE {
                let (x, z) = (pos.x * 16 + local_x as i32, pos.z * 16 + local_z as i32);
                for local_y in 0..SectionPos::SIZE {
                    let y = base_y + local_y as i32;
                    if y <= 20 || (y == 40 && roofed(x, z)) {
                        blocks.set_cell(local_x, local_y, local_z, STONE);
                        solid += 1;
                    }
                }
            }
        }
        if solid == 0 {
            return VoxelPalette(PalettedContainer::Homogeneous(AIR));
        }
        blocks
    };

    let mut pump = Pump::new(1 << 20, 3);
    for column in columns_in_load_order() {
        let edits: Vec<Edit> = (0..SECTIONS_Y)
            .map(|y| {
                let pos = SectionPos::new(column.x, y, column.z);
                Edit::LoadSection {
                    entity: 0,
                    pos,
                    blocks: Arc::new(blocks_of(pos)),
                }
            })
            .collect();
        pump.tick(edits);
    }
    pump.settle();
    check(&pump);
}

/// Sky light entering a shaft and spreading sideways along a tunnel, which is
/// the one shape where a section column's sky floor swings from the world floor
/// to the surface inside a single column and the answer at depth is carried
/// horizontally across chunk seams.
#[test]
fn a_shaft_lights_a_tunnel_across_chunk_seams() {
    const GROUND: i32 = 79;
    const TUNNEL_LOW: i32 = 16;
    const TUNNEL_HIGH: i32 = 18;
    const SHAFT_X: std::ops::Range<i32> = 24..26;
    const TUNNEL_Z: std::ops::Range<i32> = 24..26;

    let open = |x: i32, y: i32, z: i32| {
        if y > GROUND {
            return true;
        }
        let in_shaft = SHAFT_X.contains(&x) && TUNNEL_Z.contains(&z) && y >= TUNNEL_LOW;
        let in_tunnel = (TUNNEL_LOW..=TUNNEL_HIGH).contains(&y)
            && TUNNEL_Z.contains(&z)
            && (SHAFT_X.start..COLUMNS * 16 - 8).contains(&x);
        in_shaft || in_tunnel
    };

    let blocks_of = |pos: SectionPos| {
        let mut blocks = VoxelPalette(PalettedContainer::Homogeneous(AIR));
        let mut solid = 0;
        for local_z in 0..SectionPos::SIZE {
            for local_x in 0..SectionPos::SIZE {
                for local_y in 0..SectionPos::SIZE {
                    let (x, y, z) = (
                        pos.x * 16 + local_x as i32,
                        pos.y * 16 + local_y as i32,
                        pos.z * 16 + local_z as i32,
                    );
                    if !open(x, y, z) {
                        blocks.set_cell(local_x, local_y, local_z, STONE);
                        solid += 1;
                    }
                }
            }
        }
        if solid == 0 {
            return VoxelPalette(PalettedContainer::Homogeneous(AIR));
        }
        blocks
    };

    for (budget, epochs) in [(8u64 << 20, 1usize), (1, 1), (1 << 20, 4)] {
        let mut pump = Pump::new(budget, epochs);
        for column in columns_in_load_order() {
            let edits: Vec<Edit> = (0..SECTIONS_Y)
                .map(|y| {
                    let pos = SectionPos::new(column.x, y, column.z);
                    Edit::LoadSection {
                        entity: 0,
                        pos,
                        blocks: Arc::new(blocks_of(pos)),
                    }
                })
                .collect();
            pump.tick(edits);
        }
        pump.settle();

        // Named separately from the reference diff so a failure says which cell
        // of the tunnel went dark rather than the first disagreement anywhere.
        let mut profile = Vec::new();
        for x in SHAFT_X.start..COLUMNS * 16 - 8 {
            profile.push(
                pump.world
                    .light_at(BlockPos::new(x, TUNNEL_LOW + 1, 25), Layer::Sky)
                    .get(),
            );
        }
        assert!(
            profile[0] == 15,
            "budget={budget} epochs={epochs}: the shaft bottom is {}, not 15",
            profile[0]
        );
        for (step, pair) in profile.windows(2).enumerate() {
            assert!(
                pair[0] <= pair[1] + 1,
                "budget={budget} epochs={epochs}: sky light drops from {} to {} at x={} \
                 (profile {profile:?})",
                pair[0],
                pair[1],
                SHAFT_X.start + step as i32,
            );
        }
        check(&pump);
    }
}
