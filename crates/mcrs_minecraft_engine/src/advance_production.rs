use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Local, Query, Res, ResMut};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::player::chunk_view::{ChunkTrackingView, SentChunkView};
use mcrs_minecraft_engine_world::voxel_update::BlockChange;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::light_gate::LightGated;
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_voxel::ColumnPos;

use crate::column_production::{ColumnProduction, Lights};
use crate::column_snapshot::ColumnSnapshots;
use crate::column_store::ColumnStoreLink;
use crate::column_streams::ColumnStreams;
use crate::foreign_views::ForeignViews;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "drains the column store answers, which arrive from another thread without a change in the world"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the halo changes move out of the production state each run, and the loaded section data moves on to the column store and the light engine"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(4))]
pub(crate) fn advance_production(
    mut production: ResMut<ColumnProduction>,
    mut store: ResMut<ColumnStoreLink>,
    dims: Query<(
        &ColumnLoading,
        &ColumnIndex,
        &DimensionTypeConfig,
        Option<&ColumnStreams>,
    )>,
    snapshots: ColumnSnapshots,
    sent_views: Query<&SentChunkView>,
    foreign_views: Option<Res<ForeignViews>>,
    mut placed: MessageReader<BlockPlaced>,
    light_gated: Option<Res<LightGated>>,
    mut pending: Local<Vec<ColumnPos>>,
    mut views: Local<Vec<ChunkTrackingView>>,
) {
    let lights = match light_gated {
        Some(_) => Lights::Yes,
        None => Lights::No,
    };
    let store = &mut *store;
    let dim = dims.iter().next();
    let dim_row = dim.map(|(loading, index, rows, _)| (loading, index, rows));
    let loading = dim.map(|(loading, ..)| loading);
    views.extend(sent_views.iter().filter_map(|view| view.0));
    views.extend(foreign_views.iter().flat_map(|foreign| foreign.views()));
    production.screen(&mut views, dim.map(|(_, _, rows, _)| rows));
    let lit_by_data_layer = |column: ColumnPos| {
        dim.is_some_and(|(loading, _, _, streams)| {
            streams.is_some_and(|streams| streams.holds(column)) && !loading.lights(column)
        })
    };
    let halo_changes = production.take_halo_changes();
    let changes = placed
        .read()
        .map(|placed| BlockChange {
            pos: placed.block_pos,
            state: placed.new_state,
        })
        .chain(halo_changes);
    production.note_block_changes(changes, lit_by_data_layer, store, lights);
    production.take_store_answers(store, lights);
    production.take_snapshots(dim_row, &snapshots, store, lights, &mut pending);
    production.take_light(loading, store);
    production.move_wanted(loading, store, lights, &mut pending);
}
