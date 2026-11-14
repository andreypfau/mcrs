use bevy_app::{App, FixedUpdate};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_engine_attributes::AttributeEdits;
use mcrs_minecraft_engine_world::tick::TickSet;

/// The systems of the sources of this crate, in `Project` before
/// [`AttributeEdits`](mcrs_minecraft_engine_attributes::AttributeEdits). They edit different
/// attributes, so their order against each other does not change a value.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeSources;

pub(crate) fn configure_attribute_sources(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        AttributeSources
            .in_set(TickSet::Project)
            .before(AttributeEdits),
    );
}
