extern crate std;

use std::sync::LazyLock;

use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_collision_table::BlockWorld;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_minecraft_volume::SparseVolume;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

use crate::spawn_blocks::{SpawnBlocks, SpawnColumnHeights};

const MIN_Y: i32 = 0;
const MAX_Y: i32 = 31;

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let tags = test_registries().tags::<Block>().unwrap();
    Tables::new(collision_table(&test_corpus().0, &tags))
});

fn state_of(text: &str) -> BlockStateId {
    let (name, properties) = text.split_once('[').unwrap_or((text, ""));
    let block = test_corpus().0.block(name).unwrap();
    properties
        .trim_end_matches(']')
        .split(',')
        .filter(|pair| !pair.is_empty())
        .fold(block.default_state_id, |state, pair| {
            let (key, value) = pair.split_once('=').unwrap();
            block.with_text(state, key, value).unwrap()
        })
}

enum Query {
    Column {
        x: i32,
        top: i32,
        surface: i32,
        ocean_floor: i32,
    },
    HeightFix {
        x: i32,
        y: i32,
    },
}

fn volume() -> SparseVolume {
    let mut volume = SparseVolume::unbounded();
    for y in 0..2 {
        volume.insert(SectionPos::new(0, y, 0), BlockStateId::AIR);
    }
    let stone = state_of("minecraft:stone");
    let blocks = [
        (0, 5, "minecraft:stone"),
        (2, 5, "minecraft:stone_slab[type=bottom]"),
        (4, 5, "minecraft:stone"),
        (4, 6, "minecraft:water"),
        (6, 5, "minecraft:oak_leaves"),
        (10, 2, "minecraft:stone"),
        (12, 3, "minecraft:stone"),
        (12, 4, "minecraft:water"),
        (14, 3, "minecraft:stone"),
        (14, 4, "minecraft:seagrass"),
    ];
    for (x, y, block) in blocks {
        volume
            .set_block(BlockPos::new(x, y, 0), state_of(block))
            .unwrap();
    }
    for y in MIN_Y..=MAX_Y {
        volume.set_block(BlockPos::new(8, y, 0), stone).unwrap();
    }
    volume
}

#[test]
fn spawn_placement_follows_the_column_test_and_the_height_fix() {
    let volume = volume();
    let blocks = SpawnBlocks {
        world: BlockWorld {
            blocks: &volume,
            table: &TABLES.collision,
        },
        tables: &TABLES,
        min_y: MIN_Y,
        max_y: MAX_Y,
    };
    let cases = [
        (
            "flat ground",
            Query::Column {
                x: 0,
                top: 5,
                surface: 5,
                ocean_floor: 5,
            },
            Some(DVec3::new(0.5, 6.0, 0.5)),
        ),
        (
            "no full top face",
            Query::Column {
                x: 2,
                top: 5,
                surface: 5,
                ocean_floor: 5,
            },
            None,
        ),
        (
            "water on top",
            Query::Column {
                x: 4,
                top: 6,
                surface: 6,
                ocean_floor: 5,
            },
            None,
        ),
        (
            "leaves on top",
            Query::Column {
                x: 6,
                top: 5,
                surface: 5,
                ocean_floor: 5,
            },
            Some(DVec3::new(6.5, 6.0, 0.5)),
        ),
        (
            "solid to the build limit",
            Query::HeightFix { x: 8, y: 10 },
            Some(DVec3::new(8.5, 31.0, 0.5)),
        ),
        (
            "clear at the minimum height under a ceiling",
            Query::HeightFix { x: 10, y: 0 },
            Some(DVec3::new(10.5, 0.0, 0.5)),
        ),
        (
            "water block counts as a full cube",
            Query::HeightFix { x: 12, y: 10 },
            Some(DVec3::new(12.5, 5.0, 0.5)),
        ),
        (
            "seagrass counts with its collision shape",
            Query::HeightFix { x: 14, y: 10 },
            Some(DVec3::new(14.5, 4.0, 0.5)),
        ),
    ];
    for (name, query, expected) in cases {
        let found = match query {
            Query::Column {
                x,
                top,
                surface,
                ocean_floor,
            } => blocks.column(
                x,
                0,
                SpawnColumnHeights {
                    motion: top,
                    surface,
                    ocean_floor,
                },
            ),
            Query::HeightFix { x, y } => Some(blocks.height_fix(BlockPos::new(x, y, 0))),
        };
        assert_eq!(found, expected, "{name}");
    }
}
