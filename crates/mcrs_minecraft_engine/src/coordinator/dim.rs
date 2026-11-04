use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_engine_world::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_resource::ResourceKeyBuf;
use tracing::{debug, warn};

use crate::coordinator::destination;
use crate::coordinator::dim_cells::{DimCells, DimTicked};
use crate::coordinator::entity_id_ranges::EntityIdRanges;
use crate::coordinator::roster::PlayerRoster;
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::to_muxer::{DimClosed, DimDelivered, RoutedDeparture};

/// Takes what the dimension worlds handed over in their last tick, the dimensions in the order
/// they ticked: records their column traces, grants the entity id ranges they ask for, keeps
/// the players they report, applies the clock changes they made, picks the destination of each
/// player that left them, and hands the rest to the muxer.
pub(crate) fn take_dim_outputs(world: &mut World) {
    let taken = world
        .get_resource_mut::<DimCells>()
        .map(|mut cells| cells.take_outputs())
        .unwrap_or_default();
    for DimTicked { label, output } in taken {
        if !output.traces.is_empty()
            && let Some(sink) = world.get_resource::<ColumnTraceSink>()
            && let Some(dimension) = world.get::<ResourceKeyBuf<DimensionKey>>(label)
        {
            sink.record(dimension.as_str(), output.traces);
        }
        if output.entity_id_request.is_some()
            && let Some(mut ranges) = world.get_resource_mut::<EntityIdRanges>()
        {
            ranges.answer(label);
        }
        if let Some(mut roster) = world.get_resource_mut::<PlayerRoster>() {
            roster.report(label, output.roster);
        }
        for edit in output.clock_edits {
            world.trigger(edit);
        }
        let departures: Vec<RoutedDeparture> = output
            .departures
            .into_iter()
            .map(|departure| destination::route(world, label, departure))
            .collect();
        world.write_message(DimDelivered {
            dim: label,
            packets: output.packets,
            released: output.released,
            attached: output.attached,
            tiles: output.tiles,
            departures,
            orders: output.orders,
            handoffs: output.handoffs,
        });
    }
}

/// Marks the host entity that labels a dimension world. The entity carries the key of the
/// dimension and no other state: it gives the dimension a host-unique name.
#[derive(bevy_ecs::component::Component)]
pub struct DimSubAppHandle;

/// Stops the dimension worlds of the `DimDespawnQueue`, tells the muxer, and despawns their
/// label entities.
pub(crate) fn drain_dim_despawn_queue(app: &mut App) {
    let Some(mut queue) = app.world_mut().get_resource_mut::<DimDespawnQueue>() else {
        return;
    };
    let entities: Vec<Entity> = std::mem::take(&mut queue.0);
    for entity in entities {
        let world = app.world_mut();
        let removed = world
            .get_resource_mut::<DimCells>()
            .is_some_and(|mut cells| cells.remove(entity));
        if !removed {
            warn!(?entity, "DimDespawnQueue entry names no dimension world");
        }
        world.write_message(DimClosed { dim: entity });
        if let Some(mut roster) = world.get_resource_mut::<PlayerRoster>() {
            roster.forget(entity);
        }
        if let Some(mut running) = world.get_resource_mut::<RunningDimensions>() {
            running.remove(entity);
        }

        // The `OnRemove<DimSubAppHandle>` observer queues an entity that is despawned already.
        match world.get_entity_mut(entity) {
            Ok(entity_mut) => entity_mut.despawn(),
            Err(_) => debug!(
                ?entity,
                "DimDespawnQueue entity already absent from host world (expected on the OnRemove observer path)"
            ),
        }
    }
}
