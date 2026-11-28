use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Startup;
use bevy_app::Update;
use bevy_ecs::prelude::*;
use fixture_sets::ChildSystems;
use fixture_sets::Enabled;
use fixture_sets::GatedSystems;

#[derive(Component)]
pub struct Player(pub u8);

#[cfg_attr(
    mcrs_lint,
    expect(mcrs::exclusive_system, mcrs::ungated_system, reason = "fixture")
)]
fn drain(_: &mut World) {}

#[cfg_attr(mcrs_lint, expect(mcrs::ungated_system, reason = "fixture"))]
fn step(_: Query<&Player>) {}

fn conditioned(_: Query<&Player>) {}

fn in_gated_set(_: Query<&Player>) {}

fn in_child_of_gated_set(_: Query<&Player>) {}

#[cfg_attr(mcrs_lint, expect(mcrs::ungated_system, reason = "fixture"))]
fn in_open_set(_: Query<&Player>) {}

fn single(_: Single<&Player>) {}

fn setup(_: Commands) {}

#[cfg_attr(mcrs_lint, expect(mcrs::ungated_system, reason = "fixture"))]
fn through_helper(_: Query<&Player>) {}

fn through_gated_helper(_: Query<&Player>) {}

#[cfg_attr(mcrs_lint, expect(mcrs::ungated_system, reason = "fixture"))]
fn in_set_gated_in_another_schedule(_: Query<&Player>) {}

#[cfg_attr(mcrs_lint, expect(mcrs::exclusive_system, reason = "fixture"))]
fn exclusive_at_startup(_: &mut World) {}

pub fn build(app: &mut App) {
    fixture_sets::configure(app);
    app.add_systems(
        FixedUpdate,
        (
            (drain, step).chain(),
            conditioned.run_if(resource_exists::<Enabled>),
            in_gated_set.in_set(GatedSystems),
            in_child_of_gated_set.in_set(ChildSystems::Gated),
            in_open_set.in_set(ChildSystems::Open),
            single,
        ),
    )
    .add_systems(
        Update,
        in_set_gated_in_another_schedule.in_set(GatedSystems),
    )
    .add_systems(Startup, (setup, exclusive_at_startup));
    fixture_sets::add_every_frame(app, through_helper);
    fixture_sets::add_gated(app, through_gated_helper);
}
