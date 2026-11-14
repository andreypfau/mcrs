use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::entity_births::EntityBirths;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::BirthSequences;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_replication::SpawnLook;

use crate::Form;
use crate::item_spawn_request::FixedKey;
use crate::launch::Launching;
use crate::wire_stack::wire_stack_of;

/// The motion row of an item entity: the plugin, not the pass, ends it (IEN-AGE-001).
fn motion_row() -> MotionRow {
    MotionRow {
        lifetime: None,
        ..MotionRow::item()
    }
}

/// Gives each dropped stack without a motion its motion: the start of its launch, or its position
/// at rest. A stack dropped without a launch gets the default values (IEN-DAT-002). A stack
/// without a wire form gets no appearance, so it never moves.
pub(crate) fn spawn_motions(
    mut commands: Commands,
    pending: Query<
        (Entity, &Dropped, Option<&Launching>, Option<&FixedKey>),
        (With<Kind>, Without<MotionState>),
    >,
    births: EntityBirths,
    mut sequences: Local<BirthSequences>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
) {
    for (stack, dropped, launching, fixed) in &pending {
        let (source, form) = match launching {
            Some(launching) => (launching.source, launching.form),
            None => (
                BirthSource::Block(block_at(dropped.pos)),
                Form::Fixed(Start {
                    position: dropped.pos,
                    velocity: DVec3::ZERO,
                }),
            ),
        };
        let key = match fixed {
            Some(fixed) => fixed.0,
            None => births.born(&source, sequences.next(births.tick(), &source)),
        };
        let start = form.start(&KeyedStream::new(&key, births.tick()));
        let body = Body {
            position: start.position,
            velocity: start.velocity,
            ..Body::default()
        };
        let mut state = EntityState::new(key, body, None);
        state.phase = key.phase_offset();
        let mut entity = commands.entity(stack);
        entity
            .remove::<(Launching, FixedKey)>()
            .insert((MotionState(state), MotionKind(motion_row())))
            .insert_if_new(ItemEntity::default());
        if let Some(wire) = wire_stack_of(&mut stacks, &table, stack) {
            entity.insert(Appearance {
                kind: &EntityTypeData::ITEM,
                look: SpawnLook::Item(wire),
            });
        }
    }
}
