use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_client_world::columns::{ColumnChange, ColumnStore};
use mcrs_minecraft_collision_table::BlockWorld;
use mcrs_minecraft_engine_entity_motion::{
    Crowding, MotionHold, MotionState, MotionStep, StirredTiles,
};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_visual::{Age, ClientEntity};
use mcrs_minecraft_physics::Side;

use crate::client_tables::ClientTables;
use crate::motion_tick::MotionTick;
use crate::ticked::ticked;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::marker_toggle,
        reason = "the motion engine defines `MotionHold` as the marker that keeps an entity out of its step query"
    )
)]
pub(crate) fn hold_entities_that_do_not_step(
    mut commands: Commands,
    entities: Populated<(Entity, &ClientEntity, Ref<Age>, Has<MotionHold>), With<MotionState>>,
) {
    for (entity, client, age, held) in &entities {
        let first_orb_tick = client.kind == EntityType::ExperienceOrb && age.0 == 1;
        let steps = ticked(&age) && !first_orb_tick;
        match (steps, held) {
            (true, true) => {
                commands.entity(entity).remove::<MotionHold>();
            }
            (false, false) => {
                commands.entity(entity).insert(MotionHold);
            }
            _ => {}
        }
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "the system clears the stirred tiles of the previous tick, also when no block changed"
    )
)]
pub(crate) fn stir_changed_blocks(
    mut changes: MessageReader<ColumnChange>,
    mut stirred: ResMut<StirredTiles>,
) {
    let tiles = changes.read().filter_map(|change| match change {
        ColumnChange::Reblocked(at, _) => Some(TileKey::at(at.as_dvec3())),
        ColumnChange::Arrived(column, _) => Some(TileKey::at(DVec3::new(
            f64::from(column.min_block_x()),
            0.0,
            f64::from(column.min_block_z()),
        ))),
        _ => None,
    });
    stirred.replace(tiles);
}

pub(crate) fn step_simulated_entities(
    columns: Res<ColumnStore>,
    tables: Res<ClientTables>,
    tick: Res<MotionTick>,
    mut step: MotionStep,
) {
    let world = BlockWorld {
        blocks: &*columns,
        table: &tables.0.collision,
    };
    step.run(
        &world,
        &tables.0,
        Side::Viewer,
        tick.0,
        &Crowding::default(),
    );
}
