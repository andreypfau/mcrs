use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, Direction};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_value_provider::IntProvider;
use mcrs_minecraft_worldgen_feature::placer::{Predicate, WorldGenVolume};

use crate::tree::provider::StateProvider;

#[derive(Clone, Debug)]
pub struct CompiledCrevasse {
    pub max_width: i32,
    pub max_length: i32,
    pub min_footprint: usize,
    pub cursor_surface_search_range: i32,
    pub column_surface_search_range: i32,
    pub twistiness: f32,
    pub tapering: f32,
    pub turn_factor: f32,
    pub carve_depth: IntProvider,
    pub fill_depth: IntProvider,
    pub surface_block: StateProvider,
    pub fill_block: StateProvider,
    pub carvable_blocks: Predicate,
    pub acceptable_barrier: Predicate,
}

#[derive(Clone, Copy, Debug)]
struct CrevasseColumn {
    surface_y: i32,
    surface_state: VoxelId,
    depth: i32,
}

impl CrevasseColumn {
    fn spans(&self, y: i32) -> bool {
        (self.surface_y - self.depth..=self.surface_y).contains(&y)
    }
}

type Columns = BTreeMap<(i32, i32), CrevasseColumn>;

#[derive(Clone, Copy)]
struct Branch {
    origin: BlockPos,
    yaw: f32,
    steps: i32,
    depth: i32,
}

struct Footprint<'a> {
    columns: &'a Columns,
    kept: &'a BTreeSet<(i32, i32)>,
    fill_level: i32,
}

pub fn place_crevasse<W: WorldGenVolume>(
    config: &CompiledCrevasse,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
) -> bool {
    let carve_depth = config.carve_depth.sample(rng);
    if origin.y <= volume.extent().min_y + carve_depth {
        return false;
    }
    let half_length = config.max_length / 2;
    let yaw = rng.next_f32() * std::f32::consts::TAU;
    let fill_depth = config.fill_depth.sample(rng);

    let mut columns = Columns::new();
    for direction in 0..2 {
        let heading = yaw + direction as f32 * std::f32::consts::PI;
        let branch = Branch {
            origin,
            yaw: heading,
            steps: half_length,
            depth: carve_depth,
        };
        walk_branch(config, volume, rng, branch, &mut columns);
    }

    let kept = connected_columns(&columns, origin);
    if kept.len() < config.min_footprint {
        return false;
    }
    let lowest_surface = kept
        .iter()
        .filter_map(|key| columns.get(key))
        .map(|column| column.surface_y)
        .min()
        .unwrap_or(i32::MAX);
    let fill_level = (origin.y - fill_depth).min(lowest_surface.saturating_sub(1));

    let footprint = Footprint {
        columns: &columns,
        kept: &kept,
        fill_level,
    };
    for &key in &kept {
        if let Some(column) = columns.get(&key) {
            carve_column(config, volume, rng, key, *column, &footprint);
        }
    }
    true
}

