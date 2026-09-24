use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::{WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_structure::orient::Orientation;
use mcrs_minecraft_worldgen_structure::piece::{SmallDoor, StrongholdKind};

use crate::canvas::{ChestStates, PieceCanvas};
use crate::{Oriented, state};

pub const CORRIDOR_LOOT: &str = "minecraft:chests/stronghold_corridor";
pub const LIBRARY_LOOT: &str = "minecraft:chests/stronghold_library";
pub const CROSSING_LOOT: &str = "minecraft:chests/stronghold_crossing";

#[derive(Clone, Debug)]
pub struct StrongholdBlocks {
    pub stone_bricks: Oriented,
    pub cracked_stone_bricks: Oriented,
    pub mossy_stone_bricks: Oriented,
    pub infested_stone_bricks: Oriented,
    pub cave_air: Oriented,
    pub stone_brick_slab: Oriented,
    pub smooth_stone_slab: Oriented,
    pub smooth_stone_slab_double: Oriented,
    pub oak_planks: Oriented,
    pub bookshelf: Oriented,
    pub cobweb: Oriented,
    pub cobblestone: Oriented,
    pub water: Oriented,
    pub lava: Oriented,
    pub end_portal: Oriented,
    pub torch: Oriented,
    pub wall_torch_north: Oriented,
    pub wall_torch_south: Oriented,
    pub wall_torch_east: Oriented,
    pub wall_torch_west: Oriented,
    pub ladder_south: Oriented,
    pub ladder_west: Oriented,
    pub stone_brick_stairs_north: Oriented,
    pub cobblestone_stairs_south: Oriented,
    pub bars_ns: Oriented,
    pub bars_we: Oriented,
    pub bars_nse: Oriented,
    pub bars_w: Oriented,
    pub bars_e: Oriented,
    pub fence_we: Oriented,
    pub fence_ns: Oriented,
    pub fence_ne: Oriented,
    pub fence_se: Oriented,
    pub fence_nw: Oriented,
    pub fence_sw: Oriented,
    pub fence_e: Oriented,
    pub fence_w: Oriented,
    pub fence_nswe: Oriented,
    pub oak_door_lower: Oriented,
    pub oak_door_upper: Oriented,
    pub iron_door_lower: Oriented,
    pub iron_door_upper: Oriented,
    pub iron_door_west_lower: Oriented,
    pub iron_door_west_upper: Oriented,
    pub button_north: Oriented,
    pub button_south: Oriented,
    /// Indexed by `Direction::HORIZONTAL` (north first), then by whether the
    /// frame holds an eye.
    pub portal_frame: [[Oriented; 2]; 4],
    pub spawner: VoxelId,
    pub chest: ChestStates,
}

impl StrongholdBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
    ) -> Result<Self, FeatureCompileError> {
        let of = |block: &str, properties: &[(&str, &str)]| {
            Ok(Oriented::of(world, state(blocks, block, properties)?))
        };
        let plain = |block: &str| of(block, &[]);
        let sides = |block: &str, on: &[&str]| {
            let on: Vec<(&str, &str)> = on.iter().map(|side| (*side, "true")).collect();
            of(block, &on)
        };
        let bars = |on: &[&str]| sides("minecraft:iron_bars", on);
        let fence = |on: &[&str]| sides("minecraft:oak_fence", on);
        let frame = |facing: &str| -> Result<[Oriented; 2], FeatureCompileError> {
            Ok([
                of(
                    "minecraft:end_portal_frame",
                    &[("facing", facing), ("eye", "false")],
                )?,
                of(
                    "minecraft:end_portal_frame",
                    &[("facing", facing), ("eye", "true")],
                )?,
            ])
        };
        Ok(StrongholdBlocks {
            stone_bricks: plain("minecraft:stone_bricks")?,
            cracked_stone_bricks: plain("minecraft:cracked_stone_bricks")?,
            mossy_stone_bricks: plain("minecraft:mossy_stone_bricks")?,
            infested_stone_bricks: plain("minecraft:infested_stone_bricks")?,
            cave_air: plain("minecraft:cave_air")?,
            stone_brick_slab: plain("minecraft:stone_brick_slab")?,
            smooth_stone_slab: plain("minecraft:smooth_stone_slab")?,
            smooth_stone_slab_double: of("minecraft:smooth_stone_slab", &[("type", "double")])?,
            oak_planks: plain("minecraft:oak_planks")?,
            bookshelf: plain("minecraft:bookshelf")?,
            cobweb: plain("minecraft:cobweb")?,
            cobblestone: plain("minecraft:cobblestone")?,
            water: plain("minecraft:water")?,
            lava: plain("minecraft:lava")?,
            end_portal: plain("minecraft:end_portal")?,
            torch: plain("minecraft:torch")?,
            wall_torch_north: of("minecraft:wall_torch", &[("facing", "north")])?,
            wall_torch_south: of("minecraft:wall_torch", &[("facing", "south")])?,
            wall_torch_east: of("minecraft:wall_torch", &[("facing", "east")])?,
            wall_torch_west: of("minecraft:wall_torch", &[("facing", "west")])?,
            ladder_south: of("minecraft:ladder", &[("facing", "south")])?,
            ladder_west: of("minecraft:ladder", &[("facing", "west")])?,
            stone_brick_stairs_north: of("minecraft:stone_brick_stairs", &[("facing", "north")])?,
            cobblestone_stairs_south: of("minecraft:cobblestone_stairs", &[("facing", "south")])?,
            bars_ns: bars(&["north", "south"])?,
            bars_we: bars(&["west", "east"])?,
            bars_nse: bars(&["north", "south", "east"])?,
            bars_w: bars(&["west"])?,
            bars_e: bars(&["east"])?,
            fence_we: fence(&["west", "east"])?,
            fence_ns: fence(&["north", "south"])?,
            fence_ne: fence(&["north", "east"])?,
            fence_se: fence(&["south", "east"])?,
            fence_nw: fence(&["north", "west"])?,
            fence_sw: fence(&["south", "west"])?,
            fence_e: fence(&["east"])?,
            fence_w: fence(&["west"])?,
            fence_nswe: fence(&["north", "south", "west", "east"])?,
            oak_door_lower: plain("minecraft:oak_door")?,
            oak_door_upper: of("minecraft:oak_door", &[("half", "upper")])?,
            iron_door_lower: plain("minecraft:iron_door")?,
            iron_door_upper: of("minecraft:iron_door", &[("half", "upper")])?,
            iron_door_west_lower: of("minecraft:iron_door", &[("facing", "west")])?,
            iron_door_west_upper: of(
                "minecraft:iron_door",
                &[("facing", "west"), ("half", "upper")],
            )?,
            button_north: of("minecraft:stone_button", &[("facing", "north")])?,
            button_south: of("minecraft:stone_button", &[("facing", "south")])?,
            portal_frame: [
                frame("north")?,
                frame("east")?,
                frame("south")?,
                frame("west")?,
            ],
            spawner: state(blocks, "minecraft:spawner", &[])?,
            chest: ChestStates::compile(blocks)?,
        })
    }

    /// `SmoothStoneSelector.next` on an edge cell.
    fn smooth_stone<R: Random>(&self, rng: &mut R) -> &Oriented {
        let selection = rng.next_f32();
        if selection < 0.2 {
            &self.cracked_stone_bricks
        } else if selection < 0.5 {
            &self.mossy_stone_bricks
        } else if selection < 0.55 {
            &self.infested_stone_bricks
        } else {
            &self.stone_bricks
        }
    }
}

