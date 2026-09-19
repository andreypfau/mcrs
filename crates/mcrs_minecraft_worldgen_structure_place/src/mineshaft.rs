use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_chunk::{Blocks, BlocksMut, Volume, VoxelId};
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_core::{BlockPos, BoundingBox, ResourceLocation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, StateQuery, states_of,
};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{BiomeMask, StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::{GeneratedEntity, chest_minecart};
use mcrs_minecraft_worldgen_structure::MineshaftType;
use mcrs_minecraft_worldgen_structure::piece::{MineshaftKind, MineshaftPiece};

use crate::canvas::{PieceCanvas, replaceable_by_structures};
use crate::{Oriented, block_mask, state};

pub const ABANDONED_MINESHAFT_LOOT: &str = "minecraft:chests/abandoned_mineshaft";

const MAX_PILLAR_HEIGHT: i32 = 20;
const MAX_CHAIN_HEIGHT: i32 = 50;

/// The blocks whose class is `FallingBlock`, which a chain never hangs from.
const FALLING_BLOCKS: [&str; 23] = [
    "minecraft:sand",
    "minecraft:red_sand",
    "minecraft:gravel",
    "minecraft:anvil",
    "minecraft:chipped_anvil",
    "minecraft:damaged_anvil",
    "minecraft:dragon_egg",
    "minecraft:white_concrete_powder",
    "minecraft:orange_concrete_powder",
    "minecraft:magenta_concrete_powder",
    "minecraft:light_blue_concrete_powder",
    "minecraft:yellow_concrete_powder",
    "minecraft:lime_concrete_powder",
    "minecraft:pink_concrete_powder",
    "minecraft:gray_concrete_powder",
    "minecraft:light_gray_concrete_powder",
    "minecraft:cyan_concrete_powder",
    "minecraft:purple_concrete_powder",
    "minecraft:blue_concrete_powder",
    "minecraft:brown_concrete_powder",
    "minecraft:green_concrete_powder",
    "minecraft:red_concrete_powder",
    "minecraft:black_concrete_powder",
];

#[derive(Clone, Debug)]
pub struct MineshaftBlocks {
    pub cave_air: Oriented,
    pub planks: Oriented,
    pub wood: VoxelId,
    pub fence: VoxelId,
    pub fence_west: Oriented,
    pub fence_east: Oriented,
    pub chain: VoxelId,
    pub cobweb: Oriented,
    pub rail_ns: Oriented,
    pub rail_ew: Oriented,
    pub torch_south: Oriented,
    pub torch_north: Oriented,
    pub spawner: VoxelId,
    /// `canBeReplaced`, negated: the timber a `placeBlock` never overwrites.
    pub timber: StateMask,
    pub replaceable_by_structures: StateMask,
    /// `canHangChainBelow`: `canSupportCenter(DOWN)` on a block that does not fall.
    pub chain_support: StateMask,
    pub blocking: BiomeMask,
}

impl MineshaftBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
        mineshaft_type: MineshaftType,
        blocking: BiomeMask,
    ) -> Result<Self, FeatureCompileError> {
        let (log, planks, fence) = match mineshaft_type {
            MineshaftType::Normal => (
                "minecraft:oak_log",
                "minecraft:oak_planks",
                "minecraft:oak_fence",
            ),
            MineshaftType::Mesa => (
                "minecraft:dark_oak_log",
                "minecraft:dark_oak_planks",
                "minecraft:dark_oak_fence",
            ),
        };
        let oriented = |block: &str, properties: &[(&str, &str)]| {
            Ok(Oriented::of(world, state(blocks, block, properties)?))
        };
        let unstable =
            ResourceLocation::parse("minecraft:unstable_bottom_center").expect("a literal id");
        let unstable = states_of(blocks, StateQuery::BlockTag(&unstable))?;
        let mut chain_support = FixedBitSet::clone(&world.center_down);
        chain_support.difference_with(&unstable);
        chain_support.difference_with(&*block_mask(blocks, &FALLING_BLOCKS)?);
        Ok(MineshaftBlocks {
            cave_air: oriented("minecraft:cave_air", &[])?,
            planks: oriented(planks, &[])?,
            wood: state(blocks, log, &[])?,
            fence: state(blocks, fence, &[])?,
            fence_west: oriented(fence, &[("west", "true")])?,
            fence_east: oriented(fence, &[("east", "true")])?,
            chain: state(blocks, "minecraft:iron_chain", &[])?,
            cobweb: oriented("minecraft:cobweb", &[])?,
            rail_ns: oriented("minecraft:rail", &[("shape", "north_south")])?,
            rail_ew: oriented("minecraft:rail", &[("shape", "east_west")])?,
            torch_south: oriented("minecraft:wall_torch", &[("facing", "south")])?,
            torch_north: oriented("minecraft:wall_torch", &[("facing", "north")])?,
            spawner: state(blocks, "minecraft:spawner", &[])?,
            timber: block_mask(blocks, &[planks, log, fence, "minecraft:iron_chain"])?,
            replaceable_by_structures: replaceable_by_structures(blocks, world)?,
            chain_support: chain_support.into(),
            blocking,
        })
    }
}

