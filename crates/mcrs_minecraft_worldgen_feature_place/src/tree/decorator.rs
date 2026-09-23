use rustc_hash::FxHashSet as HashSet;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
pub use mcrs_minecraft_core::value_provider::Weighted;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::block_predicate::Direction;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen_feature::tree::TreeDecorator;

use super::TreeTables;
use super::provider::StateProvider;
#[cfg(test)]
use crate::block_entity::BEE_MIN_TICKS_IN_HIVE;
use crate::block_entity::{BeeOccupant, GeneratedBlockEntity};
use crate::tree::trunk::random_horizontal;
use mcrs_minecraft_random::{shuffle, shuffled};

/// The four sides a bee nest may sit on: the horizontals minus the one it would
/// face away from, since it always faces south.
const HIVE_SIDES: [Direction; 3] = [Direction::East, Direction::South, Direction::West];

const BEE_NEST_FACING: Direction = Direction::South;

/// Every state and set of states a decorator names, resolved once.
#[derive(Clone, Debug)]
pub struct TreePalette {
    pub vines: StateMask,
    pub shelf_mushrooms: StateMask,
    /// A vine attached on one side only, indexed over [`Direction::HORIZONTAL`].
    pub vine_side: [VoxelId; 4],
    /// Facing [`BEE_NEST_FACING`].
    pub bee_nest: VoxelId,
    /// `[age][facing]`, the facing over [`Direction::HORIZONTAL`].
    pub cocoa: [[VoxelId; 4]; 3],
    /// `[age][facing]`, the facing over [`Direction::HORIZONTAL`].
    pub shelf_mushroom: [[VoxelId; 4]; 2],
    /// Indexed by the `tip` property.
    pub pale_hanging_moss: [VoxelId; 2],
    pub creaking_heart: VoxelId,
}

/// What a decorator sees of the tree that just went in: the blocks it wrote,
/// each list ordered bottom to top, and the volume under them.
pub struct DecoratorContext<'a, W> {
    pub volume: &'a mut W,
    pub tables: &'a TreeTables,
    pub sink: &'a mut dyn TreeSink<W>,
    logs: Vec<BlockPos>,
    leaves: Vec<BlockPos>,
    roots: Vec<BlockPos>,
    decorations: Vec<BlockPos>,
}

/// `java.util.HashSet` iteration order over `BlockPos`, which is the order the
/// reference's decorator context starts from before its stable sort on Y.
///
/// The tree feature accumulates its trunk, foliage and root positions in a
/// `HashSet`, so within one Y the order is the table's, not the placer's, and
/// every decorator that shuffles or walks a list starts from that permutation.
/// The set also deduplicates, which a placer revisiting a position relies on.
///
/// Known ceiling: a bin of eight or more once the table holds sixty-four
/// entries becomes a red-black tree in the JVM and iterates in a different
/// order. `BlockPos` hashes spread well enough that a tree never reaches it.
pub(crate) fn java_set_order(positions: &[BlockPos]) -> Vec<BlockPos> {
    let mut table: Vec<Vec<(u32, BlockPos)>> = vec![Vec::new(); 16];
    let mut size = 0usize;
    for &pos in positions {
        let hash = pos
            .y
            .wrapping_add(pos.z.wrapping_mul(31))
            .wrapping_mul(31)
            .wrapping_add(pos.x) as u32;
        let hash = hash ^ (hash >> 16);
        let index = (hash as usize) & (table.len() - 1);
        let bucket = &mut table[index];
        if bucket.iter().any(|(_, seen)| *seen == pos) {
            continue;
        }
        bucket.push((hash, pos));
        size += 1;
        if size > table.len() * 3 / 4 {
            let capacity = table.len() * 2;
            let mut grown: Vec<Vec<(u32, BlockPos)>> = vec![Vec::new(); capacity];
            for entry in table.into_iter().flatten() {
                grown[(entry.0 as usize) & (capacity - 1)].push(entry);
            }
            table = grown;
        }
    }
    table.into_iter().flatten().map(|(_, pos)| pos).collect()
}

impl<'a, W: WorldGenVolume> DecoratorContext<'a, W> {
    pub fn new(
        volume: &'a mut W,
        tables: &'a TreeTables,
        sink: &'a mut dyn TreeSink<W>,
        logs: &[BlockPos],
        leaves: &[BlockPos],
        roots: &[BlockPos],
    ) -> Self {
        let mut logs = java_set_order(logs);
        let mut leaves = java_set_order(leaves);
        let mut roots = java_set_order(roots);
        for list in [&mut logs, &mut leaves, &mut roots] {
            list.sort_by_key(|pos| pos.y);
        }
        DecoratorContext {
            volume,
            tables,
            sink,
            logs,
            leaves,
            roots,
            decorations: Vec::new(),
        }
    }

