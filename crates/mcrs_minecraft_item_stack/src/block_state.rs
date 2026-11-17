use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::BlockStateProperty;
use crate::entity_form::EntityForm;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct BlockState(Cow<'static, [BlockStateProperty]>);

impl BlockState {
    #[must_use]
    pub fn new(properties: impl IntoIterator<Item = BlockStateProperty>) -> Self {
        let mut sorted: Vec<BlockStateProperty> = properties.into_iter().collect();
        sorted.sort_by(|left, right| left.name.cmp(&right.name));
        let mut unique: Vec<BlockStateProperty> = Vec::with_capacity(sorted.len());
        for property in sorted {
            match unique.last_mut() {
                Some(last) if last.name == property.name => *last = property,
                _ => unique.push(property),
            }
        }
        Self(Cow::Owned(unique))
    }

    #[must_use]
    pub fn properties(&self) -> &[BlockStateProperty] {
        &self.0
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        let index = (self.0)
            .binary_search_by(|entry| entry.name.as_str().cmp(name))
            .ok()?;
        self.0.get(index).map(|entry| entry.value.as_str())
    }
}

impl EntityForm for BlockState {
    fn borrow_static(&'static self) -> Self {
        Self(Cow::Borrowed(&self.0))
    }
}
