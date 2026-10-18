use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;

use super::Stage;
use super::ctx::Slot;
use crate::registry::Names;
use crate::tags::TagTable;

struct Visible {
    key: ResourceLocation,
    names: Option<Arc<Names>>,
    tags: Option<Arc<TagTable>>,
}

#[doc(hidden)]
pub struct StageView {
    stage: Stage,
    registries: Vec<Visible>,
}

impl StageView {
    pub(crate) fn new(stage: Stage, slots: &[Slot<'_>]) -> Self {
        let registries = slots
            .iter()
            .filter(|slot| slot.stage() <= Some(stage))
            .map(|slot| Visible {
                key: slot.key().clone(),
                names: slot.names.clone(),
                tags: slot.tags.clone(),
            })
            .collect();
        Self { stage, registries }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn registries(&self) -> impl ExactSizeIterator<Item = RegistryView<'_>> {
        self.registries
            .iter()
            .map(|visible| RegistryView { visible })
    }
}

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RegistryView<'a> {
    visible: &'a Visible,
}

impl<'a> RegistryView<'a> {
    pub fn key(&self) -> &'a ResourceLocation {
        &self.visible.key
    }

    pub fn names(&self) -> &'a [ResourceLocation] {
        self.visible.names.as_deref().map_or(&[], Names::all)
    }

    pub fn tags(&self) -> impl Iterator<Item = (u16, &'a ResourceLocation, &'a [u16])> {
        self.visible
            .tags
            .as_deref()
            .into_iter()
            .flat_map(TagTable::resolved)
    }
}