    /// The positions the decorators wrote, which join the tree's bounding box
    /// and pre-fill the leaf relaxation's shape.
    pub fn into_decorations(self) -> Vec<BlockPos> {
        self.decorations
    }

    fn holds(&self, mask: &StateMask, pos: BlockPos) -> bool {
        self.volume.holds(mask, pos)
    }

    fn is_air(&self, pos: BlockPos) -> bool {
        self.volume.is_air(pos)
    }

    fn is_water_or_water_nearby(&self, pos: BlockPos) -> bool {
        let water = &self.volume.world().water_fluid;
        self.holds(water, pos)
            || Direction::HORIZONTAL
                .iter()
                .any(|side| self.holds(water, pos + side.normal()))
    }

    fn set(&mut self, pos: BlockPos, state: VoxelId) {
        self.decorations.push(pos);
        self.volume.set(pos, state);
    }

    fn place_vine(&mut self, pos: BlockPos, side: Direction) {
        self.set(pos, self.tables.palette.vine_side[horizontal_index(side)]);
    }
    fn lowest_trunk_or_root(&self) -> Vec<BlockPos> {
        if self.roots.is_empty() {
            self.logs.clone()
        } else if !self.logs.is_empty() && self.roots[0].y == self.logs[0].y {
            self.logs.iter().chain(&self.roots).copied().collect()
        } else {
            self.roots.clone()
        }
    }
}

/// A tree decorator with the block state provider four of them carry resolved.
#[derive(Clone, Debug)]
pub struct CompiledTreeDecorator(pub TreeDecorator<StateProvider>);

/// What a tree's decorators write beside blocks: the block entities they grow,
/// and the bare `pale_moss_patch` some of them run on the tree's own source.
///
/// One interface rather than two parameters, because both reach the same state
/// — the patch is a feature of its own and may grow block entities too — and
/// two parameters cannot both borrow it.
pub trait TreeSink<W> {
    fn block_entity(&mut self, entity: GeneratedBlockEntity);

    /// `pale_moss` runs a whole feature on the tree's source, and that feature
    /// lives outside this crate.
    fn moss_patch(&mut self, volume: &mut W, rng: &mut WorldgenRandom, at: BlockPos);
}

/// A sink with no patch to run: it keeps the block entities and drops the moss.
///
/// Only right where the corpus carries no `pale_moss` decorator — a tree that
/// does grows nothing where its patch belongs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntitiesOnly(pub Vec<GeneratedBlockEntity>);

impl<W> TreeSink<W> for EntitiesOnly {
    fn block_entity(&mut self, entity: GeneratedBlockEntity) {
        self.0.push(entity);
    }

    fn moss_patch(&mut self, _window: &mut W, _rng: &mut WorldgenRandom, _at: BlockPos) {}
}

