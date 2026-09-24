use crate::dim::{expire_moves, pump_channels};
use crate::world::bridge::OutboundFlush;
use crate::world::sub_app_builder::{
    ColumnDrain, DimSubAppHandle, drain_dim_despawn_queue, drain_dim_spawn_queue,
};
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use mcrs_minecraft_level::world::sub_app::DimAppLabel;
use std::num::NonZeroU32;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(n) => n,
    None => unreachable!(),
};

pub fn run_server_loop(app: App) {
    mcrs_minecraft_level::server_loop::run_server_loop(
        app,
        DEFAULT_TPS,
        |app| {
            pump_channels(app);
            expire_moves(app);
            drain_dim_spawn_queue(app);
            drain_dim_despawn_queue(app);
        },
        drain_columns,
    );
}

/// Passes on what landed since the tick: every dimension sends the columns whose sections
/// are in, the host bridges them and writes the sockets.
fn drain_columns(app: &mut App) {
    let dims: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<DimSubAppHandle>>()
        .iter(app.world())
        .collect();
    for dim in dims {
        if let Some(sub_app) = app.get_sub_app_mut(DimAppLabel(dim)) {
            sub_app.world_mut().run_schedule(ColumnDrain);
        }
    }
    pump_channels(app);
    app.world_mut().run_schedule(OutboundFlush);
}
