use crate::tick::TickSet;
use crate::world::lifecycle::ticket::TicketPlugin;
use crate::world::storage::column::ColumnPlugin;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::IntoScheduleConfigs;

pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                TickSet::DomainInput,
                TickSet::ClientInput,
                TickSet::Simulate,
                TickSet::WorldWrite,
                TickSet::Project,
                TickSet::Output,
            )
                .chain(),
        );
        app.add_plugins(TicketPlugin);
        app.add_plugins(ColumnPlugin);
    }
}
