use std::mem;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_engine_cell::CellId;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_engine_world::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_resource::ResourceKeyBuf;
use tracing::{debug, warn};

use crate::coordinator::dim_cells::{CellMove, DimCells, DimTicked};
use crate::coordinator::entity_id_ranges::EntityIdRanges;
use crate::coordinator::roster::PlayerRoster;
use crate::coordinator::running_dimensions::RunningDimensions;
use crate::dim_tick::DimOutput;
use crate::to_muxer::{Destination, DimClosed, DimDelivered, RoutedDeparture};

/// Takes what the dimension worlds handed over in their last tick, the cells in the order they
/// ticked: records their column traces, grants the entity id ranges they ask for, keeps the
/// players they report, hands the messages between the cells of a dimension on, applies the
/// clock changes they made, picks the destination of each player that left them, and hands the
/// rest of each dimension, its cells merged, to the muxer.
pub(crate) fn take_dim_outputs(world: &mut World) {
    let taken = world
        .get_resource_mut::<DimCells>()
        .map(|mut cells| cells.take_outputs())
        .unwrap_or_default();
    let mut merged: Vec<DimMerged> = Vec::new();
    let mut moves: Vec<CellMove> = Vec::new();
    for DimTicked {
        label,
        cell,
        mut output,
    } in taken
    {
        if !output.traces.is_empty()
            && let Some(sink) = world.get_resource::<ColumnTraceSink>()
            && let Some(dimension) = world.get::<ResourceKeyBuf<DimensionKey>>(label)
        {
            sink.record(dimension.as_str(), mem::take(&mut output.traces));
        }
        if output.wants_entity_ids
            && let Some(mut ranges) = world.get_resource_mut::<EntityIdRanges>()
        {
            ranges.answer(cell);
        }
        if let Some(mut roster) = world.get_resource_mut::<PlayerRoster>() {
            roster.report(cell, mem::take(&mut output.roster));
        }
        if let Some(mut cells) = world.get_resource_mut::<DimCells>() {
            moves.extend(cells.post(cell, mem::take(&mut output.cell_mail)));
            let gone = output
                .departures
                .iter()
                .map(|departure| departure.session)
                .chain(output.client.released.iter().copied());
            for session in gone {
                cells.forget_session(session);
            }
        }
        match merged.iter_mut().find(|dim| dim.label == label) {
            Some(dim) => dim.output.merge(output),
            None => merged.push(DimMerged { label, output }),
        }
    }
    if let Some(mut roster) = world.get_resource_mut::<PlayerRoster>() {
        for CellMove { session, from, to } in moves {
            roster.move_row(session, from, to);
        }
    }
    for DimMerged { label, mut output } in merged {
        output.client.tiles.coalesce();
        for edit in output.clock_edits {
            world.trigger(edit);
        }
        for edit in output.whitelist_edits {
            world.trigger(edit);
        }
        for edit in output.ban_edits {
            world.trigger(edit);
        }
        for edit in output.server_edits {
            world.trigger(edit);
        }
        let running = world.get_resource::<RunningDimensions>();
        let departures: Vec<RoutedDeparture> = output
            .departures
            .into_iter()
            .map(|departure| RoutedDeparture {
                destination: running.and_then(|running| {
                    running
                        .iter()
                        .find(|found| found.key == departure.to && found.label != label)
                        .map(|found| Destination {
                            label: found.label,
                            key: found.key.clone(),
                            dimension_type: found.dimension_type,
                        })
                }),
                departure,
            })
            .collect();
        world.write_message(DimDelivered {
            dim: label,
            client: output.client,
            departures,
        });
    }
}

/// What the cells of the dimension of `label` handed over in one tick, merged.
struct DimMerged {
    label: Entity,
    output: DimOutput,
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
        let cells: Vec<CellId> = world
            .get_resource::<DimCells>()
            .map(|dim_cells| dim_cells.cells_of(entity).collect())
            .unwrap_or_default();
        let removed = world
            .get_resource_mut::<DimCells>()
            .is_some_and(|mut dim_cells| dim_cells.remove(entity));
        if !removed {
            warn!(?entity, "DimDespawnQueue entry names no dimension world");
        }
        world.write_message(DimClosed { dim: entity });
        if let Some(mut roster) = world.get_resource_mut::<PlayerRoster>() {
            for cell in cells {
                roster.forget(cell);
            }
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
