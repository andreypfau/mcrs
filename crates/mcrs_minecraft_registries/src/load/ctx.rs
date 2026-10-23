use std::collections::HashMap;
use std::sync::Arc;

use mcrs_minecraft_core::{
    Id, Registered, RegistryValue, ResourceKey, ResourceLocation, TagId, TagKey,
};

use super::declaration::Declaration;
use crate::registries::{Erased, Stage, key_of};
use crate::refs::{MissingRef, ResolveError, Resolver};
use crate::registry::{Names, Registry};
use crate::tags::{TagTable, Tags};

pub(crate) struct Slot<'d> {
    pub(crate) declaration: &'d Declaration,
    pub(crate) names: Option<Arc<Names>>,
    pub(crate) tags: Option<Arc<TagTable>>,
    pub(crate) registry: Option<Erased>,
}

impl Slot<'_> {
    pub(crate) fn key(&self) -> &ResourceLocation {
        self.declaration.key()
    }

    pub(crate) fn stage(&self) -> Stage {
        self.declaration.stage()
    }
}

pub struct LoadCtx<'a> {
    slots: &'a [Slot<'a>],
    by_key: &'a HashMap<ResourceLocation, usize>,
    stage: Stage,
}

impl<'a> LoadCtx<'a> {
    pub(crate) fn new(
        slots: &'a [Slot<'a>],
        by_key: &'a HashMap<ResourceLocation, usize>,
        stage: Stage,
    ) -> Self {
        Self {
            slots,
            by_key,
            stage,
        }
    }

    fn slot<R: Registered>(&self) -> Option<&'a Slot<'a>> {
        self.slots.get(*self.by_key.get(key_of::<R>())?)
    }

    pub fn registry<V: RegistryValue + 'static>(&self) -> Option<&Registry<V>> {
        let slot = self.slot::<V::Registry>()?;
        slot.registry.as_ref()?.downcast_ref::<Registry<V>>()
    }

    pub fn tags<R: Registered>(&self) -> Option<Tags<R>> {
        let slot = self.slot::<R>()?;
        slot.tags.as_ref().map(|table| Tags::new(Arc::clone(table)))
    }

    fn visible<R: Registered>(&self) -> Result<&'a Slot<'a>, ResolveError> {
        let Some(slot) = self.slot::<R>() else {
            return Err(ResolveError::new("the referenced registry is not declared"));
        };
        match slot.stage() > self.stage {
            true => Err(ResolveError::new(format!(
                "{} loads after this entry, so it cannot be referenced here",
                slot.key()
            ))),
            false => Ok(slot),
        }
    }

    // Only a registry of the stage being loaded reports its misses together; an earlier one is
    // complete, so a miss there is an error of the entry that names it.
    fn miss(&self, slot: &Slot<'_>, name: &ResourceLocation, tag: bool) -> ResolveError {
        let message = match tag {
            true => format!("unknown tag #{name} of {}", slot.key()),
            false => format!("unknown entry {name} of {}", slot.key()),
        };
        if slot.stage() != self.stage {
            return ResolveError::new(message);
        }
        let missing = MissingRef {
            registry: slot.key().clone(),
            name: name.clone(),
            tag,
        };
        ResolveError::missing(missing, message)
    }
}

impl Resolver for LoadCtx<'_> {
    fn id<R: Registered>(&self, key: &ResourceKey<R>) -> Result<Id<R>, ResolveError> {
        let slot = self.visible::<R>()?;
        let names = slot.names.as_ref().ok_or_else(|| not_loaded(slot))?;
        names
            .id(key.location())
            .ok_or_else(|| self.miss(slot, key.location(), false))
    }

    fn tag<R: Registered>(&self, tag: &TagKey<R>) -> Result<TagId<R>, ResolveError> {
        let slot = self.visible::<R>()?;
        let tags = slot.tags.as_ref().ok_or_else(|| not_loaded(slot))?;
        tags.id(tag.as_str())
            .ok_or_else(|| self.miss(slot, tag.location(), true))
    }
}

fn not_loaded(slot: &Slot<'_>) -> ResolveError {
    ResolveError::new(format!("{} is not loaded yet", slot.key()))
}
