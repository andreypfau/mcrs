use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_structure::orient::{Orientation, world_pos};
use mcrs_minecraft_worldgen_structure::piece::DesertPyramidPiece;

use crate::canvas::{ChestStates, PieceCanvas, replaceable_by_structures};
use crate::{Oriented, state};

pub const DESERT_PYRAMID_LOOT: &str = "minecraft:chests/desert_pyramid";
pub const DESERT_PYRAMID_ARCHAEOLOGY_LOOT: &str = "minecraft:archaeology/desert_pyramid";

/// `DesertPyramidPiece.addCellar`'s room centre, piece-local.
const CELLAR_CENTRE: IVec3 = IVec3::new(16, -4, 13);

#[derive(Clone, Debug)]
pub struct DesertPyramidBlocks {
    pub sandstone: Oriented,
    pub cut_sandstone: Oriented,
    pub chiseled_sandstone: Oriented,
    pub sandstone_slab: Oriented,
    pub stairs_north: Oriented,
    pub stairs_south: Oriented,
    pub stairs_east: Oriented,
    pub stairs_west: Oriented,
    pub air: Oriented,
    pub orange_terracotta: Oriented,
    pub blue_terracotta: Oriented,
    pub tnt: Oriented,
    pub stone_pressure_plate: Oriented,
    pub sand: Oriented,
    pub suspicious_sand: VoxelId,
    pub chest: ChestStates,
    pub replaceable_by_structures: StateMask,
    pub world_seed: i64,
}

impl DesertPyramidBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
        world_seed: i64,
    ) -> Result<Self, FeatureCompileError> {
        let oriented = |block: &str| Ok(Oriented::of(world, state(blocks, block, &[])?));
        let stairs = |facing: &str| {
            Ok(Oriented::of(
                world,
                state(blocks, "minecraft:sandstone_stairs", &[("facing", facing)])?,
            ))
        };
        Ok(DesertPyramidBlocks {
            sandstone: oriented("minecraft:sandstone")?,
            cut_sandstone: oriented("minecraft:cut_sandstone")?,
            chiseled_sandstone: oriented("minecraft:chiseled_sandstone")?,
            sandstone_slab: oriented("minecraft:sandstone_slab")?,
            stairs_north: stairs("north")?,
            stairs_south: stairs("south")?,
            stairs_east: stairs("east")?,
            stairs_west: stairs("west")?,
            air: oriented("minecraft:air")?,
            orange_terracotta: oriented("minecraft:orange_terracotta")?,
            blue_terracotta: oriented("minecraft:blue_terracotta")?,
            tnt: oriented("minecraft:tnt")?,
            stone_pressure_plate: oriented("minecraft:stone_pressure_plate")?,
            sand: oriented("minecraft:sand")?,
            suspicious_sand: state(blocks, "minecraft:suspicious_sand", &[])?,
            chest: ChestStates::compile(blocks)?,
            replaceable_by_structures: replaceable_by_structures(blocks, world)?,
            world_seed,
        })
    }
}