/// The volume as `MineShaftPiece.canBeReplaced` sees it: `placeBlock` leaves
/// timber alone, while `setBlock` on the level writes through to `inner`.
struct Timbered<'a, W> {
    inner: &'a mut W,
    timber: &'a StateMask,
}

impl<W: WorldGenVolume> Volume for Timbered<'_, W> {
    fn min(&self) -> BlockPos {
        self.inner.min()
    }

    fn max(&self) -> BlockPos {
        self.inner.max()
    }
}

impl<W: WorldGenVolume> Blocks for Timbered<'_, W> {
    fn get(&self, p: BlockPos) -> VoxelId {
        self.inner.get(p)
    }
}

impl<W: WorldGenVolume> BlocksMut for Timbered<'_, W> {
    fn set(&mut self, p: BlockPos, id: VoxelId) {
        if !self.inner.holds(self.timber, p) {
            self.inner.set(p, id);
        }
    }
}

impl<W: WorldGenVolume> WorldGenVolume for Timbered<'_, W> {
    fn world(&self) -> &WorldStates {
        self.inner.world()
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32 {
        self.inner.height(kind, x, z)
    }

    fn biome(&self, p: BlockPos) -> u32 {
        self.inner.biome(p)
    }

    fn extent(&self) -> HeightContext {
        self.inner.extent()
    }

    fn would_survive(&self, state: VoxelId, p: BlockPos) -> bool {
        self.inner.would_survive(state, p)
    }
}

type Canvas<'a, W> = PieceCanvas<'a, Timbered<'a, W>>;

/// Each type's `postProcess` for one column.
pub fn paint_mineshaft<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    piece: &MineshaftPiece,
    region: &mut W,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
    clip: BoundingBox,
    rng: &mut XoroshiroRandom,
) {
    if in_invalid_location(b, region, piece.bounds, clip) {
        return;
    }
    let mut timbered = Timbered {
        inner: region,
        timber: &b.timber,
    };
    let mut c = PieceCanvas {
        volume: &mut timbered,
        entities,
        bounds: piece.bounds,
        orientation: piece.orientation(),
        clip,
    };
    match &piece.kind {
        MineshaftKind::Room { entrances } => room(b, &mut c, entrances),
        MineshaftKind::Corridor {
            has_rails,
            spider_corridor,
            num_sections,
        } => corridor(
            b,
            &mut c,
            spawns,
            rng,
            *has_rails,
            *spider_corridor,
            *num_sections,
        ),
        MineshaftKind::Crossing { two_floored } => crossing(b, &mut c, *two_floored),
        MineshaftKind::Stairs => stairs(b, &mut c),
    }
}

