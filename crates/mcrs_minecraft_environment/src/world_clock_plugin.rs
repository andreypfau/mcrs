use bevy_app::{App, FixedUpdate, Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::world_clocks::{AdvanceTime, WorldClock, WorldClocks};

/// Holds a state for each clock of the loaded registries and advances the clocks in each
/// fixed tick while [`AdvanceTime`] is true.
pub struct WorldClockPlugin;

/// The system that advances the clocks, in `FixedUpdate`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorldClockSystems;

impl Plugin for WorldClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClocks>()
            .init_resource::<AdvanceTime>()
            .add_systems(
                PreUpdate,
                seed_clocks.run_if(resource_exists_and_changed::<CurrentRegistries>),
            )
            .add_systems(FixedUpdate, advance_clocks.in_set(WorldClockSystems));
    }
}

fn seed_clocks(mut clocks: ResMut<WorldClocks>, registries: Res<CurrentRegistries>) {
    let count = registries
        .registry::<WorldClock>()
        .map_or(0, |registry| registry.len());
    if clocks.len() != count {
        clocks.reconcile(count);
    }
}

fn advance_clocks(mut clocks: ResMut<WorldClocks>, advance_time: Res<AdvanceTime>) {
    if !advance_time.0 {
        return;
    }
    for (_, state) in clocks.iter_mut() {
        state.advance();
    }
}