/// `DesertPyramidPiece.postProcess` for one column, `bounds` already sunk to
/// its floor. The first draw is the reference's ground-height offset, spent
/// here and read from the start chunk's stream by the caller.
pub fn paint_desert_pyramid<W: WorldGenVolume, R: Random>(
    b: &DesertPyramidBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    const W: i32 = DesertPyramidPiece::WIDTH;
    const D: i32 = DesertPyramidPiece::DEPTH;
    rng.next_i32_bound(3);

    c.generate_box([0, -4, 0], [W - 1, 0, D - 1], &b.sandstone, &b.sandstone, false);
    for i in 1..=9 {
        c.generate_box(
            [i, i, i],
            [W - 1 - i, i, D - 1 - i],
            &b.sandstone,
            &b.sandstone,
            false,
        );
        c.generate_box(
            [i + 1, i, i + 1],
            [W - 2 - i, i, D - 2 - i],
            &b.air,
            &b.air,
            false,
        );
    }
    let sandstone = b.sandstone.unoriented();
    for x in 0..W {
        for z in 0..D {
            c.fill_column_down(&b.replaceable_by_structures, sandstone, x, -5, z);
        }
    }

    c.generate_box([0, 0, 0], [4, 9, 4], &b.sandstone, &b.air, false);
    c.generate_box([1, 10, 1], [3, 10, 3], &b.sandstone, &b.sandstone, false);
    c.place(&b.stairs_north, 2, 10, 0);
    c.place(&b.stairs_south, 2, 10, 4);
    c.place(&b.stairs_east, 0, 10, 2);
    c.place(&b.stairs_west, 4, 10, 2);
    c.generate_box([W - 5, 0, 0], [W - 1, 9, 4], &b.sandstone, &b.air, false);
    c.generate_box([W - 4, 10, 1], [W - 2, 10, 3], &b.sandstone, &b.sandstone, false);
    c.place(&b.stairs_north, W - 3, 10, 0);
    c.place(&b.stairs_south, W - 3, 10, 4);
    c.place(&b.stairs_east, W - 5, 10, 2);
    c.place(&b.stairs_west, W - 1, 10, 2);
    c.generate_box([8, 0, 0], [12, 4, 4], &b.sandstone, &b.air, false);
    c.generate_box([9, 1, 0], [11, 3, 4], &b.air, &b.air, false);
    c.place(&b.cut_sandstone, 9, 1, 1);
    c.place(&b.cut_sandstone, 9, 2, 1);
    c.place(&b.cut_sandstone, 9, 3, 1);
    c.place(&b.cut_sandstone, 10, 3, 1);
    c.place(&b.cut_sandstone, 11, 3, 1);
    c.place(&b.cut_sandstone, 11, 2, 1);
    c.place(&b.cut_sandstone, 11, 1, 1);
    c.generate_box([4, 1, 1], [8, 3, 3], &b.sandstone, &b.air, false);
    c.generate_box([4, 1, 2], [8, 2, 2], &b.air, &b.air, false);
    c.generate_box([12, 1, 1], [16, 3, 3], &b.sandstone, &b.air, false);
    c.generate_box([12, 1, 2], [16, 2, 2], &b.air, &b.air, false);
    c.generate_box([5, 4, 5], [W - 6, 4, D - 6], &b.sandstone, &b.sandstone, false);
    c.generate_box([9, 4, 9], [11, 4, 11], &b.air, &b.air, false);
    c.generate_box([8, 1, 8], [8, 3, 8], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box([12, 1, 8], [12, 3, 8], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box([8, 1, 12], [8, 3, 12], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box([12, 1, 12], [12, 3, 12], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box([1, 1, 5], [4, 4, 11], &b.sandstone, &b.sandstone, false);
    c.generate_box([W - 5, 1, 5], [W - 2, 4, 11], &b.sandstone, &b.sandstone, false);
    c.generate_box([6, 7, 9], [6, 7, 11], &b.sandstone, &b.sandstone, false);
    c.generate_box([W - 7, 7, 9], [W - 7, 7, 11], &b.sandstone, &b.sandstone, false);
    c.generate_box([5, 5, 9], [5, 7, 11], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box([W - 6, 5, 9], [W - 6, 7, 11], &b.cut_sandstone, &b.cut_sandstone, false);
    c.place(&b.air, 5, 5, 10);
    c.place(&b.air, 5, 6, 10);
    c.place(&b.air, 6, 6, 10);
    c.place(&b.air, W - 6, 5, 10);
    c.place(&b.air, W - 6, 6, 10);
    c.place(&b.air, W - 7, 6, 10);
    c.generate_box([2, 4, 4], [2, 6, 4], &b.air, &b.air, false);
    c.generate_box([W - 3, 4, 4], [W - 3, 6, 4], &b.air, &b.air, false);
    c.place(&b.stairs_north, 2, 4, 5);
    c.place(&b.stairs_north, 2, 3, 4);
    c.place(&b.stairs_north, W - 3, 4, 5);
    c.place(&b.stairs_north, W - 3, 3, 4);
    c.generate_box([1, 1, 3], [2, 2, 3], &b.sandstone, &b.sandstone, false);
    c.generate_box([W - 3, 1, 3], [W - 2, 2, 3], &b.sandstone, &b.sandstone, false);
    c.place(&b.sandstone, 1, 1, 2);
    c.place(&b.sandstone, W - 2, 1, 2);
    c.place(&b.sandstone_slab, 1, 2, 2);
    c.place(&b.sandstone_slab, W - 2, 2, 2);
    c.place(&b.stairs_west, 2, 1, 2);
    c.place(&b.stairs_east, W - 3, 1, 2);
    c.generate_box([4, 3, 5], [4, 3, 17], &b.sandstone, &b.sandstone, false);
    c.generate_box([W - 5, 3, 5], [W - 5, 3, 17], &b.sandstone, &b.sandstone, false);
    c.generate_box([3, 1, 5], [4, 2, 16], &b.air, &b.air, false);
    c.generate_box([W - 6, 1, 5], [W - 5, 2, 16], &b.air, &b.air, false);
    for z in (5..=17).step_by(2) {
        c.place(&b.cut_sandstone, 4, 1, z);
        c.place(&b.chiseled_sandstone, 4, 2, z);
        c.place(&b.cut_sandstone, W - 5, 1, z);
        c.place(&b.chiseled_sandstone, W - 5, 2, z);
    }

    c.place(&b.orange_terracotta, 10, 0, 7);
    c.place(&b.orange_terracotta, 10, 0, 8);
    c.place(&b.orange_terracotta, 9, 0, 9);
    c.place(&b.orange_terracotta, 11, 0, 9);
    c.place(&b.orange_terracotta, 8, 0, 10);
    c.place(&b.orange_terracotta, 12, 0, 10);
    c.place(&b.orange_terracotta, 7, 0, 10);
    c.place(&b.orange_terracotta, 13, 0, 10);
    c.place(&b.orange_terracotta, 9, 0, 11);
    c.place(&b.orange_terracotta, 11, 0, 11);
    c.place(&b.orange_terracotta, 10, 0, 12);
    c.place(&b.orange_terracotta, 10, 0, 13);
    c.place(&b.blue_terracotta, 10, 0, 10);

    for x in [0, W - 1] {
        c.place(&b.cut_sandstone, x, 2, 1);
        c.place(&b.orange_terracotta, x, 2, 2);
        c.place(&b.cut_sandstone, x, 2, 3);
        c.place(&b.cut_sandstone, x, 3, 1);
        c.place(&b.orange_terracotta, x, 3, 2);
        c.place(&b.cut_sandstone, x, 3, 3);
        c.place(&b.orange_terracotta, x, 4, 1);
        c.place(&b.chiseled_sandstone, x, 4, 2);
        c.place(&b.orange_terracotta, x, 4, 3);
        c.place(&b.cut_sandstone, x, 5, 1);
        c.place(&b.orange_terracotta, x, 5, 2);
        c.place(&b.cut_sandstone, x, 5, 3);
        c.place(&b.orange_terracotta, x, 6, 1);
        c.place(&b.chiseled_sandstone, x, 6, 2);
        c.place(&b.orange_terracotta, x, 6, 3);
        c.place(&b.orange_terracotta, x, 7, 1);
        c.place(&b.orange_terracotta, x, 7, 2);
        c.place(&b.orange_terracotta, x, 7, 3);
        c.place(&b.cut_sandstone, x, 8, 1);
        c.place(&b.cut_sandstone, x, 8, 2);
        c.place(&b.cut_sandstone, x, 8, 3);
    }

    for x in [2, W - 3] {
        c.place(&b.cut_sandstone, x - 1, 2, 0);
        c.place(&b.orange_terracotta, x, 2, 0);
        c.place(&b.cut_sandstone, x + 1, 2, 0);
        c.place(&b.cut_sandstone, x - 1, 3, 0);
        c.place(&b.orange_terracotta, x, 3, 0);
        c.place(&b.cut_sandstone, x + 1, 3, 0);
        c.place(&b.orange_terracotta, x - 1, 4, 0);
        c.place(&b.chiseled_sandstone, x, 4, 0);
        c.place(&b.orange_terracotta, x + 1, 4, 0);
        c.place(&b.cut_sandstone, x - 1, 5, 0);
        c.place(&b.orange_terracotta, x, 5, 0);
        c.place(&b.cut_sandstone, x + 1, 5, 0);
        c.place(&b.orange_terracotta, x - 1, 6, 0);
        c.place(&b.chiseled_sandstone, x, 6, 0);
        c.place(&b.orange_terracotta, x + 1, 6, 0);
        c.place(&b.orange_terracotta, x - 1, 7, 0);
        c.place(&b.orange_terracotta, x, 7, 0);
        c.place(&b.orange_terracotta, x + 1, 7, 0);
        c.place(&b.cut_sandstone, x - 1, 8, 0);
        c.place(&b.cut_sandstone, x, 8, 0);
        c.place(&b.cut_sandstone, x + 1, 8, 0);
    }

    c.generate_box([8, 4, 0], [12, 6, 0], &b.cut_sandstone, &b.cut_sandstone, false);
    c.place(&b.air, 8, 6, 0);
    c.place(&b.air, 12, 6, 0);
    c.place(&b.orange_terracotta, 9, 5, 0);
    c.place(&b.chiseled_sandstone, 10, 5, 0);
    c.place(&b.orange_terracotta, 11, 5, 0);
    c.generate_box([8, -14, 8], [12, -11, 12], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box(
        [8, -10, 8],
        [12, -10, 12],
        &b.chiseled_sandstone,
        &b.chiseled_sandstone,
        false,
    );
    c.generate_box([8, -9, 8], [12, -9, 12], &b.cut_sandstone, &b.cut_sandstone, false);
    c.generate_box([8, -8, 8], [12, -1, 12], &b.sandstone, &b.sandstone, false);
    c.generate_box([9, -11, 9], [11, -1, 11], &b.air, &b.air, false);
    c.place(&b.stone_pressure_plate, 10, -11, 10);
    c.generate_box([9, -13, 9], [11, -13, 11], &b.tnt, &b.air, false);
    c.place(&b.air, 8, -11, 10);
    c.place(&b.air, 8, -10, 10);
    c.place(&b.chiseled_sandstone, 7, -10, 10);
    c.place(&b.cut_sandstone, 7, -11, 10);
    c.place(&b.air, 12, -11, 10);
    c.place(&b.air, 12, -10, 10);
    c.place(&b.chiseled_sandstone, 13, -10, 10);
    c.place(&b.cut_sandstone, 13, -11, 10);
    c.place(&b.air, 10, -11, 8);
    c.place(&b.air, 10, -10, 8);
    c.place(&b.chiseled_sandstone, 10, -10, 7);
    c.place(&b.cut_sandstone, 10, -11, 7);
    c.place(&b.air, 10, -11, 12);
    c.place(&b.air, 10, -10, 12);
    c.place(&b.chiseled_sandstone, 10, -10, 13);
    c.place(&b.cut_sandstone, 10, -11, 13);

    for (step_x, step_z) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        c.create_chest(
            rng,
            &b.chest,
            10 + step_x * 2,
            -11,
            10 + step_z * 2,
            DESERT_PYRAMID_LOOT,
        );
    }

    add_cellar_stairs(b, c, rng);
    add_cellar_room(b, c, rng);
}

fn add_cellar_stairs<W: WorldGenVolume, R: Random>(
    b: &DesertPyramidBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    let IVec3 { x, y, z } = CELLAR_CENTRE;
    c.place(&b.stairs_west, 13, -1, 17);
    c.place(&b.stairs_west, 14, -2, 17);
    c.place(&b.stairs_west, 15, -3, 17);
    let variant = rng.next_bool();
    c.place(&b.sand, x - 4, y + 4, z + 4);
    c.place(&b.sand, x - 3, y + 4, z + 4);
    c.place(&b.sand, x - 2, y + 4, z + 4);
    c.place(&b.sand, x - 1, y + 4, z + 4);
    c.place(&b.sand, x, y + 4, z + 4);
    c.place(&b.sand, x - 2, y + 3, z + 4);
    c.place(if variant { &b.sand } else { &b.sandstone }, x - 1, y + 3, z + 4);
    c.place(if variant { &b.sandstone } else { &b.sand }, x, y + 3, z + 4);
    c.place(&b.sand, x - 1, y + 2, z + 4);
    c.place(&b.sandstone, x, y + 2, z + 4);
    c.place(&b.sand, x, y + 1, z + 4);
}

fn add_cellar_room<W: WorldGenVolume, R: Random>(
    b: &DesertPyramidBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    let IVec3 { x, y, z } = CELLAR_CENTRE;
    let cut = &b.cut_sandstone;
    let chiseled = &b.chiseled_sandstone;
    c.generate_box([x - 3, y + 1, z - 3], [x - 3, y + 1, z + 2], cut, cut, true);
    c.generate_box([x + 3, y + 1, z - 3], [x + 3, y + 1, z + 2], cut, cut, true);
    c.generate_box([x - 3, y + 1, z - 3], [x + 3, y + 1, z - 2], cut, cut, true);
    c.generate_box([x - 3, y + 1, z + 3], [x + 3, y + 1, z + 3], cut, cut, true);
    c.generate_box([x - 3, y + 2, z - 3], [x - 3, y + 2, z + 2], chiseled, chiseled, true);
    c.generate_box([x + 3, y + 2, z - 3], [x + 3, y + 2, z + 2], chiseled, chiseled, true);
    c.generate_box([x - 3, y + 2, z - 3], [x + 3, y + 2, z - 2], chiseled, chiseled, true);
    c.generate_box([x - 3, y + 2, z + 3], [x + 3, y + 2, z + 3], chiseled, chiseled, true);
    c.generate_box([x - 3, -1, z - 3], [x - 3, -1, z + 2], cut, cut, true);
    c.generate_box([x + 3, -1, z - 3], [x + 3, -1, z + 2], cut, cut, true);
    c.generate_box([x - 3, -1, z - 3], [x + 3, -1, z - 2], cut, cut, true);
    c.generate_box([x - 3, -1, z + 3], [x + 3, -1, z + 3], cut, cut, true);
    for roof_x in x - 2..=x + 2 {
        for roof_z in z - 2..=z + 2 {
            let state = if rng.next_f32() < 0.33 {
                &b.sandstone
            } else {
                &b.sand
            };
            c.place(state, roof_x, y + 4, roof_z);
        }
    }
    let orange = &b.orange_terracotta;
    c.place(&b.blue_terracotta, x, y, z);
    c.place(orange, x + 1, y, z - 1);
    c.place(orange, x + 1, y, z + 1);
    c.place(orange, x - 1, y, z - 1);
    c.place(orange, x - 1, y, z + 1);
    c.place(orange, x + 2, y, z);
    c.place(orange, x - 2, y, z);
    c.place(orange, x, y, z + 2);
    c.place(orange, x, y, z - 2);
    c.place(orange, x + 3, y, z);
    c.place(cut, x + 4, y + 1, z);
    c.place(chiseled, x + 4, y + 2, z);
    c.place(orange, x - 3, y, z);
    c.place(cut, x - 4, y + 1, z);
    c.place(chiseled, x - 4, y + 2, z);
    c.place(orange, x, y, z + 3);
    c.place(orange, x, y, z - 3);
    c.place(cut, x, y + 1, z - 4);
    c.place(chiseled, x, -2, z - 4);
}

/// Where `placeCollapsedRoof` leaves its one marked cell: a positional draw at
/// the roof's first corner, from the world seed.
pub fn collapsed_roof_pos(
    world_seed: i64,
    bounds: BoundingBox,
    orientation: Orientation,
) -> BlockPos {
    let IVec3 { x, y, z } = CELLAR_CENTRE;
    let (x0, y0, z0, x1, z1) = (x - 2, y + 4, z - 2, x + 2, z + 2);
    let corner = world_pos(Some(orientation), bounds, IVec3::new(x0, y0, z0));
    let mut rng = mcrs_minecraft_random::legacy::LegacyRandom::new(world_seed as u64)
        .fork_at(*corner);
    let roof_x = rng.next_int_between_inclusive(x0, x1);
    let roof_z = rng.next_int_between_inclusive(z0, z1);
    world_pos(Some(orientation), bounds, IVec3::new(roof_x, y0, roof_z))
}

/// `getPotentialSuspiciousSandWorldPositions`: the cellar's sand box and the
/// eight cells behind its four doorways, in the order the reference lists
/// them.
pub fn potential_suspicious_sand(
    bounds: BoundingBox,
    orientation: Orientation,
) -> impl Iterator<Item = BlockPos> {
    let IVec3 { x, y, z } = CELLAR_CENTRE;
    let sand_box = (y + 1..=y + 3)
        .flat_map(move |y| (x - 2..=x + 2).flat_map(move |x| (z - 2..=z + 2).map(move |z| (x, y, z))));
    let doorways = [
        (x + 3, y + 1, z),
        (x + 3, y + 2, z),
        (x - 3, y + 1, z),
        (x - 3, y + 2, z),
        (x, y + 1, z + 3),
        (x, y + 2, z + 3),
        (x, y + 1, z - 3),
        (x, y + 2, z - 3),
    ];
    sand_box
        .chain(doorways)
        .map(move |(x, y, z)| world_pos(Some(orientation), bounds, IVec3::new(x, y, z)))
}
