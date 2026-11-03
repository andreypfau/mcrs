use bevy_ecs::prelude::{Query, With};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::world::dimension::Dimension;

/// The name of the dimension that a dimension world holds, for its logs.
#[derive(SystemParam)]
pub struct DimensionName<'w, 's> {
    keys: Query<
        'w,
        's,
        &'static ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
        With<Dimension>,
    >,
}

impl DimensionName<'_, '_> {
    /// The name, or `"unknown"` in a world that holds no dimension.
    pub fn as_str(&self) -> &str {
        self.keys
            .iter()
            .next()
            .map_or("unknown", ResourceKeyBuf::as_str)
    }
}
