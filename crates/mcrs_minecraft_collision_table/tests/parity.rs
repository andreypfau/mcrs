//! Vanilla parity of the tick of server-simulated entities: each fixture under
//! `tests/fixtures/physics` holds the states that a vanilla server printed after each tick of one
//! experiment of the physics specification. `tools/physics-fixtures/record.sh` records them.

use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_entity_key::BirthKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_key::SourceKind;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::ListedBodies;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::tick_entity;

mod common;
mod fixture;

use fixture::Failure;
use fixture::Fixture;
use fixture::Recorded;
use fixture::compare;
use fixture::start_state;

/// A recorded entity that the kernel steps.
struct Simulated<'a> {
    recorded: &'a Recorded,
    row: MotionRow,
    state: EntityState,
    failure: Failure,
}

/// Steps every simulated entity of a fixture together, with the push between the living ones
/// from the state at the start of each tick; records the first tick that differs for each.
fn replay(
    fixture: &Fixture,
    tables: &Tables,
    colliders: &[EntityCollider],
    entities: &mut [Simulated<'_>],
) {
    let mut world = world_of(fixture);
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    let rules = PushRules::default();
    let ticks = entities
        .iter()
        .map(|entity| entity.recorded.ticks.len())
        .max()
        .unwrap_or(0);
    for tick in 1..ticks {
        for (min, max, state) in fixture::changes_before(fixture, tick) {
            world.fill(min, max, state);
        }
        let rows: Vec<GridRow> = entities
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| grid_row(entity, index))
            .collect();
        let bodies = ListedBodies {
            colliders: colliders.to_vec(),
            rows,
        };
        let view = View::new(&world, &bodies, tables);
        for (index, entity) in entities.iter_mut().enumerate() {
            if entity.failure.is_some() {
                continue;
            }
            let Some(Some(expected)) = entity.recorded.ticks.get(tick).copied() else {
                continue;
            };
            let own = grid_row(entity, index);
            actions.clear();
            tick_entity(
                view,
                SERVER,
                &entity.row,
                &mut entity.state,
                true,
                &mut actions,
                &mut scratch,
            );
            if let Some(own) = own {
                let stream = KeyedStream::new(&own.key, tick as u64);
                push_entity(
                    &bodies,
                    &own,
                    &rules,
                    &stream,
                    &mut entity.state.body.velocity,
                    &mut scratch,
                );
            }
            entity.state.age = entity.state.age.wrapping_add(1);
            entity.failure = compare(&entity.state.body, &expected).map(|text| (tick, text));
        }
    }
}

fn grid_row(entity: &Simulated<'_>, index: usize) -> Option<GridRow> {
    entity.row.category.is_living().then(|| {
        let id = EntityId(index as u64 + 1);
        let width = entity.row.dimensions.width;
        GridRow {
            position: entity.state.body.position,
            half_width: width / 2.0,
            height: entity.row.dimensions.height,
            key: BirthKey::new(0, SourceKind::Command, 0, index as u32),
            id,
            root_vehicle: id,
            team: TeamSlot::default(),
            flags: if entity.recorded.kind == "armor_stand" {
                GridFlags::empty()
            } else {
                GridFlags::PUSHABLE
            },
            push_targets: if entity.recorded.kind == "armor_stand" {
                PushTargets::None
            } else {
                PushTargets::Each
            },
        }
    })
}

fn world_of(fixture: &Fixture) -> TestWorld {
    let mut world = TestWorld::new();
    for (min, max, state) in fixture::block_boxes(fixture) {
        world.fill(min, max, state);
    }
    world.set_border(fixture::border_of(fixture));
    world
}

const SERVER: Side = Side::Server { authority: true };

#[test]
fn server_entities_follow_the_vanilla_fixtures() {
    let tables = Tables::new(common::table().clone());
    let mut failures = String::new();
    let mut checked = 0;
    for fixture in fixture::fixtures(&mcrs_minecraft_fixtures::fixture("physics").files()) {
        let colliders: Vec<EntityCollider> = fixture
            .entities
            .iter()
            .filter(|recorded| recorded.collider)
            .filter_map(|recorded| {
                let sample = recorded.ticks.first().copied().flatten()?;
                Some(EntityCollider {
                    bounds: Aabb::of_body(DVec3::from_array(sample.pos), 1.0, 1.0),
                    id: EntityId(1000),
                })
            })
            .collect();
        let runs: Vec<Vec<Failure>> = (0..4)
            .map(|phase| {
                let mut entities: Vec<Simulated<'_>> = fixture::simulated(&fixture)
                    .filter_map(|(recorded, row)| {
                        let first = recorded.ticks.first().copied().flatten()?;
                        let key = BirthKey::new(0, SourceKind::Command, 0, 0);
                        Some(Simulated {
                            recorded,
                            row,
                            state: start_state(recorded, &row, key, &first, phase),
                            failure: None,
                        })
                    })
                    .collect();
                replay(&fixture, &tables, &colliders, &mut entities);
                entities.into_iter().map(|entity| entity.failure).collect()
            })
            .collect();
        checked += fixture::report(&fixture, &runs, &mut failures);
    }
    println!("{checked} recorded entities");
    assert!(failures.is_empty(), "{}", failures);
}