fn walk_branch<W: WorldGenVolume>(
    config: &CompiledCrevasse,
    volume: &W,
    rng: &mut WorldgenRandom,
    branch: Branch,
    columns: &mut Columns,
) {
    let Branch {
        origin,
        mut yaw,
        steps,
        depth,
    } = branch;
    let mut cursor_x = f64::from(origin.x);
    let mut cursor_z = f64::from(origin.z);
    let mut previous_y = origin.y;
    let mut acceleration = 0.0f32;
    for step in 0..steps {
        cursor_x += f64::from(yaw).cos();
        cursor_z += f64::from(yaw).sin();
        let turn_direction = rng.next_gaussian() as f32;
        let turn_magnitude = rng.next_f32();
        acceleration = acceleration * 0.5 + turn_direction * turn_magnitude * config.twistiness;
        yaw += acceleration * config.turn_factor;
        let block_x = cursor_x.floor() as i32;
        let block_z = cursor_z.floor() as i32;

        let Some((cursor, _)) = find_surface(
            config,
            volume,
            BlockPos::new(block_x, previous_y, block_z),
            config.cursor_surface_search_range,
        ) else {
            continue;
        };

        let taper = 1.0 - step as f32 / steps as f32 * config.tapering;
        let radius = (config.max_width as f32 * 0.5 * taper).max(0.5);
        let radius_squared = radius * radius;
        let max_depth = ((depth as f32 * taper) as i32).max(1);
        let reach = radius.ceil() as i32;
        for x in block_x - reach..=block_x + reach {
            for z in block_z - reach..=block_z + reach {
                let dx = (x - block_x) as f32;
                let dz = (z - block_z) as f32;
                if dx * dx + dz * dz > radius_squared || columns.contains_key(&(x, z)) {
                    continue;
                }
                let Some((surface, surface_state)) = find_surface(
                    config,
                    volume,
                    BlockPos::new(x, cursor.y, z),
                    config.column_surface_search_range,
                ) else {
                    continue;
                };
                let mut carved = 1;
                let mut below = surface.y - 1;
                while carved < max_depth
                    && config
                        .carvable_blocks
                        .test(volume, BlockPos::new(x, below, z))
                {
                    carved += 1;
                    below -= 1;
                }
                columns.insert(
                    (x, z),
                    CrevasseColumn {
                        surface_y: surface.y,
                        surface_state,
                        depth: carved - 1,
                    },
                );
            }
        }
        previous_y = cursor.y;
    }
}

/// The highest carvable block with air directly above it, searched over
/// `range` blocks centred on `center`.
fn find_surface<W: WorldGenVolume>(
    config: &CompiledCrevasse,
    volume: &W,
    center: BlockPos,
    range: i32,
) -> Option<(BlockPos, VoxelId)> {
    let top = center.y + range / 2;
    let bottom = volume.extent().min_y.max(top - range);
    let mut above_empty = volume.is_air(BlockPos::new(center.x, top + 1, center.z));
    for y in (bottom..=top).rev() {
        let pos = BlockPos::new(center.x, y, center.z);
        if above_empty && config.carvable_blocks.test(volume, pos) {
            return Some((pos, volume.get(pos)));
        }
        above_empty = volume.is_air(pos);
    }
    None
}

fn neighbours((x, z): (i32, i32)) -> impl Iterator<Item = (i32, i32)> {
    Direction::HORIZONTAL.into_iter().map(move |side| {
        let normal = side.normal();
        (x + normal.x, z + normal.z)
    })
}

/// A column whose surface steps more than one block from any collected
/// neighbour's is never kept, and never relays the search.
fn connected_columns(columns: &Columns, origin: BlockPos) -> BTreeSet<(i32, i32)> {
    let is_steep = |key: (i32, i32)| {
        columns.get(&key).is_some_and(|column| {
            neighbours(key).any(|side| {
                columns
                    .get(&side)
                    .is_some_and(|other| (other.surface_y - column.surface_y).abs() > 1)
            })
        })
    };
    let start = (origin.x, origin.z);
    let mut kept = BTreeSet::new();
    if !columns.contains_key(&start) || is_steep(start) {
        return kept;
    }
    kept.insert(start);
    let mut pending = VecDeque::from([start]);
    while let Some(key) = pending.pop_front() {
        for side in neighbours(key) {
            if columns.contains_key(&side) && !kept.contains(&side) && !is_steep(side) {
                kept.insert(side);
                pending.push_back(side);
            }
        }
    }
    kept
}

