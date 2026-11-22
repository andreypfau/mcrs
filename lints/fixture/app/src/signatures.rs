use bevy_ecs::prelude::*;
use fixture_sets::Age;

use crate::tick_rules::Score;

#[derive(Component)]
pub struct Marker;

#[cfg_attr(mcrs_lint, expect(mcrs::borrowed_reborrowable, reason = "fixture"))]
pub fn borrows_commands(_: &mut Commands) {}

pub fn passes_commands(_: Commands) {}

#[cfg_attr(mcrs_lint, expect(mcrs::zst_query, reason = "fixture"))]
pub fn fetches_marker(_: Query<(&Marker, Entity)>) {}

#[cfg_attr(mcrs_lint, expect(mcrs::always_changed_filter, reason = "fixture"))]
pub fn reacts_to_age(_: Query<&Score, Changed<Age>>) {}

pub fn reacts_to_score(_: Query<&Score, Or<(Changed<Score>, Added<Score>)>>) {}

#[cfg_attr(mcrs_lint, expect(mcrs::too_many_arguments, reason = "fixture"))]
pub fn eight(_: u8, _: u8, _: u8, _: u8, _: u8, _: u8, _: u8, _: u8) {}

pub fn many_system_params(
    _: Query<&Score>,
    _: Query<Entity, With<Marker>>,
    _: Commands,
    _: Res<fixture_sets::Enabled>,
    _: Local<u8>,
    _: Query<Entity>,
    _: Query<&Age>,
    _: Single<Entity>,
    _: u8,
) {
}

#[cfg_attr(mcrs_lint, expect(mcrs::unit_in_bundle, reason = "fixture"))]
pub fn spawns_unit(mut commands: Commands) {
    commands.spawn((Score(1), ()));
}

pub fn spawns_bundle(mut commands: Commands) {
    commands.spawn((Score(1), Marker));
}