/// `isInInvalidLocation`: a blocking biome at the centre of the piece's box
/// grown by one and clipped, or liquid anywhere on that box's shell.
fn in_invalid_location<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    region: &W,
    bounds: BoundingBox,
    clip: BoundingBox,
) -> bool {
    let min = (*bounds.min - IVec3::ONE).max(*clip.min);
    let max = (*bounds.max + IVec3::ONE).min(*clip.max);
    let centre = (min + max) / 2;
    if b.blocking.contains(region.biome(centre.into()) as usize) {
        return true;
    }
    let liquid = |x, y, z| region.holds(&region.world().any_fluid, BlockPos::new(x, y, z));
    for x in min.x..=max.x {
        for z in min.z..=max.z {
            if liquid(x, min.y, z) || liquid(x, max.y, z) {
                return true;
            }
        }
    }
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            if liquid(x, y, min.z) || liquid(x, y, max.z) {
                return true;
            }
        }
    }
    for z in min.z..=max.z {
        for y in min.y..=max.y {
            if liquid(min.x, y, z) || liquid(max.x, y, z) {
                return true;
            }
        }
    }
    false
}

fn air<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    min: [i32; 3],
    max: [i32; 3],
) {
    c.generate_box(min, max, &b.cave_air, &b.cave_air, false);
}

fn sturdy_up<W: WorldGenVolume>(c: &Canvas<'_, W>, pos: BlockPos) -> bool {
    c.volume.inner.holds(&c.volume.world().sturdy_up, pos)
}

/// `setPlanksBlock`: planks under an interior cell whose block is not sturdy.
fn set_planks<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    x: i32,
    y: i32,
    z: i32,
) {
    if !c.is_interior(x, y, z) {
        return;
    }
    let pos = c.world_pos(x, y, z);
    if !sturdy_up(c, pos) {
        c.volume.inner.set(pos, b.planks.unoriented());
    }
}

fn room<W: WorldGenVolume>(b: &MineshaftBlocks, c: &mut Canvas<'_, W>, entrances: &[BoundingBox]) {
    let (min, max) = (*c.bounds.min, *c.bounds.max);
    air(
        b,
        c,
        [min.x, min.y + 1, min.z],
        [max.x, (min.y + 3).min(max.y), max.z],
    );
    for entrance in entrances {
        let (emin, emax) = (*entrance.min, *entrance.max);
        air(b, c, [emin.x, emax.y - 2, emin.z], [emax.x, emax.y, emax.z]);
    }
    c.upper_half_sphere(
        [min.x, min.y + 4, min.z],
        [max.x, max.y, max.z],
        &b.cave_air,
        false,
    );
}

fn stairs<W: WorldGenVolume>(b: &MineshaftBlocks, c: &mut Canvas<'_, W>) {
    air(b, c, [0, 5, 0], [2, 7, 1]);
    air(b, c, [0, 0, 7], [2, 2, 8]);
    for i in 0..5 {
        let lowered = if i < 4 { 1 } else { 0 };
        air(b, c, [0, 5 - i - lowered, 2 + i], [2, 7 - i, 2 + i]);
    }
}

fn crossing<W: WorldGenVolume>(b: &MineshaftBlocks, c: &mut Canvas<'_, W>, two_floored: bool) {
    let (min, max) = (*c.bounds.min, *c.bounds.max);
    if two_floored {
        air(
            b,
            c,
            [min.x + 1, min.y, min.z],
            [max.x - 1, min.y + 3 - 1, max.z],
        );
        air(
            b,
            c,
            [min.x, min.y, min.z + 1],
            [max.x, min.y + 3 - 1, max.z - 1],
        );
        air(
            b,
            c,
            [min.x + 1, max.y - 2, min.z],
            [max.x - 1, max.y, max.z],
        );
        air(
            b,
            c,
            [min.x, max.y - 2, min.z + 1],
            [max.x, max.y, max.z - 1],
        );
        air(
            b,
            c,
            [min.x + 1, min.y + 3, min.z + 1],
            [max.x - 1, min.y + 3, max.z - 1],
        );
    } else {
        air(b, c, [min.x + 1, min.y, min.z], [max.x - 1, max.y, max.z]);
        air(b, c, [min.x, min.y, min.z + 1], [max.x, max.y, max.z - 1]);
    }
    for (x, z) in [
        (min.x + 1, min.z + 1),
        (min.x + 1, max.z - 1),
        (max.x - 1, min.z + 1),
        (max.x - 1, max.z - 1),
    ] {
        if !is_air(c, x, max.y + 1, z) {
            c.generate_box([x, min.y, z], [x, max.y, z], &b.planks, &b.cave_air, false);
        }
    }
    for x in min.x..=max.x {
        for z in min.z..=max.z {
            set_planks(b, c, x, min.y - 1, z);
        }
    }
}

