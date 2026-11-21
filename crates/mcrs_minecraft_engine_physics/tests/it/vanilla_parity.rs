#![expect(clippy::unwrap_used, reason = "a malformed fixture fails the test")]

use bevy_ecs::entity::Entity;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_physics::Tables;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

use crate::common;
use crate::fixture;

use fixture::Failure;
use fixture::Fixture;
use fixture::compare;
use fixture::start_state;

const AREA: i32 = 3;

const SKIPPED: [&str; 2] = ["e15_border", "e16_shulker"];

fn scene_of(fixture: &Fixture) -> TestScene {
    let mut scene = TestScene::empty(Tables::new(common::table().clone()));
    scene.tick = 1;
    let [ox, oy, oz] = fixture.origin.map(|value| value.div_euclid(16));
    for y in -AREA..=AREA {
        for z in -AREA..=AREA {
            for x in -AREA..=AREA {
                let section = SectionPos::new(ox + x, oy + y, oz + z);
                scene.load_section(section, BlockStateId::AIR);
            }
        }
    }
    for (min, max, state) in fixture::block_boxes(fixture) {
        fill(&mut scene, min, max, state);
    }
    scene
}

fn fill(scene: &mut TestScene, min: BlockPos, max: BlockPos, state: BlockStateId) {
    for y in min.y..=max.y {
        for z in min.z..=max.z {
            for x in min.x..=max.x {
                scene.place_block(BlockPos::new(x, y, z), state).unwrap();
            }
        }
    }
}

fn replay(fixture: &Fixture, phase: u8) -> Vec<Failure> {
    let mut scene = scene_of(fixture);
    let recorded: Vec<_> = fixture::simulated(fixture).collect();
    let mut entities: Vec<Entity> = Vec::new();
    for (index, (entity, row)) in recorded.iter().enumerate() {
        let first = entity.ticks.first().copied().flatten().unwrap();
        let key = EntityUuid::from_u128(u128::from(index as u32));
        let state = start_state(entity, row, key, &first, phase);
        entities.push(scene.spawn(EntityId(index as u64 + 1), state, *row));
    }
    let mut failures: Vec<Failure> = vec![None; recorded.len()];
    let ticks = recorded.iter().map(|(entity, _)| entity.ticks.len()).max();
    for tick in 1..ticks.unwrap_or(0) {
        for (min, max, state) in fixture::changes_before(fixture, tick) {
            fill(&mut scene, min, max, state);
        }
        scene.step();
        for (((entity, _), failure), &spawned) in recorded.iter().zip(&mut failures).zip(&entities)
        {
            let (None, Some(Some(expected))) = (&failure, entity.ticks.get(tick).copied()) else {
                continue;
            };
            *failure = match scene.state(spawned) {
                Some(state) => compare(&state.body, &expected),
                None => Some("the entity is gone".to_owned()),
            }
            .map(|text| (tick, text));
        }
    }
    failures
}

#[test]
fn the_pass_follows_the_vanilla_fixtures() {
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
        let runs: Vec<Vec<Failure>> = (0..4).map(|phase| replay(&fixture, phase)).collect();
        checked += fixture::report(&fixture, &runs, &mut failures);
    }
    println!("{checked} recorded entities");
    assert!(failures.is_empty(), "{failures}");
}
