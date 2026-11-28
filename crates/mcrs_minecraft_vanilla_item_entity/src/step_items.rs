use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Populated;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_entity_motion_replication::MotionId;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Lifetime;
use mcrs_minecraft_item_entity::merge_due;
use mcrs_minecraft_physics::block_at;

const VOID_DEPTH: f64 = 64.0;

#[derive(Resource, Debug, Default)]
pub(crate) struct Stepped(pub(crate) Vec<SteppedItem>);

#[derive(Clone, Copy, Debug)]
pub(crate) struct SteppedItem {
    pub(crate) stack: Entity,
    pub(crate) key: EntityUuid,
    pub(crate) position: DVec3,
    pub(crate) merges: bool,
}

pub(crate) fn step_items(
    mut commands: Commands,
    mut items: Query<(Entity, &mut ItemEntity, &MotionState, &StepOutcome), With<MotionId>>,
    dimension: Option<Single<&DimensionTypeConfig, With<Dimension>>>,
    clock: Res<BirthClock>,
    mut stepped: ResMut<Stepped>,
) {
    stepped.0.clear();
    let floor = dimension.map_or(f64::NEG_INFINITY, |config| {
        f64::from(config.min_y) - VOID_DEPTH
    });
    for (stack, mut item, state, outcome) in &mut items {
        let state = &state.0;
        let position = state.body.position;
        let before = if outcome.is_of(clock.0) {
            outcome.start
        } else {
            position
        };
        if before.y < floor {
            commands.entity(stack).despawn();
            continue;
        }
        item.count_down();
        stepped.0.push(SteppedItem {
            stack,
            key: state.key,
            position,
            merges: merge_due(state.age, block_at(before) != block_at(position)),
        });
    }
    stepped.0.sort_unstable_by_key(|item| item.key);
}

pub(crate) fn age_items(
    mut commands: Commands,
    mut items: Populated<(Entity, &mut ItemEntity), With<MotionId>>,
) {
    for (stack, mut item) in &mut items {
        if item.grow() == Lifetime::Ends {
            commands.entity(stack).despawn();
        }
    }
}

pub(crate) fn any_stepped(stepped: Res<Stepped>) -> bool {
    !stepped.0.is_empty()
}

pub(crate) fn project_positions(stepped: Res<Stepped>, mut items: Query<&mut Dropped>) {
    for item in &stepped.0 {
        if let Ok(mut dropped) = items.get_mut(item.stack)
            && dropped.pos != item.position
        {
            dropped.pos = item.position;
        }
    }
}
