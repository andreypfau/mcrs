use bevy_ecs::prelude::*;
use mcrs_minecraft_client_world::columns::{ColumnStore, Columns};
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::ClientEntity;
use mcrs_voxel::{BlockStateId, SectionPos};

use crate::add_entity_data::AddEntityData;
use crate::client_tables::ClientTables;
use crate::fuse::Fuse;
use crate::item_entity_stack::ItemEntityStack;
use crate::motion_tick::MotionTick;
use crate::network_entities::NetworkEntities;

const BELOW_WORLD: f64 = 64.0;

pub(crate) fn remove_spent_entities(
    mut commands: Commands,
    mut network: ResMut<NetworkEntities>,
    tick: Res<MotionTick>,
    tables: Res<ClientTables>,
    columns: Res<ColumnStore>,
    entities: Query<(
        Entity,
        &ClientEntity,
        &StepOutcome,
        Option<&AddEntityData>,
        Option<&Fuse>,
        Option<&ItemEntityStack>,
    )>,
) {
    let lowest = columns
        .extent()
        .map(|extent| f64::from(extent.min_section_y) * SectionPos::SIZE as f64 - BELOW_WORLD);
    let below_world = |outcome: &StepOutcome| lowest.is_some_and(|lowest| outcome.start.y < lowest);
    for (entity, client, outcome, data, fuse, stack) in &entities {
        if !outcome.is_of(tick.0) {
            continue;
        }
        let spent = match client.kind {
            EntityType::Item => {
                stack.is_none_or(|stack| stack.0.is_empty()) || below_world(outcome)
            }
            EntityType::FallingBlock => {
                let state = data
                    .and_then(|data| u16::try_from(data.0).ok())
                    .and_then(|number| BlockStateId::checked(number, tables.0.collision.len()));
                state.is_none_or(|state| state == BlockStateId::AIR)
            }
            EntityType::Tnt => fuse.is_some_and(|fuse| fuse.0 <= 0),
            EntityType::ExperienceOrb => below_world(outcome),
            _ => false,
        };
        if !spent {
            continue;
        }
        if network.get(client.network_id) == Some(entity) {
            network.remove(client.network_id);
        }
        commands.entity(entity).despawn();
    }
}
