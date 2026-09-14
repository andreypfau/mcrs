use crate::{AppState, TagFile, TagFileLoader};
use bevy_app::{App, MainScheduleOrder, Plugin, PreUpdate};
use bevy_asset::AssetApp;
use bevy_ecs::prelude::World;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::{NextState, StateTransition};

/// Foundation plugin — registers the `AppState` state machine, the `TagFile`
/// asset type, and the `TagFileLoader`.
///
/// All other `mc_*` plugins depend on this one.
fn run_pending_transition(world: &mut World) {
    if matches!(
        *world.resource::<NextState<AppState>>(),
        NextState::Pending(_)
    ) {
        world.run_schedule(StateTransition);
    }
}

pub struct MinecraftCorePlugin;

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.init_state::<AppState>();
        // The transition schedule costs its sixteen systems every frame whether or not a
        // transition is pending, so it leaves the frame order and runs only on request.
        let transition = StateTransition.intern();
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .labels
            .retain(|label| *label != transition);
        app.add_systems(PreUpdate, run_pending_transition);
        app.init_asset::<TagFile>();
        app.register_asset_loader(TagFileLoader);
    }
}
