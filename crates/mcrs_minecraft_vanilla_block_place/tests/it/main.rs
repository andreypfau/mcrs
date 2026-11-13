#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_vanilla_block_place::{Click, Spot, placed_state, replaceable};
use mcrs_voxel::BlockStateId;

fn blocks() -> &'static Blocks {
    &mcrs_minecraft_engine_registries::test_corpus().0
}

fn block(name: &str) -> Id<Block> {
    let blocks = blocks();
    blocks
        .block_index(blocks.default_state(&format!("minecraft:{name}")).unwrap())
        .unwrap()
}

fn state(name: &str, values: &[(&str, &str)]) -> BlockStateId {
    let entry = blocks().block(&format!("minecraft:{name}")).unwrap();
    values
        .iter()
        .fold(entry.default_state_id, |state, (property, value)| {
            entry.with_text(state, property, value).unwrap()
        })
}

fn click(face: Direction, cursor_y: f32, yaw: f32, pitch: f32) -> Click {
    Click {
        face,
        cursor_y,
        yaw,
        pitch,
    }
}

fn up(yaw: f32) -> Click {
    click(Direction::Up, 1.0, yaw, 0.0)
}

fn placed(name: &str, click: Click) -> Option<BlockStateId> {
    placed_state(blocks(), block(name), &click, BlockStateId::AIR)
}

#[test]
fn plc_sta_the_placed_states_of_result_r36() {
    let west = |cursor_y| click(Direction::West, cursor_y, 0.0, 0.0);
    let pitched = |pitch| click(Direction::Up, 1.0, -90.0, pitch);
    let cases: &[(&str, Click, &[(&str, &str)])] = &[
        ("oak_slab", west(0.5), &[("type", "bottom")]),
        ("oak_slab", west(0.51), &[("type", "top")]),
        (
            "oak_slab",
            click(Direction::Down, 0.0, 0.0, 0.0),
            &[("type", "top")],
        ),
        ("oak_slab", up(0.0), &[("type", "bottom")]),
        (
            "oak_stairs",
            west(0.6),
            &[("half", "top"), ("facing", "south")],
        ),
        (
            "oak_stairs",
            up(180.0),
            &[("half", "bottom"), ("facing", "north")],
        ),
        ("oak_fence_gate", up(90.0), &[("facing", "west")]),
        ("furnace", up(0.0), &[("facing", "north")]),
        ("white_glazed_terracotta", up(-90.0), &[("facing", "west")]),
        ("anvil", up(0.0), &[("facing", "west")]),
        ("anvil", up(-90.0), &[("facing", "south")]),
        (
            "oak_log",
            click(Direction::North, 0.5, 0.0, 0.0),
            &[("axis", "z")],
        ),
        (
            "oak_log",
            click(Direction::East, 0.5, 0.0, 0.0),
            &[("axis", "x")],
        ),
        ("oak_log", up(0.0), &[("axis", "y")]),
        ("piston", pitched(-46.0), &[("facing", "down")]),
        ("piston", pitched(-44.0), &[("facing", "west")]),
        ("piston", pitched(46.0), &[("facing", "up")]),
        ("observer", pitched(-46.0), &[("facing", "up")]),
        ("observer", pitched(0.0), &[("facing", "east")]),
        (
            "end_rod",
            click(Direction::South, 0.5, 0.0, 0.0),
            &[("facing", "south")],
        ),
        ("hopper", up(0.0), &[("facing", "down")]),
        (
            "hopper",
            click(Direction::East, 0.5, 0.0, 0.0),
            &[("facing", "west")],
        ),
    ];
    for (name, click, values) in cases {
        assert_eq!(
            placed(name, *click),
            Some(state(name, values)),
            "{name} {click:?}"
        );
    }
}

#[test]
fn plc_sta_005_only_a_water_source_makes_the_placed_state_hold_water() {
    let slab = block("oak_slab");
    let in_water = |level: &str| {
        placed_state(
            blocks(),
            slab,
            &up(0.0),
            state("water", &[("level", level)]),
        )
    };

    assert_eq!(
        in_water("0"),
        Some(state("oak_slab", &[("waterlogged", "true")]))
    );
    assert_eq!(in_water("4"), Some(state("oak_slab", &[])));
    assert_eq!(
        placed("conduit", up(0.0)),
        Some(state("conduit", &[("waterlogged", "false")]))
    );
}

#[test]
fn a_block_of_two_parts_is_not_placed_yet() {
    for name in ["oak_door", "red_bed", "tall_grass"] {
        assert_eq!(placed(name, up(0.0)), None, "{name}");
    }
}

#[test]
fn plc_tgt_002_the_replaceable_blocks_of_result_r13() {
    let dirt = block("dirt");
    let cases: &[(BlockStateId, Id<Block>, Spot, bool)] = &[
        (BlockStateId::AIR, dirt, Spot::Clicked, true),
        (state("short_grass", &[]), dirt, Spot::Clicked, true),
        (state("water", &[("level", "4")]), dirt, Spot::Clicked, true),
        (state("lava", &[]), dirt, Spot::Clicked, true),
        (state("snow", &[("layers", "1")]), dirt, Spot::Clicked, true),
        (
            state("snow", &[("layers", "2")]),
            dirt,
            Spot::Adjacent,
            false,
        ),
        (state("light", &[]), dirt, Spot::Clicked, false),
        (state("light", &[]), dirt, Spot::Adjacent, true),
        (state("sculk_vein", &[]), dirt, Spot::Clicked, true),
        (state("dandelion", &[]), dirt, Spot::Clicked, false),
        (state("torch", &[]), dirt, Spot::Clicked, false),
        (state("stone", &[]), dirt, Spot::Adjacent, false),
        (state("fern", &[]), block("fern"), Spot::Clicked, false),
    ];
    for (target, held, spot, expected) in cases {
        assert_eq!(
            replaceable(blocks(), *target, *held, *spot),
            *expected,
            "{target:?} {spot:?}"
        );
    }
}
