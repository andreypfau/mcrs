use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

use crate::coordinator::dimension_order;

/// The dimensions that run, in the dimension order. The coordinator owns it, and every world
/// gets a copy when it changes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RunningDimensions(Vec<RunningDimension>);

/// A dimension that runs.
#[derive(Clone, Debug, PartialEq)]
pub struct RunningDimension {
    /// The host entity that labels the world of the dimension.
    pub label: Entity,
    pub key: ResourceKeyBuf<DimensionKey>,
    pub dimension_type: Id<DimensionType>,
}

impl RunningDimensions {
    pub fn iter(&self) -> impl Iterator<Item = &RunningDimension> {
        self.0.iter()
    }

    /// The running dimension of `key`.
    pub fn find(&self, key: &ResourceKeyBuf<DimensionKey>) -> Option<&RunningDimension> {
        self.0.iter().find(|dimension| dimension.key == *key)
    }

    pub(crate) fn insert(&mut self, dimension: RunningDimension) {
        self.0.push(dimension);
        self.0
            .sort_by(|a, b| dimension_order::rank(&a.key).cmp(&dimension_order::rank(&b.key)));
    }

    pub(crate) fn remove(&mut self, label: Entity) {
        self.0.retain(|dimension| dimension.label != label);
    }
}
