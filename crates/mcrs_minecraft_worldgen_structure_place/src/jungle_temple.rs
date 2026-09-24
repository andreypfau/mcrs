use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_structure::piece::JungleTemplePiece;

use crate::canvas::{ChestStates, PieceCanvas};
use crate::{Oriented, block_mask};

pub const JUNGLE_TEMPLE_LOOT: &str = "minecraft:chests/jungle_temple";
pub const JUNGLE_TEMPLE_DISPENSER_LOOT: &str = "minecraft:chests/jungle_temple_dispenser";

#[derive(Clone, Debug)]
pub struct JungleTempleBlocks {
    pub cobblestone: Oriented,
    pub mossy_cobblestone: Oriented,
    pub air: Oriented,
    pub stairs_north: Oriented,
    pub stairs_south: Oriented,
    pub stairs_east: Oriented,
    pub stairs_west: Oriented,
    pub hook_north: Oriented,
    pub hook_south: Oriented,
    pub hook_east: Oriented,
    pub hook_west: Oriented,
    pub tripwire_ew: Oriented,
    pub tripwire_ns: Oriented,
    pub wire_ns: Oriented,
    pub wire_nw: Oriented,
    pub wire_ew: Oriented,
    pub wire_ws: Oriented,
    pub wire_n_s_up: Oriented,
    pub wire_all: Oriented,
    pub vine_south: Oriented,
    pub vine_east: Oriented,
    pub dispenser_north: Oriented,
    pub dispenser_west: Oriented,
    pub dispenser_states: StateMask,
    pub chiseled_stone_bricks: Oriented,
    pub lever: Oriented,
    pub piston_up: Oriented,
    pub piston_west: Oriented,
    pub repeater_north: Oriented,
    pub chest: ChestStates,
}

impl JungleTempleBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
    ) -> Result<Self, FeatureCompileError> {
        let of = |block: &str, properties: &[(&str, &str)]| {
            Oriented::named(world, blocks, block, properties)
        };
        let stairs = |facing| of("minecraft:cobblestone_stairs", &[("facing", facing)]);
        let hook = |facing| {
            of(
                "minecraft:tripwire_hook",
                &[("facing", facing), ("attached", "true")],
            )
        };
        let tripwire = |a, b| {
            of(
                "minecraft:tripwire",
                &[(a, "true"), (b, "true"), ("attached", "true")],
            )
        };
        let wire = |sides: &[(&str, &str)]| of("minecraft:redstone_wire", sides);
        Ok(JungleTempleBlocks {
            cobblestone: of("minecraft:cobblestone", &[])?,
            mossy_cobblestone: of("minecraft:mossy_cobblestone", &[])?,
            air: of("minecraft:air", &[])?,
            stairs_north: stairs("north")?,
            stairs_south: stairs("south")?,
            stairs_east: stairs("east")?,
            stairs_west: stairs("west")?,
            hook_north: hook("north")?,
            hook_south: hook("south")?,
            hook_east: hook("east")?,
            hook_west: hook("west")?,
            tripwire_ew: tripwire("east", "west")?,
            tripwire_ns: tripwire("north", "south")?,
            wire_ns: wire(&[("north", "side"), ("south", "side")])?,
            wire_nw: wire(&[("north", "side"), ("west", "side")])?,
            wire_ew: wire(&[("east", "side"), ("west", "side")])?,
            wire_ws: wire(&[("west", "side"), ("south", "side")])?,
            wire_n_s_up: wire(&[("north", "side"), ("south", "up")])?,
            wire_all: wire(&[
                ("north", "side"),
                ("south", "side"),
                ("east", "side"),
                ("west", "side"),
            ])?,
            vine_south: of("minecraft:vine", &[("south", "true")])?,
            vine_east: of("minecraft:vine", &[("east", "true")])?,
            dispenser_north: of("minecraft:dispenser", &[("facing", "north")])?,
            dispenser_west: of("minecraft:dispenser", &[("facing", "west")])?,
            dispenser_states: block_mask(blocks, &["minecraft:dispenser"])?,
            chiseled_stone_bricks: of("minecraft:chiseled_stone_bricks", &[])?,
            lever: of("minecraft:lever", &[("facing", "north"), ("face", "wall")])?,
            piston_up: of("minecraft:sticky_piston", &[("facing", "up")])?,
            piston_west: of("minecraft:sticky_piston", &[("facing", "west")])?,
            repeater_north: of("minecraft:repeater", &[("facing", "north")])?,
            chest: ChestStates::compile(blocks)?,
        })
    }
}

/// `JungleTemplePiece.MossStoneSelector` over a box: one float per cell.
fn moss_box<W: WorldGenVolume, R: Random>(
    b: &JungleTempleBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    min: [i32; 3],
    max: [i32; 3],
) {
    c.generate_selected_box(min, max, false, |_| {
        if rng.next_f32() < 0.4 {
            &b.cobblestone
        } else {
            &b.mossy_cobblestone
        }
    });
}

