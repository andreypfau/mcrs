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

#[derive(Component)]
pub struct Age(pub u32);

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
pub fn enabled_changed(enabled: Option<Res<Enabled>>) -> bool {
    enabled.is_some_and(|enabled| enabled.is_changed())
}

#[cfg_attr(mcrs_lint, expect(mcrs::ungated_system, reason = "fixture"))]
fn grow(mut ages: Query<&mut Age>) {
    for mut age in &mut ages {
        age.0 += 1;
    }
}

pub fn configure(app: &mut App) {
    app.configure_sets(FixedUpdate, GatedSystems.run_if(resource_exists::<Enabled>))
        .configure_sets(FixedUpdate, ChildSystems::Gated.in_set(GatedSystems))
        .configure_sets(FixedUpdate, ChildSystems::Open)
        .add_systems(FixedUpdate, grow);
}

pub fn add_every_frame<M>(app: &mut App, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
    app.add_systems(Update, systems);
}

pub fn add_gated<M>(app: &mut App, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
    app.add_systems(Update, systems.run_if(resource_exists::<Enabled>));
}