/// `generateBox` with the smooth stone selector: mixed stone bricks on the
/// faces, cave air inside.
fn walls<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    min: [i32; 3],
    max: [i32; 3],
    skip_air: bool,
) {
    c.generate_selected_box(min, max, skip_air, |edge| {
        if edge {
            b.smooth_stone(rng)
        } else {
            &b.cave_air
        }
    });
}

/// `generateSmallDoor`.
fn door<W: WorldGenVolume>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    kind: SmallDoor,
    x: i32,
    y: i32,
    z: i32,
) {
    let sb = &b.stone_bricks;
    let frame = |c: &mut PieceCanvas<'_, W>| {
        c.place(sb, x, y, z);
        c.place(sb, x, y + 1, z);
        c.place(sb, x, y + 2, z);
        c.place(sb, x + 1, y + 2, z);
        c.place(sb, x + 2, y + 2, z);
        c.place(sb, x + 2, y + 1, z);
        c.place(sb, x + 2, y, z);
    };
    match kind {
        SmallDoor::Opening => c.solid(&b.cave_air, [x, y, z], [x + 2, y + 2, z]),
        SmallDoor::WoodDoor => {
            frame(c);
            c.place(&b.oak_door_lower, x + 1, y, z);
            c.place(&b.oak_door_upper, x + 1, y + 1, z);
        }
        SmallDoor::Grates => {
            c.place(&b.cave_air, x + 1, y, z);
            c.place(&b.cave_air, x + 1, y + 1, z);
            c.place(&b.bars_w, x, y, z);
            c.place(&b.bars_w, x, y + 1, z);
            c.place(&b.bars_we, x, y + 2, z);
            c.place(&b.bars_we, x + 1, y + 2, z);
            c.place(&b.bars_we, x + 2, y + 2, z);
            c.place(&b.bars_e, x + 2, y + 1, z);
            c.place(&b.bars_e, x + 2, y, z);
        }
        SmallDoor::IronDoor => {
            frame(c);
            c.place(&b.iron_door_lower, x + 1, y, z);
            c.place(&b.iron_door_upper, x + 1, y + 1, z);
            c.place(&b.button_north, x + 2, y + 1, z + 1);
            c.place(&b.button_south, x + 2, y + 1, z - 1);
        }
    }
}