fn is_air<W: WorldGenVolume>(c: &Canvas<'_, W>, x: i32, y: i32, z: i32) -> bool {
    let state = c.get(x, y, z);
    c.volume.world().air_states.contains(state.0 as usize)
}

fn corridor<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    spawns: &mut Vec<GeneratedEntity>,
    rng: &mut XoroshiroRandom,
    has_rails: bool,
    spider_corridor: bool,
    num_sections: i32,
) {
    let length = num_sections * 5 - 1;
    air(b, c, [0, 0, 0], [2, 1, length]);
    c.generate_maybe_box(
        rng,
        0.8,
        [0, 2, 0],
        [2, 2, length],
        &b.cave_air,
        &b.cave_air,
        false,
        false,
    );
    if spider_corridor {
        c.generate_maybe_box(
            rng,
            0.6,
            [0, 0, 0],
            [2, 1, length],
            &b.cobweb,
            &b.cave_air,
            false,
            true,
        );
    }
    let mut placed_spider = false;
    for section in 0..num_sections {
        let z = 2 + section * 5;
        place_support(b, c, rng, z);
        for (probability, dz) in [(0.1, -1), (0.1, 1), (0.05, -2), (0.05, 2)] {
            maybe_cobweb(b, c, rng, probability, 0, 2, z + dz);
            maybe_cobweb(b, c, rng, probability, 2, 2, z + dz);
        }
        if rng.next_i32_bound(100) == 0 {
            create_minecart(b, c, spawns, rng, 2, 0, z - 1);
        }
        if rng.next_i32_bound(100) == 0 {
            create_minecart(b, c, spawns, rng, 0, 0, z + 1);
        }
        if spider_corridor && !placed_spider {
            let spawner_z = z - 1 + rng.next_i32_bound(3);
            let pos = c.world_pos(1, 0, spawner_z);
            if c.clip.is_inside(pos) && c.is_interior(1, 0, spawner_z) {
                placed_spider = true;
                c.volume.inner.set(pos, b.spawner);
                c.entities.push(GeneratedBlockEntity::mob_spawner(
                    pos,
                    "minecraft:cave_spider",
                ));
            }
        }
    }
    for x in 0..=2 {
        for z in 0..=length {
            set_planks(b, c, x, -1, z);
        }
    }
    double_support(b, c, 2);
    if num_sections > 1 {
        double_support(b, c, length - 2);
    }
    if has_rails {
        for z in 0..=length {
            let floor = c.get(1, -1, z);
            let world = c.volume.world();
            if !world.air_states.contains(floor.0 as usize)
                && world.solid_render.contains(floor.0 as usize)
            {
                let probability = if c.is_interior(1, 0, z) { 0.7 } else { 0.9 };
                c.maybe_generate_block(rng, probability, 1, 0, z, &b.rail_ns);
            }
        }
    }
}

/// `placeSupport` over the corridor's full width and height.
fn place_support<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    rng: &mut XoroshiroRandom,
    z: i32,
) {
    if (0..=2).any(|x| is_air(c, x, 3, z)) {
        return;
    }
    c.generate_box([0, 0, z], [0, 1, z], &b.fence_west, &b.cave_air, false);
    c.generate_box([2, 0, z], [2, 1, z], &b.fence_east, &b.cave_air, false);
    if rng.next_i32_bound(4) == 0 {
        c.generate_box([0, 2, z], [0, 2, z], &b.planks, &b.cave_air, false);
        c.generate_box([2, 2, z], [2, 2, z], &b.planks, &b.cave_air, false);
    } else {
        c.generate_box([0, 2, z], [2, 2, z], &b.planks, &b.cave_air, false);
        c.maybe_generate_block(rng, 0.05, 1, 2, z - 1, &b.torch_south);
        c.maybe_generate_block(rng, 0.05, 1, 2, z + 1, &b.torch_north);
    }
}

fn maybe_cobweb<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    rng: &mut XoroshiroRandom,
    probability: f32,
    x: i32,
    y: i32,
    z: i32,
) {
    if c.is_interior(x, y, z) && rng.next_f32() < probability && sturdy_neighbours(c, x, y, z, 2) {
        c.place(&b.cobweb, x, y, z);
    }
}

