use std::sync::Arc;

use bevy_ecs::resource::Resource;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

/// Every dimension the world spawns, in spawn order, with its dimension type.
#[derive(Resource, Clone, Debug)]
pub struct DimensionList {
    keys: Arc<[ResourceKeyBuf<Dimension>]>,
    types: Vec<Id<DimensionType>>,
}

impl DimensionList {
    pub fn new(
        list: impl IntoIterator<Item = (ResourceKeyBuf<Dimension>, Id<DimensionType>)>,
    ) -> Self {
        let (keys, types): (Vec<_>, Vec<_>) = list.into_iter().unzip();
        DimensionList {
            keys: keys.into(),
            types,
        }
    }

    /// Every joining player is handed this one list rather than a copy of it.
    #[must_use]
    pub fn keys(&self) -> &Arc<[ResourceKeyBuf<Dimension>]> {
        &self.keys
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceKeyBuf<Dimension>, Id<DimensionType>)> {
        self.keys.iter().zip(self.types.iter().copied())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}