/// Each type's `postProcess` for one column.
pub fn paint_stronghold<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    kind: StrongholdKind,
    entry_door: SmallDoor,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    match kind {
        StrongholdKind::Start | StrongholdKind::StairsDown => stairs_down(b, c, rng, entry_door),
        StrongholdKind::Straight { left, right } => straight(b, c, rng, entry_door, left, right),
        StrongholdKind::PrisonHall => prison_hall(b, c, rng, entry_door),
        StrongholdKind::LeftTurn => left_turn(b, c, rng, entry_door),
        StrongholdKind::RightTurn => right_turn(b, c, rng, entry_door),
        StrongholdKind::RoomCrossing { variant } => room_crossing(b, c, rng, entry_door, variant),
        StrongholdKind::StraightStairsDown => straight_stairs_down(b, c, rng, entry_door),
        StrongholdKind::FiveCrossing {
            left_low,
            left_high,
            right_low,
            right_high,
        } => five_crossing(
            b, c, rng, entry_door, left_low, left_high, right_low, right_high,
        ),
        StrongholdKind::ChestCorridor => chest_corridor(b, c, rng, entry_door),
        StrongholdKind::Library { tall } => library(b, c, rng, entry_door, tall),
        StrongholdKind::PortalRoom => portal_room(b, c, rng),
        StrongholdKind::FillerCorridor { steps } => filler_corridor(b, c, steps),
    }
}

