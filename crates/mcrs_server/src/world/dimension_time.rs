use bevy_app::{FixedPostUpdate, Plugin};
use bevy_app::prelude::FixedLast;
use bevy_ecs::change_detection::ResMut;
use bevy_ecs::prelude::{Component, Resource};
use bevy_reflect::Reflect;

pub struct DimensionTimePlugin;

impl Plugin for DimensionTimePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.insert_resource(DimensionTime(0));
        app.add_systems(FixedPostUpdate, update_time);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Reflect, Resource)]
pub struct DimensionTime(pub u64);

fn update_time(
    mut dimension_time: ResMut<DimensionTime>,
) {
    dimension_time.0 = dimension_time.0.wrapping_add(1);
}