impl CompiledTreeDecorator {
    pub fn place<W: WorldGenVolume>(
        &self,
        ctx: &mut DecoratorContext<W>,
        rng: &mut WorldgenRandom,
    ) {
        match &self.0 {
            TreeDecorator::TrunkVine {} => {
                for index in 0..ctx.logs.len() {
                    let pos = ctx.logs[index];
                    for side in VINE_SIDES {
                        if rng.next_i32_bound(3) > 0 {
                            let at = pos + side.normal();
                            if ctx.is_air(at) {
                                ctx.place_vine(at, side.opposite());
                            }
                        }
                    }
                }
            }
            TreeDecorator::LeaveVine { probability } => {
                for index in 0..ctx.leaves.len() {
                    let pos = ctx.leaves[index];
                    for side in VINE_SIDES {
                        if rng.next_f32() < probability.0 as f32 {
                            let at = pos + side.normal();
                            if ctx.is_air(at) {
                                hang_vine(ctx, at, side.opposite());
                            }
                        }
                    }
                }
            }
            TreeDecorator::PaleMoss {
                leaves_probability,
                trunk_probability,
                ground_probability,
            } => {
                let shuffled = shuffled(&ctx.logs, rng);
                if shuffled.is_empty() {
                    return;
                }
                let ground_origin = shuffled
                    .iter()
                    .min_by_key(|pos| pos.y)
                    .copied()
                    .expect("the list is not empty");
                if rng.next_f32() < ground_probability.0 as f32 {
                    ctx.sink
                        .moss_patch(ctx.volume, rng, ground_origin + IVec3::Y);
                }
                for index in 0..ctx.logs.len() {
                    let pos = ctx.logs[index];
                    if rng.next_f32() < trunk_probability.0 as f32 {
                        let below = pos + IVec3::NEG_Y;
                        if ctx.is_air(below) {
                            hang_moss(ctx, below, rng);
                        }
                    }
                }
                for index in 0..ctx.leaves.len() {
                    let pos = ctx.leaves[index];
                    if rng.next_f32() < leaves_probability.0 as f32 {
                        let below = pos + IVec3::NEG_Y;
                        if ctx.is_air(below) {
                            hang_moss(ctx, below, rng);
                        }
                    }
                }
            }
            TreeDecorator::CreakingHeart { probability } => {
                if ctx.logs.is_empty() || rng.next_f32() >= probability.0 as f32 {
                    return;
                }
                let shuffled = shuffled(&ctx.logs, rng);
                let target = shuffled.into_iter().find(|pos| {
                    Direction::all()
                        .iter()
                        .all(|side| ctx.holds(&ctx.tables.states.logs, *pos + side.normal()))
                });
                if let Some(pos) = target {
                    ctx.set(pos, ctx.tables.palette.creaking_heart);
                }
            }
            TreeDecorator::Cocoa { probability } => {
                if rng.next_f32() >= probability.0 as f32 || ctx.logs.is_empty() {
                    return;
                }
                let base_y = ctx.logs[0].y;
                for index in 0..ctx.logs.len() {
                    let pos = ctx.logs[index];
                    if pos.y - base_y > 2 {
                        continue;
                    }
                    for (facing, side) in Direction::HORIZONTAL.into_iter().enumerate() {
                        if rng.next_f32() <= 0.25 {
                            let at = pos + side.opposite().normal();
                            if ctx.is_air(at) {
                                let age = rng.next_i32_bound(3) as usize;
                                ctx.set(at, ctx.tables.palette.cocoa[age][facing]);
                            }
                        }
                    }
                }
            }
            TreeDecorator::ShelfMushroom { probability } => {
                if rng.next_f32() >= probability.0 as f32 || ctx.logs.is_empty() {
                    return;
                }
                if ctx.logs[0].y == ctx.logs[ctx.logs.len() - 1].y {
                    place_mushrooms_on_fallen_log(ctx, rng);
                } else {
                    place_mushrooms_on_standing_tree(ctx, rng);
                }
            }
            TreeDecorator::Beehive { probability } => place_beehive(ctx, rng, probability.0 as f32),
            TreeDecorator::AlterGround { provider } => {
                let ground = ctx.lowest_trunk_or_root();
                let Some(first) = ground.first().copied() else {
                    return;
                };
                for pos in ground.iter().filter(|pos| pos.y == first.y) {
                    let pos = *pos;
                    for offset in [(-1, -1), (2, -1), (-1, 2), (2, 2)] {
                        place_circle(ctx, provider, rng, pos + IVec3::new(offset.0, 0, offset.1));
                    }
                    for _ in 0..5 {
                        let placement = rng.next_i32_bound(64);
                        let (x, z) = (placement % 8, placement / 8);
                        if x == 0 || x == 7 || z == 0 || z == 7 {
                            place_circle(ctx, provider, rng, pos + IVec3::new(-3 + x, 0, -3 + z));
                        }
                    }
                }
            }
            TreeDecorator::AttachedToLeaves {
                probability,
                exclusion_radius_xz,
                exclusion_radius_y,
                block_provider: provider,
                required_empty_blocks,
                directions,
            } => {
                let (exclusion_radius_xz, exclusion_radius_y) =
                    (exclusion_radius_xz.0, exclusion_radius_y.0);
                let mut excluded: HashSet<BlockPos> = HashSet::default();
                for leaf in shuffled(&ctx.leaves, rng) {
                    let side = directions[rng.next_i32_bound(directions.len() as i32) as usize];
                    let at = leaf + side.normal();
                    if excluded.contains(&at)
                        || rng.next_f32() >= probability.0 as f32
                        || !(1..=required_empty_blocks.0)
                            .all(|distance| ctx.is_air(leaf + side.normal() * distance))
                    {
                        continue;
                    }
                    for x in -exclusion_radius_xz..=exclusion_radius_xz {
                        for y in -exclusion_radius_y..=exclusion_radius_y {
                            for z in -exclusion_radius_xz..=exclusion_radius_xz {
                                excluded.insert(at + IVec3::new(x, y, z));
                            }
                        }
                    }
                    let state = provider.state(ctx.volume, rng, at);
                    ctx.set(at, state);
                }
            }
            TreeDecorator::PlaceOnGround {
                tries,
                radius,
                height,
                block_state_provider: provider,
            } => {
                let (radius, height) = (radius.0, height.0);
                let ground = ctx.lowest_trunk_or_root();
                let Some(first) = ground.first().copied() else {
                    return;
                };
                let (mut min_x, mut max_x) = (first.x, first.x);
                let (mut min_z, mut max_z) = (first.z, first.z);
                for pos in ground.iter().filter(|pos| pos.y == first.y) {
                    min_x = min_x.min(pos.x);
                    max_x = max_x.max(pos.x);
                    min_z = min_z.min(pos.z);
                    max_z = max_z.max(pos.z);
                }
                let (min, max) = (
                    BlockPos::new(min_x - radius, first.y - height, min_z - radius),
                    BlockPos::new(max_x + radius, first.y + height, max_z + radius),
                );
                for _ in 0..tries.0 {
                    let pos = BlockPos::new(
                        rng.next_int_between_inclusive(min.x, max.x),
                        rng.next_int_between_inclusive(min.y, max.y),
                        rng.next_int_between_inclusive(min.z, max.z),
                    );
                    let above = pos + IVec3::Y;
                    if (ctx.is_air(above) || ctx.holds(&ctx.tables.palette.vines, above))
                        && ctx.holds(&ctx.volume.world().solid_render, pos)
                        && ctx
                            .volume
                            .height(HeightmapName::MotionBlockingNoLeaves, pos.x, pos.z)
                            <= above.y
                    {
                        let state = provider.state(ctx.volume, rng, above);
                        ctx.set(above, state);
                    }
                }
            }
            TreeDecorator::AttachedToLogs {
                probability,
                block_provider: provider,
                directions,
            } => {
                for log in shuffled(&ctx.logs, rng) {
                    let side = directions[rng.next_i32_bound(directions.len() as i32) as usize];
                    let at = log + side.normal();
                    if rng.next_f32() <= probability.0 as f32 && ctx.is_air(at) {
                        let state = provider.state(ctx.volume, rng, at);
                        ctx.set(at, state);
                    }
                }
            }
        }
    }
}