fn stairs_down<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
) {
    let sb = &b.stone_bricks;
    let slab = &b.smooth_stone_slab;
    walls(b, c, rng, [0, 0, 0], [4, 10, 4], true);
    door(b, c, entry_door, 1, 7, 0);
    door(b, c, SmallDoor::Opening, 1, 1, 4);
    c.place(sb, 2, 6, 1);
    c.place(sb, 1, 5, 1);
    c.place(slab, 1, 6, 1);
    c.place(sb, 1, 5, 2);
    c.place(sb, 1, 4, 3);
    c.place(slab, 1, 5, 3);
    c.place(sb, 2, 4, 3);
    c.place(sb, 3, 3, 3);
    c.place(slab, 3, 4, 3);
    c.place(sb, 3, 3, 2);
    c.place(sb, 3, 2, 1);
    c.place(slab, 3, 3, 1);
    c.place(sb, 2, 2, 1);
    c.place(sb, 1, 1, 1);
    c.place(slab, 1, 2, 1);
    c.place(sb, 1, 1, 2);
    c.place(slab, 1, 1, 3);
}

fn straight<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
    left: bool,
    right: bool,
) {
    walls(b, c, rng, [0, 0, 0], [4, 4, 6], true);
    door(b, c, entry_door, 1, 1, 0);
    door(b, c, SmallDoor::Opening, 1, 1, 6);
    c.maybe_generate_block(rng, 0.1, 1, 2, 1, &b.wall_torch_east);
    c.maybe_generate_block(rng, 0.1, 3, 2, 1, &b.wall_torch_west);
    c.maybe_generate_block(rng, 0.1, 1, 2, 5, &b.wall_torch_east);
    c.maybe_generate_block(rng, 0.1, 3, 2, 5, &b.wall_torch_west);
    if left {
        c.solid(&b.cave_air, [0, 1, 2], [0, 3, 4]);
    }
    if right {
        c.solid(&b.cave_air, [4, 1, 2], [4, 3, 4]);
    }
}

fn prison_hall<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
) {
    walls(b, c, rng, [0, 0, 0], [8, 4, 10], true);
    door(b, c, entry_door, 1, 1, 0);
    c.solid(&b.cave_air, [1, 1, 10], [3, 3, 10]);
    walls(b, c, rng, [4, 1, 1], [4, 3, 1], false);
    walls(b, c, rng, [4, 1, 3], [4, 3, 3], false);
    walls(b, c, rng, [4, 1, 7], [4, 3, 7], false);
    walls(b, c, rng, [4, 1, 9], [4, 3, 9], false);
    for y in 1..=3 {
        c.place(&b.bars_ns, 4, y, 4);
        c.place(&b.bars_nse, 4, y, 5);
        c.place(&b.bars_ns, 4, y, 6);
        c.place(&b.bars_we, 5, y, 5);
        c.place(&b.bars_we, 6, y, 5);
        c.place(&b.bars_we, 7, y, 5);
    }
    c.place(&b.bars_ns, 4, 3, 2);
    c.place(&b.bars_ns, 4, 3, 8);
    c.place(&b.iron_door_west_lower, 4, 1, 2);
    c.place(&b.iron_door_west_upper, 4, 2, 2);
    c.place(&b.iron_door_west_lower, 4, 1, 8);
    c.place(&b.iron_door_west_upper, 4, 2, 8);
}

fn turns_left(c: &PieceCanvas<'_, impl WorldGenVolume>) -> bool {
    matches!(c.orientation, Some(Orientation::North | Orientation::East))
}

fn left_turn<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
) {
    walls(b, c, rng, [0, 0, 0], [4, 4, 4], true);
    door(b, c, entry_door, 1, 1, 0);
    if turns_left(c) {
        c.solid(&b.cave_air, [0, 1, 1], [0, 3, 3]);
    } else {
        c.solid(&b.cave_air, [4, 1, 1], [4, 3, 3]);
    }
}

fn right_turn<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
) {
    walls(b, c, rng, [0, 0, 0], [4, 4, 4], true);
    door(b, c, entry_door, 1, 1, 0);
    if turns_left(c) {
        c.solid(&b.cave_air, [4, 1, 1], [4, 3, 3]);
    } else {
        c.solid(&b.cave_air, [0, 1, 1], [0, 3, 3]);
    }
}

