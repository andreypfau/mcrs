use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_world::entity_births::EntityBirths;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_entity_key::BirthSequences;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::block_at;

use crate::Form;
use crate::item_row::ItemRow;
use crate::launch::Launching;

/// The motion row of an item entity: the plugin, not the pass, ends it (IEN-AGE-001).
pub(crate) fn motion_row() -> MotionRow {
    MotionRow {
        lifetime: None,
        ..MotionRow::item()
    }
}

/// Gives each dropped stack without a row its row: the start of its launch, or its position at
/// rest. A stack dropped without a launch gets the default values (IEN-DAT-002).
pub(crate) fn spawn_rows<M: Send + Sync + 'static>(
    mut commands: Commands,
    mut pending: Query<(Entity, &mut Dropped, Option<&Launching>), (With<Kind>, Without<ItemRow>)>,
    births: EntityBirths,
    mut sequences: Local<BirthSequences>,
    mut ids: ResMut<CellEntityIds>,
    mut rows: ResMut<RowStore<M>>,
) {
    for (stack, mut dropped, launching) in &mut pending {
        let Some(id) = ids.allocate() else {
            tracing::warn!(?stack, "the cell has no entity id left; the item waits");
            continue;
        };
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
        let key = births.born(&source, sequences.next(births.tick(), &source));
        let start = form.start(&KeyedStream::new(&key, births.tick()));
        let body = Body {
            position: start.position,
            velocity: start.velocity,
            ..Body::default()
        };
        let mut state = EntityState::new(key, body, None);
        state.phase = key.phase_offset();
        rows.spawn(id, state, motion_row());
        if dropped.pos != start.position {
            dropped.pos = start.position;
        }
        commands
            .entity(stack)
            .remove::<Launching>()
            .insert(ItemRow { id, key })
            .insert_if_new(ItemEntity::default());
    }
}