/// `hasSturdyNeighbours`: `count` of the six neighbours inside the clip with a
/// sturdy face towards the cell.
fn sturdy_neighbours<W: WorldGenVolume>(
    c: &Canvas<'_, W>,
    x: i32,
    y: i32,
    z: i32,
    count: usize,
) -> bool {
    let pos = c.world_pos(x, y, z);
    let sides = [
        IVec3::NEG_Y,
        IVec3::Y,
        IVec3::NEG_Z,
        IVec3::Z,
        IVec3::NEG_X,
        IVec3::X,
    ];
    sides
        .into_iter()
        .filter(|side| {
            let neighbour = BlockPos::from(*pos + *side);
            c.clip.is_inside(neighbour) && sturdy_up(c, neighbour)
        })
        .count()
        >= count
}

/// The corridor's `createChest`: a rail and a chest minecart on it, where the
/// cell is air over something that is not.
fn create_minecart<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    spawns: &mut Vec<GeneratedEntity>,
    rng: &mut XoroshiroRandom,
    x: i32,
    y: i32,
    z: i32,
) {
    let pos = c.world_pos(x, y, z);
    let below = BlockPos::from(*pos - IVec3::Y);
    let volume = &*c.volume;
    if !c.clip.is_inside(pos) || !volume.is_air(pos) || volume.is_air(below) {
        return;
    }
    let rail = if rng.next_bool() {
        &b.rail_ns
    } else {
        &b.rail_ew
    };
    c.place(rail, x, y, z);
    spawns.push(chest_minecart(
        pos,
        ABANDONED_MINESHAFT_LOOT.to_owned(),
        rng,
    ));
}

/// `placeDoubleLowerOrUpperSupport`: both corridor edges at `z`, where the
/// floor is planks.
fn double_support<W: WorldGenVolume>(b: &MineshaftBlocks, c: &mut Canvas<'_, W>, z: i32) {
    let planks = c.volume.world().block_of(b.planks.unoriented());
    for x in [0, 2] {
        if c.volume.world().block_of(c.get(x, -1, z)) == planks {
            pillar_down_or_chain_up(b, c, x, -1, z);
        }
    }
}

/// `fillPillarDownOrChainUp`: a wood pillar down to the first sturdy block
/// within twenty, or a fence and a chain up to the first support within fifty,
/// whichever the alternating walk finds first.
fn pillar_down_or_chain_up<W: WorldGenVolume>(
    b: &MineshaftBlocks,
    c: &mut Canvas<'_, W>,
    x: i32,
    y: i32,
    z: i32,
) {
    let pos = c.world_pos(x, y, z);
    if !c.clip.is_inside(pos) {
        return;
    }
    let extent = c.volume.extent();
    let (floor, ceiling) = (extent.min_y + 1, extent.min_y + extent.depth - 1);
    let world_y = pos.y;
    let at = |dy: i32| BlockPos::new(pos.x, dy, pos.z);
    let inner = &mut *c.volume.inner;
    let mut distance = 1;
    let mut check_below = true;
    let mut check_above = true;
    while check_below || check_above {
        if check_below {
            let probe = at(world_y - distance);
            let empty = inner.holds(&b.replaceable_by_structures, probe)
                && !inner.holds(&inner.world().lava_states, probe);
            if !empty && inner.holds(&inner.world().sturdy_up, probe) {
                for fill_y in world_y - distance + 1..world_y {
                    inner.set(at(fill_y), b.wood);
                }
                return;
            }
            check_below = distance <= MAX_PILLAR_HEIGHT && empty && probe.y > floor;
        }
        if check_above {
            let probe = at(world_y + distance);
            let empty = inner.holds(&b.replaceable_by_structures, probe);
            if !empty && inner.holds(&b.chain_support, probe) {
                inner.set(at(world_y + 1), b.fence);
                for fill_y in world_y + 2..world_y + distance {
                    inner.set(at(fill_y), b.chain);
                }
                return;
            }
            check_above = distance <= MAX_CHAIN_HEIGHT && empty && probe.y < ceiling;
        }
        distance += 1;
    }
}
