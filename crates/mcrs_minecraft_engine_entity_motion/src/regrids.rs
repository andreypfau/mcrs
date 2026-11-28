use bevy_ecs::change_detection::Tick;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemChangeTick;
use bevy_utils::Parallel;
use mcrs_minecraft_physics::GridRow;

use crate::tile_edits::TileEdits;

#[derive(Resource, Default)]
pub(crate) struct Regrids {
    pub(crate) queues: Parallel<TileEdits>,
    pub(crate) stepped: Tick,
    pub(crate) departures: Vec<GridRow>,
}

pub(crate) fn stepped_since(regrids: &Regrids, ticks: &SystemChangeTick) -> bool {
    regrids
        .stepped
        .is_newer_than(ticks.last_run(), ticks.this_run())
}
