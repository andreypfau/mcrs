use crate::AppState;
use bevy_app::{App, MainScheduleOrder, Plugin, PreUpdate};
use bevy_ecs::prelude::World;
use bevy_ecs::schedule::{Schedule, ScheduleLabel, SingleThreadedExecutor};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::{NextState, StateTransition};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct PendingStateTransition;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::exclusive_system,
        reason = "runs the state transition schedule, which needs the whole world"
    )
)]
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
        let transition = StateTransition.intern();
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        order.labels.retain(|label| *label != transition);
        order.insert_after(PreUpdate, PendingStateTransition);
        let mut schedule = Schedule::new(PendingStateTransition);
        schedule.set_executor(SingleThreadedExecutor::new());
        app.add_schedule(schedule);
        app.add_systems(PendingStateTransition, run_pending_transition);
    }
}