/// The order the vine decorators walk their four sides in.
const VINE_SIDES: [Direction; 4] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
];

fn horizontal_index(direction: Direction) -> usize {
    Direction::HORIZONTAL
        .iter()
        .position(|side| *side == direction)
        .expect("a horizontal direction")
}

fn hang_vine<W: WorldGenVolume>(ctx: &mut DecoratorContext<W>, pos: BlockPos, side: Direction) {
    ctx.place_vine(pos, side);
    let mut cursor = pos + IVec3::NEG_Y;
    let mut left = 4;
    while left > 0 && ctx.is_air(cursor) {
        ctx.place_vine(cursor, side);
        cursor += IVec3::NEG_Y;
        left -= 1;
    }
}

fn hang_moss<W: WorldGenVolume>(
    ctx: &mut DecoratorContext<W>,
    pos: BlockPos,
    rng: &mut WorldgenRandom,
) {
    let mut cursor = pos;
    while ctx.is_air(cursor + IVec3::NEG_Y) && rng.next_f32() >= 0.5 {
        ctx.set(cursor, ctx.tables.palette.pale_hanging_moss[0]);
        cursor += IVec3::NEG_Y;
    }
    ctx.set(cursor, ctx.tables.palette.pale_hanging_moss[1]);
}

fn place_circle<W: WorldGenVolume>(
    ctx: &mut DecoratorContext<W>,
    provider: &StateProvider,
    rng: &mut WorldgenRandom,
    centre: BlockPos,
) {
    for x in -2..=2i32 {
        for z in -2..=2i32 {
            if x.abs() == 2 && z.abs() == 2 {
                continue;
            }
            let column = centre + IVec3::new(x, 0, z);
            for dy in (-3..=2).rev() {
                let cursor = column + IVec3::new(0, dy, 0);
                if let Some(state) = provider.optional_state(ctx.volume, rng, cursor) {
                    ctx.set(cursor, state);
                    break;
                }
                if !ctx.is_air(cursor) && dy < 0 {
                    break;
                }
            }
        }
    }
}

fn place_beehive<W: WorldGenVolume>(
    ctx: &mut DecoratorContext<W>,
    rng: &mut WorldgenRandom,
    probability: f32,
) {
    if ctx.logs.is_empty() || rng.next_f32() >= probability {
        return;
    }
    let hive_y = if let Some(lowest_leaf) = ctx.leaves.first() {
        (lowest_leaf.y - 1).max(ctx.logs[0].y + 1)
    } else {
        (ctx.logs[0].y + 1 + rng.next_i32_bound(3)).min(ctx.logs[ctx.logs.len() - 1].y)
    };
    let mut sides: Vec<BlockPos> = ctx
        .logs
        .iter()
        .filter(|pos| pos.y == hive_y)
        .flat_map(|pos| HIVE_SIDES.iter().map(move |side| *pos + side.normal()))
        .collect();
    if sides.is_empty() {
        return;
    }
    shuffle(&mut sides, rng);
    let hive = sides
        .into_iter()
        .find(|pos| ctx.is_air(*pos) && ctx.is_air(*pos + BEE_NEST_FACING.normal()));
    let Some(hive) = hive else {
        return;
    };
    ctx.set(hive, ctx.tables.palette.bee_nest);
    let occupants = 2 + rng.next_i32_bound(2);
    let bees = (0..occupants)
        .map(|_| BeeOccupant::bee(rng.next_i32_bound(599)))
        .collect();
    ctx.sink.block_entity(GeneratedBlockEntity::Beehive {
        x: hive.x,
        y: hive.y,
        z: hive.z,
        bees,
    });
}

