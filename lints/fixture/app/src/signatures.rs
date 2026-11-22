use bevy_ecs::prelude::*;

#[derive(Component)]
pub struct Marker;

#[cfg_attr(mcrs_lint, expect(mcrs::borrowed_reborrowable, reason = "fixture"))]
pub fn borrows_commands(_: &mut Commands) {}

pub fn passes_commands(_: Commands) {}

#[cfg_attr(mcrs_lint, expect(mcrs::zst_query, reason = "fixture"))]
pub fn fetches_marker(_: Query<(&Marker, Entity)>) {}