fn room_crossing<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
    variant: i32,
) {
    let sb = &b.stone_bricks;
    let slab = &b.smooth_stone_slab;
    let cobble = &b.cobblestone;
    let planks = &b.oak_planks;
    walls(b, c, rng, [0, 0, 0], [10, 6, 10], true);
    door(b, c, entry_door, 4, 1, 0);
    c.solid(&b.cave_air, [4, 1, 10], [6, 3, 10]);
    c.solid(&b.cave_air, [0, 1, 4], [0, 3, 6]);
    c.solid(&b.cave_air, [10, 1, 4], [10, 3, 6]);
    match variant {
        0 => {
            c.place(sb, 5, 1, 5);
            c.place(sb, 5, 2, 5);
            c.place(sb, 5, 3, 5);
            c.place(&b.wall_torch_west, 4, 3, 5);
            c.place(&b.wall_torch_east, 6, 3, 5);
            c.place(&b.wall_torch_south, 5, 3, 4);
            c.place(&b.wall_torch_north, 5, 3, 6);
            c.place(slab, 4, 1, 4);
            c.place(slab, 4, 1, 5);
            c.place(slab, 4, 1, 6);
            c.place(slab, 6, 1, 4);
            c.place(slab, 6, 1, 5);
            c.place(slab, 6, 1, 6);
            c.place(slab, 5, 1, 4);
            c.place(slab, 5, 1, 6);
        }
        1 => {
            for i in 0..5 {
                c.place(sb, 3, 1, 3 + i);
                c.place(sb, 7, 1, 3 + i);
                c.place(sb, 3 + i, 1, 3);
                c.place(sb, 3 + i, 1, 7);
            }
            c.place(sb, 5, 1, 5);
            c.place(sb, 5, 2, 5);
            c.place(sb, 5, 3, 5);
            c.place(&b.water, 5, 4, 5);
        }
        2 => {
            for z in 1..=9 {
                c.place(cobble, 1, 3, z);
                c.place(cobble, 9, 3, z);
            }
            for x in 1..=9 {
                c.place(cobble, x, 3, 1);
                c.place(cobble, x, 3, 9);
            }
            c.place(cobble, 5, 1, 4);
            c.place(cobble, 5, 1, 6);
            c.place(cobble, 5, 3, 4);
            c.place(cobble, 5, 3, 6);
            c.place(cobble, 4, 1, 5);
            c.place(cobble, 6, 1, 5);
            c.place(cobble, 4, 3, 5);
            c.place(cobble, 6, 3, 5);
            for y in 1..=3 {
                c.place(cobble, 4, y, 4);
                c.place(cobble, 6, y, 4);
                c.place(cobble, 4, y, 6);
                c.place(cobble, 6, y, 6);
            }
            c.place(&b.wall_torch_north, 5, 3, 5);
            for z in 2..=8 {
                c.place(planks, 2, 3, z);
                c.place(planks, 3, 3, z);
                if z <= 3 || z >= 7 {
                    c.place(planks, 4, 3, z);
                    c.place(planks, 5, 3, z);
                    c.place(planks, 6, 3, z);
                }
                c.place(planks, 7, 3, z);
                c.place(planks, 8, 3, z);
            }
            c.place(&b.ladder_west, 9, 1, 3);
            c.place(&b.ladder_west, 9, 2, 3);
            c.place(&b.ladder_west, 9, 3, 3);
            c.create_chest(rng, &b.chest, 3, 4, 8, CROSSING_LOOT);
        }
        _ => {}
    }
}