fn place_mushrooms_on_standing_tree<W: WorldGenVolume>(
    ctx: &mut DecoratorContext<W>,
    rng: &mut WorldgenRandom,
) {
    let first = random_horizontal(rng);
    let sides = [first, first.clockwise()];
    let base_y = ctx.logs[0].y;
    for index in 0..ctx.logs.len() {
        let log = ctx.logs[index];
        if !(1..=4).contains(&(log.y - base_y)) {
            continue;
        }
        for side in sides {
            if rng.next_f32() > 0.25 {
                continue;
            }
            let at = log + side.normal();
            if !replaceable_with_shelf_mushroom(ctx, at)
                || ctx.holds(&ctx.tables.palette.shelf_mushrooms, at + IVec3::NEG_Y)
            {
                continue;
            }
            place_shelf_mushroom(ctx, at, side, rng);
            break;
        }
    }
}

fn place_mushrooms_on_fallen_log<W: WorldGenVolume>(
    ctx: &mut DecoratorContext<W>,
    rng: &mut WorldgenRandom,
) {
    let last = ctx.logs[ctx.logs.len() - 1];
    let sides = if ctx.logs[0].x != last.x {
        [Direction::North, Direction::South]
    } else {
        [Direction::East, Direction::West]
    };
    for index in 0..ctx.logs.len() {
        let log = ctx.logs[index];
        for side in sides {
            if rng.next_f32() > 0.25 {
                continue;
            }
            let at = log + side.normal();
            if replaceable_with_shelf_mushroom(ctx, at)
                && !shelf_mushroom_beside(ctx, at)
                && !shelf_mushroom_beside(ctx, log)
            {
                place_shelf_mushroom(ctx, at, side, rng);
            }
        }
    }
}

fn replaceable_with_shelf_mushroom<W: WorldGenVolume>(
    ctx: &DecoratorContext<W>,
    pos: BlockPos,
) -> bool {
    ctx.holds(&ctx.volume.world().replaceable, pos) && !ctx.is_water_or_water_nearby(pos)
}

fn shelf_mushroom_beside<W: WorldGenVolume>(ctx: &DecoratorContext<W>, pos: BlockPos) -> bool {
    Direction::HORIZONTAL
        .iter()
        .any(|side| ctx.holds(&ctx.tables.palette.shelf_mushrooms, pos + side.normal()))
}

