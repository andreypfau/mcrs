use bevy_app::{App, FixedUpdate};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_engine::OwnAttributesSet;
use mcrs_minecraft_engine_mob_attributes::AttributeEdits;
use mcrs_minecraft_engine_world::tick::TickSet;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeSources;

pub(crate) fn configure_attribute_sources(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        AttributeSources
            .in_set(TickSet::Project)
            .after(AttributeEdits)
            .before(OwnAttributesSet),
    );
}