fn straight_stairs_down<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
) {
    let sb = &b.stone_bricks;
    walls(b, c, rng, [0, 0, 0], [4, 10, 7], true);
    door(b, c, entry_door, 1, 7, 0);
    door(b, c, SmallDoor::Opening, 1, 1, 7);
    for i in 0..6 {
        c.place(&b.cobblestone_stairs_south, 1, 6 - i, 1 + i);
        c.place(&b.cobblestone_stairs_south, 2, 6 - i, 1 + i);
        c.place(&b.cobblestone_stairs_south, 3, 6 - i, 1 + i);
        if i < 5 {
            c.place(sb, 1, 5 - i, 1 + i);
            c.place(sb, 2, 5 - i, 1 + i);
            c.place(sb, 3, 5 - i, 1 + i);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn five_crossing<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
    left_low: bool,
    left_high: bool,
    right_low: bool,
    right_high: bool,
) {
    let slab = &b.smooth_stone_slab;
    walls(b, c, rng, [0, 0, 0], [9, 8, 10], true);
    door(b, c, entry_door, 4, 3, 0);
    if left_low {
        c.solid(&b.cave_air, [0, 3, 1], [0, 5, 3]);
    }
    if right_low {
        c.solid(&b.cave_air, [9, 3, 1], [9, 5, 3]);
    }
    if left_high {
        c.solid(&b.cave_air, [0, 5, 7], [0, 7, 9]);
    }
    if right_high {
        c.solid(&b.cave_air, [9, 5, 7], [9, 7, 9]);
    }
    c.solid(&b.cave_air, [5, 1, 10], [7, 3, 10]);
    walls(b, c, rng, [1, 2, 1], [8, 2, 6], false);
    walls(b, c, rng, [4, 1, 5], [4, 4, 9], false);
    walls(b, c, rng, [8, 1, 5], [8, 4, 9], false);
    walls(b, c, rng, [1, 4, 7], [3, 4, 9], false);
    walls(b, c, rng, [1, 3, 5], [3, 3, 6], false);
    c.solid(slab, [1, 3, 4], [3, 3, 4]);
    c.solid(slab, [1, 4, 6], [3, 4, 6]);
    walls(b, c, rng, [5, 1, 7], [7, 1, 8], false);
    c.solid(slab, [5, 1, 9], [7, 1, 9]);
    c.solid(slab, [5, 2, 7], [7, 2, 7]);
    c.solid(slab, [4, 5, 7], [4, 5, 9]);
    c.solid(slab, [8, 5, 7], [8, 5, 9]);
    c.solid(&b.smooth_stone_slab_double, [5, 5, 7], [7, 5, 9]);
    c.place(&b.wall_torch_south, 6, 5, 6);
}

fn chest_corridor<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
) {
    let slab = &b.stone_brick_slab;
    walls(b, c, rng, [0, 0, 0], [4, 4, 6], true);
    door(b, c, entry_door, 1, 1, 0);
    door(b, c, SmallDoor::Opening, 1, 1, 6);
    c.solid(&b.stone_bricks, [3, 1, 2], [3, 1, 4]);
    c.place(slab, 3, 1, 1);
    c.place(slab, 3, 1, 5);
    c.place(slab, 3, 2, 2);
    c.place(slab, 3, 2, 4);
    for z in 2..=4 {
        c.place(slab, 2, 1, z);
    }
    c.create_chest(rng, &b.chest, 3, 2, 3, CORRIDOR_LOOT);
}

fn library<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    entry_door: SmallDoor,
    tall: bool,
) {
    let planks = &b.oak_planks;
    let shelf = &b.bookshelf;
    let height = if tall { 11 } else { 6 };
    walls(b, c, rng, [0, 0, 0], [13, height - 1, 14], true);
    door(b, c, entry_door, 4, 1, 0);
    c.generate_maybe_box(
        rng,
        0.07,
        [2, 1, 1],
        [11, 4, 13],
        &b.cobweb,
        &b.cobweb,
        false,
        false,
    );
    for d in 1..=13 {
        if (d - 1) % 4 == 0 {
            c.solid(planks, [1, 1, d], [1, 4, d]);
            c.solid(planks, [12, 1, d], [12, 4, d]);
            c.place(&b.wall_torch_east, 2, 3, d);
            c.place(&b.wall_torch_west, 11, 3, d);
            if tall {
                c.solid(planks, [1, 6, d], [1, 9, d]);
                c.solid(planks, [12, 6, d], [12, 9, d]);
            }
        } else {
            c.solid(shelf, [1, 1, d], [1, 4, d]);
            c.solid(shelf, [12, 1, d], [12, 4, d]);
            if tall {
                c.solid(shelf, [1, 6, d], [1, 9, d]);
                c.solid(shelf, [12, 6, d], [12, 9, d]);
            }
        }
    }
    for dx in (3..12).step_by(2) {
        c.solid(shelf, [3, 1, dx], [4, 3, dx]);
        c.solid(shelf, [6, 1, dx], [7, 3, dx]);
        c.solid(shelf, [9, 1, dx], [10, 3, dx]);
    }
    if tall {
        c.solid(planks, [1, 5, 1], [3, 5, 13]);
        c.solid(planks, [10, 5, 1], [12, 5, 13]);
        c.solid(planks, [4, 5, 1], [9, 5, 2]);
        c.solid(planks, [4, 5, 12], [9, 5, 13]);
        c.place(planks, 9, 5, 11);
        c.place(planks, 8, 5, 11);
        c.place(planks, 9, 5, 10);
        c.solid(&b.fence_ns, [3, 6, 3], [3, 6, 11]);
        c.solid(&b.fence_ns, [10, 6, 3], [10, 6, 9]);
        c.solid(&b.fence_we, [4, 6, 2], [9, 6, 2]);
        c.solid(&b.fence_we, [4, 6, 12], [7, 6, 12]);
        c.place(&b.fence_ne, 3, 6, 2);
        c.place(&b.fence_se, 3, 6, 12);
        c.place(&b.fence_nw, 10, 6, 2);
        for i in 0..=2 {
            c.place(&b.fence_sw, 8 + i, 6, 12 - i);
            if i != 2 {
                c.place(&b.fence_ne, 8 + i, 6, 11 - i);
            }
        }
        for y in 1..=7 {
            c.place(&b.ladder_south, 10, y, 13);
        }
        c.place(&b.fence_e, 6, 9, 7);
        c.place(&b.fence_w, 7, 9, 7);
        c.place(&b.fence_e, 6, 8, 7);
        c.place(&b.fence_w, 7, 8, 7);
        c.place(&b.fence_nswe, 6, 7, 7);
        c.place(&b.fence_nswe, 7, 7, 7);
        c.place(&b.fence_e, 5, 7, 7);
        c.place(&b.fence_w, 8, 7, 7);
        c.place(&b.fence_ne, 6, 7, 6);
        c.place(&b.fence_se, 6, 7, 8);
        c.place(&b.fence_nw, 7, 7, 6);
        c.place(&b.fence_sw, 7, 7, 8);
        c.place(&b.torch, 5, 8, 7);
        c.place(&b.torch, 8, 8, 7);
        c.place(&b.torch, 6, 8, 6);
        c.place(&b.torch, 6, 8, 8);
        c.place(&b.torch, 7, 8, 6);
        c.place(&b.torch, 7, 8, 8);
    }
    c.create_chest(rng, &b.chest, 3, 3, 5, LIBRARY_LOOT);
    if tall {
        c.place(&b.cave_air, 12, 9, 1);
        c.create_chest(rng, &b.chest, 12, 8, 1, LIBRARY_LOOT);
    }
}

