//! Vanilla parity of the entity pass: the fixtures of the kernel run through the rows by tile,
//! the freeze, the pass and the tile moves, and each tick gives the states that a vanilla server
//! printed.
#![expect(clippy::unwrap_used, reason = "a malformed fixture fails the test")]

use bevy_tasks::TaskPool;
use bevy_tasks::TaskPoolBuilder;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileGrid;
use mcrs_minecraft_engine_physics::EntityRows;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::BirthKey;
use mcrs_minecraft_entity_key::SourceKind;
use mcrs_minecraft_entity_key::TILE_WIDTH;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_minecraft_volume::SparseVolume;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

#[path = "../../mcrs_minecraft_collision_table/tests/common/mod.rs"]
mod common;
#[path = "../../mcrs_minecraft_collision_table/tests/fixture/mod.rs"]
mod fixture;

use fixture::Failure;
use fixture::Fixture;
use fixture::compare;
use fixture::start_state;

/// The loaded air around an experiment, in sections from its origin; the recorder force-loads
/// the area of every experiment.
const AREA: i32 = 3;

/// The fixtures with a world border or a collider entity, which the world of the pass does not
/// have yet.
const SKIPPED: [&str; 2] = ["e15_border", "e16_shulker"];

fn scene_of(fixture: &Fixture) -> TestScene {
    let mut scene = TestScene {
        blocks: SparseVolume::unbounded(),
        tables: Tables::new(common::table().clone()),
        rows: EntityRows::default(),
        grid: BodyGrid::default(),
        tick: 1,
        push: PushRules::default(),
    };
    let [ox, oy, oz] = fixture.origin.map(|value| value.div_euclid(16));
    for y in -AREA..=AREA {
        for z in -AREA..=AREA {
            for x in -AREA..=AREA {
                let section = SectionPos::new(ox + x, oy + y, oz + z);
                scene.blocks.insert(section, BlockStateId::AIR);
            }
        }
    }
    for (min, max, state) in fixture::block_boxes(fixture) {
        fill(&mut scene, min, max, state);
    }
    scene
}

/// Fills the box and stirs the tile of each block, as a block change of the world does.
fn fill(scene: &mut TestScene, min: BlockPos, max: BlockPos, state: BlockStateId) {
    let tiles = TileGrid {
        x: TILE_WIDTH,
        y: TILE_WIDTH,
        z: TILE_WIDTH,
    };
    for y in min.y..=max.y {
        for z in min.z..=max.z {
            for x in min.x..=max.x {
                scene
                    .blocks
                    .set_block(BlockPos::new(x, y, z), state)
                    .unwrap();
                scene.rows.stir(tiles.tile_at(x, 0, z));
            }
        }
    }
}

/// Spawns each simulated entity of `fixture` as a row and runs the scene one step per recorded
/// tick; gives the first tick that differs for each entity.
fn replay(fixture: &Fixture, pool: &TaskPool, phase: u8) -> Vec<Failure> {
    let mut scene = scene_of(fixture);
    let recorded: Vec<_> = fixture::simulated(fixture).collect();
    let ids: Vec<EntityId> = (1..=recorded.len() as u64).map(EntityId).collect();
    for ((index, (entity, row)), &id) in recorded.iter().enumerate().zip(&ids) {
        let first = entity.ticks.first().copied().flatten().unwrap();
        let key = BirthKey::new(0, SourceKind::Command, 0, index as u32);
        let state = start_state(entity, row, key, &first, phase);
        scene.rows.spawn(id, state, *row);
    }
    let mut failures: Vec<Failure> = vec![None; recorded.len()];
    let ticks = recorded.iter().map(|(entity, _)| entity.ticks.len()).max();
    for tick in 1..ticks.unwrap_or(0) {
        for (min, max, state) in fixture::changes_before(fixture, tick) {
            fill(&mut scene, min, max, state);
        }
        scene.step(pool, 1, &mut EntityWrites::default());
        for (((entity, _), failure), &id) in recorded.iter().zip(&mut failures).zip(&ids) {
            let (None, Some(Some(expected))) = (&failure, entity.ticks.get(tick).copied()) else {
                continue;
            };
            let body = scene
                .rows
                .tiles_mut()
                .flat_map(|(_, rows)| rows.rows_mut())
                .find(|row| row.id == id)
                .map(|row| row.state.body);
            *failure = match body {
                Some(body) => compare(&body, &expected),
                None => Some("the row is gone".to_owned()),
            }
            .map(|text| (tick, text));
        }
    }
    failures
}

#[test]
fn the_pass_follows_the_vanilla_fixtures() {
    let pool = TaskPoolBuilder::new().num_threads(1).build();
    let mut failures = String::new();
    let mut checked = 0;
    for fixture in fixture::fixtures(&mcrs_minecraft_fixtures::fixture("physics").files()) {
        let unsupported = fixture::border_of(&fixture).is_some()
            || fixture.entities.iter().any(|entity| entity.collider);
        assert_eq!(
            unsupported,
            SKIPPED.contains(&fixture.name.as_str()),
            "{}: the skipped fixtures are the ones with a border or a collider",
            fixture.name
        );
        if unsupported {
            continue;
        }
        let runs: Vec<Vec<Failure>> = (0..4).map(|phase| replay(&fixture, &pool, phase)).collect();
        checked += fixture::report(&fixture, &runs, &mut failures);
    }
    println!("{checked} recorded entities");
    assert!(failures.is_empty(), "{failures}");
}
