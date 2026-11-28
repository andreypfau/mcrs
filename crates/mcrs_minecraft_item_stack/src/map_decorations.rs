use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::MapDecoration;
use crate::entity_form::EntityForm;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct MapDecorations(Cow<'static, [MapDecoration]>);

impl MapDecorations {
    #[must_use]
    pub fn new(decorations: impl IntoIterator<Item = MapDecoration>) -> Option<Self> {
        let mut decorations: Vec<MapDecoration> = decorations.into_iter().collect();
        decorations.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        let is_unique = decorations.is_sorted_by(|left, right| left.key < right.key);
        is_unique.then_some(Self(Cow::Owned(decorations)))
    }

    #[must_use]
    pub fn decorations(&self) -> &[MapDecoration] {
        &self.0
    }
}

impl EntityForm for MapDecorations {
    fn borrow_static(&'static self) -> Self {
        Self(Cow::Borrowed(&self.0))
    }
}