fn portal_room<W: WorldGenVolume, R: Random>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    walls(b, c, rng, [0, 0, 0], [10, 7, 15], false);
    door(b, c, SmallDoor::Grates, 4, 1, 0);
    walls(b, c, rng, [1, 6, 1], [1, 6, 14], false);
    walls(b, c, rng, [9, 6, 1], [9, 6, 14], false);
    walls(b, c, rng, [2, 6, 1], [8, 6, 2], false);
    walls(b, c, rng, [2, 6, 14], [8, 6, 14], false);
    walls(b, c, rng, [1, 1, 1], [2, 1, 4], false);
    walls(b, c, rng, [8, 1, 1], [9, 1, 4], false);
    c.solid(&b.lava, [1, 1, 1], [1, 1, 3]);
    c.solid(&b.lava, [9, 1, 1], [9, 1, 3]);
    walls(b, c, rng, [3, 1, 8], [7, 1, 12], false);
    c.solid(&b.lava, [4, 1, 9], [6, 1, 11]);
    for z in (3..14).step_by(2) {
        c.solid(&b.bars_ns, [0, 3, z], [0, 4, z]);
        c.solid(&b.bars_ns, [10, 3, z], [10, 4, z]);
    }
    for x in (2..9).step_by(2) {
        c.solid(&b.bars_we, [x, 3, 15], [x, 4, 15]);
    }
    walls(b, c, rng, [4, 1, 5], [6, 1, 7], false);
    walls(b, c, rng, [4, 2, 6], [6, 2, 7], false);
    walls(b, c, rng, [4, 3, 7], [6, 3, 7], false);
    for x in 4..=6 {
        c.place(&b.stone_brick_stairs_north, x, 1, 4);
        c.place(&b.stone_brick_stairs_north, x, 2, 5);
        c.place(&b.stone_brick_stairs_north, x, 3, 6);
    }
    let mut all_eyes = true;
    let eyes: [bool; 12] = std::array::from_fn(|_| {
        let eye = rng.next_f32() > 0.9;
        all_eyes &= eye;
        eye
    });
    let frame = |facing: usize, eye: bool| b.portal_frame[facing][usize::from(eye)];
    let (north, east, south, west) = (0, 1, 2, 3);
    c.place(&frame(north, eyes[0]), 4, 3, 8);
    c.place(&frame(north, eyes[1]), 5, 3, 8);
    c.place(&frame(north, eyes[2]), 6, 3, 8);
    c.place(&frame(south, eyes[3]), 4, 3, 12);
    c.place(&frame(south, eyes[4]), 5, 3, 12);
    c.place(&frame(south, eyes[5]), 6, 3, 12);
    c.place(&frame(east, eyes[6]), 3, 3, 9);
    c.place(&frame(east, eyes[7]), 3, 3, 10);
    c.place(&frame(east, eyes[8]), 3, 3, 11);
    c.place(&frame(west, eyes[9]), 7, 3, 9);
    c.place(&frame(west, eyes[10]), 7, 3, 10);
    c.place(&frame(west, eyes[11]), 7, 3, 11);
    if all_eyes {
        c.solid(&b.end_portal, [4, 3, 9], [6, 3, 11]);
    }
    let pos = c.world_pos(5, 3, 6);
    if c.clip.is_inside(pos) {
        c.volume.set(pos, b.spawner);
        c.entities.push(GeneratedBlockEntity::mob_spawner(
            pos,
            "minecraft:silverfish",
        ));
    }
}

fn filler_corridor<W: WorldGenVolume>(
    b: &StrongholdBlocks,
    c: &mut PieceCanvas<'_, W>,
    steps: i32,
) {
    let sb = &b.stone_bricks;
    for i in 0..steps {
        for x in 0..=4 {
            c.place(sb, x, 0, i);
        }
        for y in 1..=3 {
            c.place(sb, 0, y, i);
            c.place(&b.cave_air, 1, y, i);
            c.place(&b.cave_air, 2, y, i);
            c.place(&b.cave_air, 3, y, i);
            c.place(sb, 4, y, i);
        }
        for x in 0..=4 {
            c.place(sb, x, 4, i);
        }
    }
}
