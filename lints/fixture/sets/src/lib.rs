use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Update;
use bevy_ecs::prelude::*;
use bevy_ecs::system::ScheduleSystem;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GatedSystems;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChildSystems {
    Gated,
    Open,
}

#[derive(Resource)]
pub struct Enabled;

pub fn configure(app: &mut App) {
    app.configure_sets(FixedUpdate, GatedSystems.run_if(resource_exists::<Enabled>))
        .configure_sets(FixedUpdate, ChildSystems::Gated.in_set(GatedSystems))
        .configure_sets(FixedUpdate, ChildSystems::Open);
}

pub fn add_every_frame<M>(app: &mut App, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
    app.add_systems(Update, systems);
}

pub fn add_gated<M>(app: &mut App, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
    app.add_systems(Update, systems.run_if(resource_exists::<Enabled>));
}