fn carve_column<W: WorldGenVolume>(
    config: &CompiledCrevasse,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    (x, z): (i32, i32),
    column: CrevasseColumn,
    footprint: &Footprint,
) {
    let Footprint {
        columns,
        kept,
        fill_level,
    } = *footprint;
    let air = volume.world().air;
    let air_block = volume.world().block_of(air);
    let bottom = column.surface_y - column.depth;
    let mut carved = false;
    for y in (bottom..=column.surface_y).rev() {
        let pos = BlockPos::new(x, y, z);
        let fill = config
            .fill_block
            .optional_state(volume, rng, pos)
            .filter(|_| y <= fill_level);
        if fill.is_none()
            && Direction::HORIZONTAL
                .iter()
                .any(|side| volume.holds(&volume.world().any_fluid, pos + side.normal()))
        {
            break;
        }
        let placed = fill.unwrap_or(air);
        let placed_block = fill.map_or(air_block, |state| volume.world().block_of(state));
        for side in Direction::HORIZONTAL {
            let neighbour = pos + side.normal();
            let carved_alongside = kept.contains(&(neighbour.x, neighbour.z))
                && columns
                    .get(&(neighbour.x, neighbour.z))
                    .is_some_and(|other| other.spans(y));
            if !carved_alongside
                && !config.acceptable_barrier.test(volume, neighbour)
                && volume.world().block_of(volume.get(neighbour)) != placed_block
            {
                volume.set(neighbour, column.surface_state);
            }
        }
        volume.set(pos, placed);
        if fill.is_some() && y == bottom {
            let below = pos + IVec3::NEG_Y;
            if !config.acceptable_barrier.test(volume, below)
                && volume.world().block_of(volume.get(below)) != placed_block
            {
                volume.set(below, column.surface_state);
            }
        }
        carved = true;
    }
    if carved {
        let top = BlockPos::new(x, column.surface_y, z);
        if let Some(lid) = config.surface_block.optional_state(volume, rng, top) {
            volume.set(top, lid);
        }
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const WATER: VoxelId = VoxelId(2);
    const LID: VoxelId = VoxelId(3);
    const SURFACE_Y: i32 = 20;

    fn crevasse() -> CompiledCrevasse {
        let stone = Predicate::MatchingStates {
            offset: IVec3::ZERO,
            states: mask_of([STONE]),
        };
        CompiledCrevasse {
            max_width: 4,
            max_length: 16,
            min_footprint: 10,
            cursor_surface_search_range: 8,
            column_surface_search_range: 8,
            twistiness: 4.0,
            tapering: 0.6,
            turn_factor: 0.05,
            carve_depth: IntProvider::Constant(8),
            fill_depth: IntProvider::Constant(4),
            surface_block: StateProvider::Simple(LID),
            fill_block: StateProvider::Simple(WATER),
            carvable_blocks: stone.clone(),
            acceptable_barrier: stone,
        }
    }

    fn stone_plain() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.any_fluid = mask_of([WATER]);
        for x in -24..=24 {
            for z in -24..=24 {
                for y in 0..=SURFACE_Y {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        volume
    }

    #[test]
    fn a_crevasse_on_a_plain_is_lidded_open_below_and_flooded_from_the_fill_level() {
        let mut volume = stone_plain();
        assert!(place_crevasse(
            &crevasse(),
            &mut volume,
            &mut WorldgenRandom::new(42),
            BlockPos::new(0, SURFACE_Y, 0)
        ));
        let fill_level = SURFACE_Y - 4;
        let block = |volume: &FakeVolume, x, y, z| volume.get(BlockPos::new(x, y, z));

        let lids: Vec<(i32, i32)> = volume
            .blocks
            .iter()
            .filter(|(_, state)| **state == LID)
            .map(|((x, y, z), _)| {
                assert_eq!(*y, SURFACE_Y);
                (*x, *z)
            })
            .collect();
        assert!(lids.len() >= 10, "{} columns", lids.len());
        assert!(lids.contains(&(0, 0)), "the origin column is carved");
        for &(x, z) in &lids {
            assert_eq!(block(&volume, x, SURFACE_Y - 1, z), volume.world.air);
        }
        assert_eq!(block(&volume, 0, fill_level, 0), WATER);
        assert!(
            volume
                .blocks
                .iter()
                .all(|((_, y, _), state)| *state != WATER || *y <= fill_level)
        );
    }

    #[test]
    fn a_crevasse_too_close_to_the_bottom_writes_nothing() {
        let mut volume = FakeVolume::default();
        assert!(!place_crevasse(
            &crevasse(),
            &mut volume,
            &mut WorldgenRandom::new(42),
            BlockPos::new(0, -64 + 8, 0)
        ));
        assert!(volume.writes.is_empty());
    }
}