/// `JungleTemplePiece.postProcess` for one column, `bounds` already raised
/// to its ground.
pub fn paint_jungle_temple<W: WorldGenVolume, R: Random>(
    b: &JungleTempleBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    const W: i32 = JungleTemplePiece::WIDTH;
    const D: i32 = JungleTemplePiece::DEPTH;
    let air = &b.air;
    let mossy = &b.mossy_cobblestone;

    moss_box(b, c, rng, [0, -4, 0], [W - 1, 0, D - 1]);
    moss_box(b, c, rng, [2, 1, 2], [9, 2, 2]);
    moss_box(b, c, rng, [2, 1, 12], [9, 2, 12]);
    moss_box(b, c, rng, [2, 1, 3], [2, 2, 11]);
    moss_box(b, c, rng, [9, 1, 3], [9, 2, 11]);
    moss_box(b, c, rng, [1, 3, 1], [10, 6, 1]);
    moss_box(b, c, rng, [1, 3, 13], [10, 6, 13]);
    moss_box(b, c, rng, [1, 3, 2], [1, 6, 12]);
    moss_box(b, c, rng, [10, 3, 2], [10, 6, 12]);
    moss_box(b, c, rng, [2, 3, 2], [9, 3, 12]);
    moss_box(b, c, rng, [2, 6, 2], [9, 6, 12]);
    moss_box(b, c, rng, [3, 7, 3], [8, 7, 11]);
    moss_box(b, c, rng, [4, 8, 4], [7, 8, 10]);
    c.generate_box([3, 1, 3], [8, 2, 11], air, air, false);
    c.generate_box([4, 3, 6], [7, 3, 9], air, air, false);
    c.generate_box([2, 4, 2], [9, 5, 12], air, air, false);
    c.generate_box([4, 6, 5], [7, 6, 9], air, air, false);
    c.generate_box([5, 7, 6], [6, 7, 8], air, air, false);
    c.generate_box([5, 1, 2], [6, 2, 2], air, air, false);
    c.generate_box([5, 2, 12], [6, 2, 12], air, air, false);
    c.generate_box([5, 5, 1], [6, 5, 1], air, air, false);
    c.generate_box([5, 5, 13], [6, 5, 13], air, air, false);
    c.place(air, 1, 5, 5);
    c.place(air, 10, 5, 5);
    c.place(air, 1, 5, 9);
    c.place(air, 10, 5, 9);

    for z in [0, 14] {
        moss_box(b, c, rng, [2, 4, z], [2, 5, z]);
        moss_box(b, c, rng, [4, 4, z], [4, 5, z]);
        moss_box(b, c, rng, [7, 4, z], [7, 5, z]);
        moss_box(b, c, rng, [9, 4, z], [9, 5, z]);
    }

    moss_box(b, c, rng, [5, 6, 0], [6, 6, 0]);

    for x in [0, 11] {
        for z in (2..=12).step_by(2) {
            moss_box(b, c, rng, [x, 4, z], [x, 5, z]);
        }
        moss_box(b, c, rng, [x, 6, 5], [x, 6, 5]);
        moss_box(b, c, rng, [x, 6, 9], [x, 6, 9]);
    }

    moss_box(b, c, rng, [2, 7, 2], [2, 9, 2]);
    moss_box(b, c, rng, [9, 7, 2], [9, 9, 2]);
    moss_box(b, c, rng, [2, 7, 12], [2, 9, 12]);
    moss_box(b, c, rng, [9, 7, 12], [9, 9, 12]);
    moss_box(b, c, rng, [4, 9, 4], [4, 9, 4]);
    moss_box(b, c, rng, [7, 9, 4], [7, 9, 4]);
    moss_box(b, c, rng, [4, 9, 10], [4, 9, 10]);
    moss_box(b, c, rng, [7, 9, 10], [7, 9, 10]);
    moss_box(b, c, rng, [5, 9, 7], [6, 9, 7]);
    c.place(&b.stairs_north, 5, 9, 6);
    c.place(&b.stairs_north, 6, 9, 6);
    c.place(&b.stairs_south, 5, 9, 8);
    c.place(&b.stairs_south, 6, 9, 8);
    c.place(&b.stairs_north, 4, 0, 0);
    c.place(&b.stairs_north, 5, 0, 0);
    c.place(&b.stairs_north, 6, 0, 0);
    c.place(&b.stairs_north, 7, 0, 0);
    c.place(&b.stairs_north, 4, 1, 8);
    c.place(&b.stairs_north, 4, 2, 9);
    c.place(&b.stairs_north, 4, 3, 10);
    c.place(&b.stairs_north, 7, 1, 8);
    c.place(&b.stairs_north, 7, 2, 9);
    c.place(&b.stairs_north, 7, 3, 10);
    moss_box(b, c, rng, [4, 1, 9], [4, 1, 9]);
    moss_box(b, c, rng, [7, 1, 9], [7, 1, 9]);
    moss_box(b, c, rng, [4, 1, 10], [7, 2, 10]);
    moss_box(b, c, rng, [5, 4, 5], [6, 4, 5]);
    c.place(&b.stairs_east, 4, 4, 5);
    c.place(&b.stairs_west, 7, 4, 5);

    for i in 0..4 {
        c.place(&b.stairs_south, 5, -i, 6 + i);
        c.place(&b.stairs_south, 6, -i, 6 + i);
        c.generate_box([5, -i, 7 + i], [6, -i, 9 + i], air, air, false);
    }

    c.generate_box([1, -3, 12], [10, -1, 13], air, air, false);
    c.generate_box([1, -3, 1], [3, -1, 13], air, air, false);
    c.generate_box([1, -3, 1], [9, -1, 5], air, air, false);

    for z in (1..=13).step_by(2) {
        moss_box(b, c, rng, [1, -3, z], [1, -2, z]);
    }

    for z in (2..=12).step_by(2) {
        moss_box(b, c, rng, [1, -1, z], [3, -1, z]);
    }

    moss_box(b, c, rng, [2, -2, 1], [5, -2, 1]);
    moss_box(b, c, rng, [7, -2, 1], [9, -2, 1]);
    moss_box(b, c, rng, [6, -3, 1], [6, -3, 1]);
    moss_box(b, c, rng, [6, -1, 1], [6, -1, 1]);
    c.place(&b.hook_east, 1, -3, 8);
    c.place(&b.hook_west, 4, -3, 8);
    c.place(&b.tripwire_ew, 2, -3, 8);
    c.place(&b.tripwire_ew, 3, -3, 8);
    c.place(&b.wire_ns, 5, -3, 7);
    c.place(&b.wire_ns, 5, -3, 6);
    c.place(&b.wire_ns, 5, -3, 5);
    c.place(&b.wire_ns, 5, -3, 4);
    c.place(&b.wire_ns, 5, -3, 3);
    c.place(&b.wire_ns, 5, -3, 2);
    c.place(&b.wire_nw, 5, -3, 1);
    c.place(&b.wire_ew, 4, -3, 1);
    c.place(mossy, 3, -3, 1);
    c.create_dispenser(
        rng,
        &b.dispenser_north,
        &b.dispenser_states,
        3,
        -2,
        1,
        JUNGLE_TEMPLE_DISPENSER_LOOT,
    );

    c.place(&b.vine_south, 3, -2, 2);
    c.place(&b.hook_north, 7, -3, 1);
    c.place(&b.hook_south, 7, -3, 5);
    c.place(&b.tripwire_ns, 7, -3, 2);
    c.place(&b.tripwire_ns, 7, -3, 3);
    c.place(&b.tripwire_ns, 7, -3, 4);
    c.place(&b.wire_ew, 8, -3, 6);
    c.place(&b.wire_ws, 9, -3, 6);
    c.place(&b.wire_n_s_up, 9, -3, 5);
    c.place(mossy, 9, -3, 4);
    c.place(&b.wire_ns, 9, -2, 4);
    c.create_dispenser(
        rng,
        &b.dispenser_west,
        &b.dispenser_states,
        9,
        -2,
        3,
        JUNGLE_TEMPLE_DISPENSER_LOOT,
    );

    c.place(&b.vine_east, 8, -1, 3);
    c.place(&b.vine_east, 8, -2, 3);
    c.create_chest(rng, &b.chest, 8, -3, 3, JUNGLE_TEMPLE_LOOT);

    c.place(mossy, 9, -3, 2);
    c.place(mossy, 8, -3, 1);
    c.place(mossy, 4, -3, 5);
    c.place(mossy, 5, -2, 5);
    c.place(mossy, 5, -1, 5);
    c.place(mossy, 6, -3, 5);
    c.place(mossy, 7, -2, 5);
    c.place(mossy, 7, -1, 5);
    c.place(mossy, 8, -3, 5);
    moss_box(b, c, rng, [9, -1, 1], [9, -1, 5]);
    c.generate_box([8, -3, 8], [10, -1, 10], air, air, false);
    c.place(&b.chiseled_stone_bricks, 8, -2, 11);
    c.place(&b.chiseled_stone_bricks, 9, -2, 11);
    c.place(&b.chiseled_stone_bricks, 10, -2, 11);
    c.place(&b.lever, 8, -2, 12);
    c.place(&b.lever, 9, -2, 12);
    c.place(&b.lever, 10, -2, 12);
    moss_box(b, c, rng, [8, -3, 8], [8, -3, 10]);
    moss_box(b, c, rng, [10, -3, 8], [10, -3, 10]);
    c.place(mossy, 10, -2, 9);
    c.place(&b.wire_ns, 8, -2, 9);
    c.place(&b.wire_ns, 8, -2, 10);
    c.place(&b.wire_all, 10, -1, 9);
    c.place(&b.piston_up, 9, -2, 8);
    c.place(&b.piston_west, 10, -2, 8);
    c.place(&b.piston_west, 10, -1, 8);
    c.place(&b.repeater_north, 10, -2, 10);
    c.create_chest(rng, &b.chest, 9, -3, 10, JUNGLE_TEMPLE_LOOT);
}