fn place_shelf_mushroom<W: WorldGenVolume>(
    ctx: &mut DecoratorContext<W>,
    pos: BlockPos,
    facing: Direction,
    rng: &mut WorldgenRandom,
) {
    let age = rng.next_i32_bound(2) as usize;
    ctx.set(
        pos,
        ctx.tables.palette.shelf_mushroom[age][horizontal_index(facing)],
    );
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    /// Measured against a JDK `HashSet<BlockPos>`: five logs sharing one Y come
    /// out 100, 104, 103, 102, 101, and a 2×2 trunk's y=65 layer comes out
    /// (0,1), (1,0), (1,1), (0,0) — neither is the order they were written in.
    #[test]
    fn the_decorator_lists_start_from_java_s_set_order() {
        let line: Vec<BlockPos> = (0..5).map(|i| BlockPos::new(100 + i, 64, -37)).collect();
        assert_eq!(
            super::java_set_order(&line)
                .iter()
                .map(|pos| pos.x)
                .collect::<Vec<i32>>(),
            vec![100, 104, 103, 102, 101]
        );

        let mut trunk = Vec::new();
        for y in 64..70 {
            for dx in 0..2 {
                for dz in 0..2 {
                    trunk.push(BlockPos::new(dx, y, dz));
                    trunk.push(BlockPos::new(dx, y, dz));
                }
            }
        }
        let ordered = super::java_set_order(&trunk);
        assert_eq!(
            ordered.len(),
            24,
            "the set deduplicates what a placer revisits"
        );
        assert_eq!(
            ordered
                .iter()
                .filter(|pos| pos.y == 65)
                .map(|pos| (pos.x, pos.z))
                .collect::<Vec<(i32, i32)>>(),
            vec![(0, 1), (1, 0), (1, 1), (0, 0)]
        );
    }

    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::super::provider::fake::{AIR, FakeVolume};
    use super::super::trunk::TreeStates;
    use super::*;
    use mcrs_minecraft_worldgen_feature::placer::WorldStates;

    const LOG: VoxelId = VoxelId(1);
    const DIRT: VoxelId = VoxelId(3);
    const WATER: VoxelId = VoxelId(4);
    const VINE: VoxelId = VoxelId(5);
    const MOSS_MID: VoxelId = VoxelId(6);
    const MOSS_TIP: VoxelId = VoxelId(7);
    const NEST: VoxelId = VoxelId(8);
    const HEART: VoxelId = VoxelId(9);
    const GRASS: VoxelId = VoxelId(60);

    fn palette() -> TreePalette {
        let cocoa = std::array::from_fn(|age| {
            std::array::from_fn(|facing| VoxelId(10 + (age * 4 + facing) as u16))
        });
        let shelf_mushroom = std::array::from_fn(|age| {
            std::array::from_fn(|facing| VoxelId(30 + (age * 4 + facing) as u16))
        });
        TreePalette {
            vines: mask_of([VINE, VoxelId(50), VoxelId(51), VoxelId(52), VoxelId(53)]),
            shelf_mushrooms: mask_of(30..38u16),
            vine_side: [VoxelId(50), VoxelId(51), VoxelId(52), VoxelId(53)],
            bee_nest: NEST,
            cocoa,
            shelf_mushroom,
            pale_hanging_moss: [MOSS_MID, MOSS_TIP],
            creaking_heart: HEART,
        }
    }

    fn rng() -> WorldgenRandom {
        WorldgenRandom::new(0x7bee_5eed)
    }

    fn trunk(from: i32, to: i32) -> Vec<BlockPos> {
        (from..=to).map(|y| BlockPos::new(0, y, 0)).collect()
    }

    struct Placed {
        volume: FakeVolume,
        block_entities: Vec<GeneratedBlockEntity>,
        rng: WorldgenRandom,
    }

    /// Runs a decorator over a bare column of logs and leaves, then checks the
    /// draw sequence it is claimed to spend by replaying that sequence from the
    /// same start: equal states mean the claim holds.
    fn decorator(json: &str, provider: Option<StateProvider>) -> CompiledTreeDecorator {
        let proto: TreeDecorator = serde_json::from_str(json).unwrap();
        CompiledTreeDecorator(
            proto
                .map_provider(|_| provider.clone().ok_or("the decorator needs a provider"))
                .unwrap(),
        )
    }

    fn place(
        decorator: &CompiledTreeDecorator,
        logs: Vec<BlockPos>,
        leaves: Vec<BlockPos>,
        world: impl IntoIterator<Item = ((i32, i32, i32), VoxelId)>,
        mut expected: impl FnMut(&mut WorldgenRandom),
    ) -> Placed {
        let tables = TreeTables {
            states: TreeStates {
                logs: mask_of([LOG]),
                ..TreeStates::default()
            },
            palette: palette(),
            leaf_distance: Default::default(),
            survive: Default::default(),
        };
        let mut volume = FakeVolume::with(world);
        volume.world = WorldStates {
            air_states: mask_of([AIR]),
            replaceable: mask_of([AIR]),
            solid_render: mask_of([DIRT, LOG]),
            water_fluid: mask_of([WATER]),
            ..WorldStates::default()
        };
        let mut sink = EntitiesOnly::default();
        let mut actual = rng();
        {
            let mut ctx =
                DecoratorContext::new(&mut volume, &tables, &mut sink, &logs, &leaves, &[]);
            decorator.place(&mut ctx, &mut actual);
        }
        let mut replay = rng();
        expected(&mut replay);
        assert_eq!(actual, replay, "draw sequence");
        Placed {
            volume,
            block_entities: sink.0,
            rng: actual,
        }
    }

    #[test]
    fn trunk_vine_draws_four_per_log_whatever_it_finds() {
        let placed = place(
            &decorator(r#"{"type":"minecraft:trunk_vine"}"#, None),
            trunk(64, 66),
            Vec::new(),
            [((-1, 64, 0), DIRT)],
            |replay| {
                for _ in 0..3 * 4 {
                    replay.next_i32_bound(3);
                }
            },
        );
        assert!(
            placed
                .volume
                .writes
                .iter()
                .all(|(_, state)| (50..54).contains(&state.0)),
            "only vines are written"
        );
    }

    #[test]
    fn leave_vine_hangs_at_most_five_deep() {
        let placed = place(
            &decorator(r#"{"type":"minecraft:leave_vine","probability":1.0}"#, None),
            Vec::new(),
            vec![BlockPos::new(0, 70, 0)],
            [],
            |replay| {
                for _ in 0..4 {
                    replay.next_f32();
                }
            },
        );
        let hanging: Vec<i32> = placed
            .volume
            .writes
            .iter()
            .filter(|((x, _, z), _)| *x == -1 && *z == 0)
            .map(|((_, y, _), _)| *y)
            .collect();
        assert_eq!(hanging, [70, 69, 68, 67, 66]);
    }

    /// The occupant draws sit inside the reference's `ifPresent`, so a
    /// generated column that carried no block entities would spend fewer draws
    /// and shift everything after it.
    #[test]
    fn beehive_always_spends_its_occupant_draws() {
        let mut expected_bees = 0;
        let placed = place(
            &decorator(r#"{"type":"minecraft:beehive","probability":1.0}"#, None),
            trunk(64, 68),
            vec![BlockPos::new(1, 68, 0)],
            [],
            |replay| {
                replay.next_f32();
                replay.next_i32_bound(3);
                replay.next_i32_bound(2);
                let occupants = 2 + replay.next_i32_bound(2);
                expected_bees = occupants;
                for _ in 0..occupants {
                    replay.next_i32_bound(599);
                }
            },
        );

        let [GeneratedBlockEntity::Beehive { x, y, z, bees }] = &placed.block_entities[..] else {
            panic!("one hive, got {:?}", placed.block_entities);
        };
        assert_eq!(bees.len() as i32, expected_bees);
        assert!(bees.iter().all(|bee| (0..599).contains(&bee.ticks_in_hive)
            && bee.min_ticks_in_hive == BEE_MIN_TICKS_IN_HIVE));
        assert_eq!(placed.volume.blocks[&(*x, *y, *z)], NEST);
        assert_eq!(*y, 67, "one below the lowest leaf");
    }

    /// Without leaves the hive height is drawn instead of derived, and the
    /// three sides of a one-log layer shuffle in two draws.
    #[test]
    fn beehive_draws_its_height_when_the_tree_has_no_leaves() {
        place(
            &decorator(r#"{"type":"minecraft:beehive","probability":1.0}"#, None),
            trunk(64, 68),
            Vec::new(),
            [],
            |replay| {
                replay.next_f32();
                replay.next_i32_bound(3);
                replay.next_i32_bound(3);
                replay.next_i32_bound(2);
                let occupants = 2 + replay.next_i32_bound(2);
                for _ in 0..occupants {
                    replay.next_i32_bound(599);
                }
            },
        );
    }

    #[test]
    fn beehive_spends_nothing_on_a_tree_with_no_logs() {
        let placed = place(
            &decorator(r#"{"type":"minecraft:beehive","probability":1.0}"#, None),
            Vec::new(),
            vec![BlockPos::new(0, 70, 0)],
            [],
            |_| {},
        );
        assert!(placed.block_entities.is_empty());
    }

    #[test]
    fn place_on_ground_draws_three_per_try_and_the_provider_only_where_it_writes() {
        let ground: Vec<((i32, i32, i32), VoxelId)> = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| ((x, 63, z), DIRT)))
            .collect();
        let mut volume_writes = 0;
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:place_on_ground","tries":8,"radius":1,"height":1,"block_state_provider":{"type":"minecraft:simple","state":"minecraft:x"}}"#,
                Some(StateProvider::Simple(GRASS)),
            ),
            trunk(64, 66),
            Vec::new(),
            ground,
            |replay| {
                for _ in 0..8 {
                    replay.next_int_between_inclusive(-1, 1);
                    replay.next_int_between_inclusive(63, 65);
                    replay.next_int_between_inclusive(-1, 1);
                }
            },
        );
        for ((_, y, _), state) in &placed.volume.writes {
            assert_eq!(*state, GRASS);
            assert_eq!(*y, 64, "grass sits one above the solid block it found");
            volume_writes += 1;
        }
        assert!(volume_writes > 0, "the tries must land somewhere");
        assert_eq!(placed.rng, placed.rng.clone());
    }

    #[test]
    fn cocoa_draws_the_age_only_where_it_writes() {
        let placed = place(
            &decorator(r#"{"type":"minecraft:cocoa","probability":1.0}"#, None),
            trunk(64, 66),
            Vec::new(),
            [((0, 64, -1), DIRT), ((0, 64, 1), DIRT)],
            |replay| {
                replay.next_f32();
                for log_y in 64..=66 {
                    for facing in 0..4 {
                        if replay.next_f32() <= 0.25 {
                            let blocked = log_y == 64 && (facing == 0 || facing == 2);
                            if !blocked {
                                replay.next_i32_bound(3);
                            }
                        }
                    }
                }
            },
        );
        assert!(
            placed
                .volume
                .writes
                .iter()
                .all(|(_, state)| (10..22).contains(&state.0))
        );
    }

    #[test]
    fn attached_to_logs_draws_a_side_then_a_chance_for_every_log() {
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:attached_to_logs","probability":1.0,"block_provider":{"type":"minecraft:simple","state":"minecraft:x"},"directions":["north","south"]}"#,
                Some(StateProvider::Simple(GRASS)),
            ),
            trunk(64, 66),
            Vec::new(),
            [],
            |replay| {
                for _ in 0..2 {
                    replay.next_i32_bound(3);
                }
                for _ in 0..3 {
                    replay.next_i32_bound(2);
                    replay.next_f32();
                }
            },
        );
        assert_eq!(placed.volume.writes.len(), 3);
    }

    #[test]
    fn attached_to_leaves_skips_the_chance_draw_inside_the_exclusion_box() {
        let leaves: Vec<BlockPos> = (0..3).map(|z| BlockPos::new(0, 70, z)).collect();
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:attached_to_leaves","probability":1.0,"exclusion_radius_xz":2,"exclusion_radius_y":2,"block_provider":{"type":"minecraft:simple","state":"minecraft:x"},"required_empty_blocks":1,"directions":["down"]}"#,
                Some(StateProvider::Simple(GRASS)),
            ),
            Vec::new(),
            leaves,
            [],
            |replay| {
                for _ in 0..2 {
                    replay.next_i32_bound(3);
                }
                replay.next_i32_bound(1);
                replay.next_f32();
                for _ in 0..2 {
                    replay.next_i32_bound(1);
                }
            },
        );
        assert_eq!(
            placed.volume.writes.len(),
            1,
            "the first leaf blacklists the box the other two would land in"
        );
    }

    #[test]
    fn shelf_mushroom_picks_two_perpendicular_sides_once_per_tree() {
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:shelf_mushroom","probability":1.0}"#,
                None,
            ),
            trunk(64, 67),
            Vec::new(),
            [],
            |replay| {
                replay.next_f32();
                replay.next_i32_bound(4);
                for _ in 0..3 {
                    for _ in 0..2 {
                        if replay.next_f32() <= 0.25 {
                            replay.next_i32_bound(2);
                            break;
                        }
                    }
                }
            },
        );
        assert!(
            placed
                .volume
                .writes
                .iter()
                .all(|((_, y, _), state)| (64..68).contains(y) && (30..38).contains(&state.0))
        );
    }

    #[test]
    fn creaking_heart_shuffles_then_takes_the_first_fully_buried_log() {
        let buried: Vec<((i32, i32, i32), VoxelId)> = [
            (0, 64, 0),
            (0, 66, 0),
            (1, 65, 0),
            (-1, 65, 0),
            (0, 65, 1),
            (0, 65, -1),
        ]
        .into_iter()
        .map(|pos| (pos, LOG))
        .collect();
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:creaking_heart","probability":1.0}"#,
                None,
            ),
            trunk(64, 66),
            Vec::new(),
            buried,
            |replay| {
                replay.next_f32();
                for _ in 0..2 {
                    replay.next_i32_bound(3);
                }
            },
        );
        assert_eq!(placed.volume.writes, [((0, 65, 0), HEART)]);
    }

    #[test]
    fn alter_ground_draws_five_placements_after_its_four_circles() {
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:alter_ground","provider":{"type":"minecraft:simple","state":"minecraft:x"}}"#,
                Some(StateProvider::Simple(DIRT)),
            ),
            trunk(64, 66),
            Vec::new(),
            [],
            |replay| {
                for _ in 0..5 {
                    replay.next_i32_bound(64);
                }
            },
        );
        assert!(
            placed.volume.writes.len() >= 4 * 21 && placed.volume.writes.len() % 21 == 0,
            "a simple provider never declines, so every circle writes all 21 of its cells: {}",
            placed.volume.writes.len()
        );
    }

    #[test]
    fn pale_moss_shuffles_the_logs_then_walks_both_lists() {
        let placed = place(
            &decorator(
                r#"{"type":"minecraft:pale_moss","leaves_probability":1.0,"trunk_probability":0.0,"ground_probability":0.0}"#,
                None,
            ),
            trunk(64, 66),
            vec![BlockPos::new(0, 67, 0)],
            [((0, 65, 0), LOG)],
            |replay| {
                replay.next_i32_bound(3);
                replay.next_i32_bound(2);
                replay.next_f32();
                for _ in 0..3 {
                    replay.next_f32();
                }
                replay.next_f32();
            },
        );
        assert_eq!(
            placed.volume.writes,
            [((0, 66, 0), MOSS_TIP)],
            "the log below stops the hanger at once"
        );
    }

    #[test]
    fn the_shuffle_is_a_descending_fisher_yates() {
        let positions = trunk(0, 4);
        let mut actual = rng();
        let shuffled = shuffled(&positions, &mut actual);
        let mut replay = rng();
        for length in (2..=5).rev() {
            replay.next_i32_bound(length);
        }
        assert_eq!(actual, replay, "one draw short of the length");
        let mut sorted = shuffled.clone();
        sorted.sort_by_key(|pos| pos.y);
        assert_eq!(sorted, positions, "a shuffle keeps every element");
    }
}
