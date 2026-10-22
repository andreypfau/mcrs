use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use mcrs_minecraft_core::{Id, ResourceLocation, TagId};
use serde::de::{DeserializeSeed, Deserializer};

use super::declaration::Erased;
use super::{Stage, Table};
use crate::diagnostics::{Missing, Referrer};
use crate::id::Key;
use crate::refs::Resolver;
use crate::registry::{Names, ReadValue, Registry, RegistryType};
use crate::tags::{TagTable, Tags};

pub(crate) struct Slot<'d> {
    pub(crate) declaration: &'d dyn Erased,
    pub(crate) names: Option<Arc<Names>>,
    pub(crate) tags: Option<Arc<TagTable>>,
    pub(crate) table: Option<Table>,
}

impl Slot<'_> {
    pub(crate) fn key(&self) -> &ResourceLocation {
        self.declaration.key()
    }

    pub(crate) fn stage(&self) -> Option<Stage> {
        self.declaration.kind().stage()
    }
}

pub struct LoadCtx<'a> {
    slots: &'a [Slot<'a>],
    by_type: &'a HashMap<TypeId, usize>,
    stage: Option<Stage>,
    referrer: RefCell<Option<(ResourceLocation, ResourceLocation)>>,
    unresolved: RefCell<Vec<(usize, Missing)>>,
    pending: Cell<Option<usize>>,
}

impl<'a> LoadCtx<'a> {
    pub(crate) fn new(
        slots: &'a [Slot<'a>],
        by_type: &'a HashMap<TypeId, usize>,
        stage: Option<Stage>,
    ) -> Self {
        Self {
            slots,
            by_type,
            stage,
            referrer: RefCell::new(None),
            unresolved: RefCell::new(Vec::new()),
            pending: Cell::new(None),
        }
    }

    pub(crate) fn enter(&self, registry: &ResourceLocation, entry: &ResourceLocation) -> usize {
        *self.referrer.borrow_mut() = Some((registry.clone(), entry.clone()));
        self.pending.set(None);
        self.unresolved.borrow().len()
    }

    // A miss stays pending only while its error unwinds: any later reference attempt, or a
    // reference seed that succeeds, means the value carried on past it.
    // chisle: a recovered miss followed by a failure raised outside every reference seed is still
    // taken as the cause; observing every deserializer call (a wrapping Deserializer) lifts it.
    fn attempt<T, E>(&self, read: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        self.pending.set(None);
        let result = read();
        if result.is_ok() {
            self.pending.set(None);
        }
        result
    }

    pub(crate) fn settle(&self, mark: usize, failed: bool) -> bool {
        let pending = self.pending.take().filter(|&at| failed && at >= mark);
        let mut misses = self.unresolved.borrow_mut();
        let cause = pending.map(|at| misses.swap_remove(at));
        misses.truncate(mark);
        misses.extend(cause);
        pending.is_some()
    }

    pub(crate) fn into_unresolved(self) -> Vec<(usize, Missing)> {
        self.unresolved.into_inner()
    }

    pub fn value<T: ReadValue>(&self) -> ValueSeed<'_, 'a, T> {
        ValueSeed {
            ctx: self,
            value: PhantomData,
        }
    }

    pub fn registry<R: RegistryType>(&self) -> Option<&Registry<R>> {
        let slot = self.slots.get(*self.by_type.get(&TypeId::of::<R>())?)?;
        slot.table.as_ref()?.registry.downcast_ref::<Registry<R>>()
    }

    pub fn tags<R: RegistryType>(&self) -> Option<Tags<R>> {
        let slot = self.slots.get(*self.by_type.get(&TypeId::of::<R>())?)?;
        slot.tags.as_ref().map(|table| Tags::new(Arc::clone(table)))
    }

    fn visible<R: 'static>(&self) -> Result<(usize, &'a Slot<'a>), String> {
        let declared = self
            .by_type
            .get(&TypeId::of::<R>())
            .and_then(|&index| Some((index, self.slots.get(index)?)));
        let Some((index, slot)) = declared else {
            return Err("the referenced registry is not declared".to_owned());
        };
        match slot.stage() > self.stage {
            true => Err(format!(
                "{} loads after this entry, so it cannot be referenced here",
                slot.key()
            )),
            false => Ok((index, slot)),
        }
    }

    fn miss(&self, index: usize, slot: &Slot<'_>, name: &ResourceLocation, tag: bool) -> String {
        let referrer = self.referrer.borrow().clone();
        if slot.stage() == self.stage
            && let Some((registry, entry)) = referrer
        {
            let mut misses = self.unresolved.borrow_mut();
            self.pending.set(Some(misses.len()));
            misses.push((
                index,
                Missing {
                    name: name.clone(),
                    tag,
                    referrer: Referrer::Entry { registry, entry },
                },
            ));
        }
        match tag {
            true => format!("unknown tag #{name} of {}", slot.key()),
            false => format!("unknown entry {name} of {}", slot.key()),
        }
    }

    fn resolve_id<R: RegistryType>(&self, name: &ResourceLocation) -> Result<Id<R>, String> {
        let (index, slot) = self.visible::<R>()?;
        let names = slot.names.as_ref().ok_or_else(|| not_loaded(slot))?;
        names
            .get(name)
            .and_then(|number| Id::checked(number, names.len()))
            .ok_or_else(|| self.miss(index, slot, name, false))
    }

    fn resolve_tag<R: RegistryType>(&self, name: &ResourceLocation) -> Result<TagId<R>, String> {
        let (index, slot) = self.visible::<R>()?;
        let tags = slot.tags.as_ref().ok_or_else(|| not_loaded(slot))?;
        tags.get(name)
            .and_then(|number| TagId::checked(number, tags.len()))
            .ok_or_else(|| self.miss(index, slot, name, true))
    }
}

impl Resolver for LoadCtx<'_> {
    fn id<R: RegistryType>(&self, key: &Key<R>) -> Result<Id<R>, String> {
        self.attempt(|| self.resolve_id(key.location()))
    }

    fn tag<R: RegistryType>(&self, tag: &Key<R>) -> Result<TagId<R>, String> {
        self.attempt(|| self.resolve_tag(tag.location()))
    }
}

fn not_loaded(slot: &Slot<'_>) -> String {
    format!("{} is not loaded yet", slot.key())
}

pub struct ValueSeed<'c, 'a, T> {
    ctx: &'c LoadCtx<'a>,
    value: PhantomData<fn() -> T>,
}

impl<'de, T: ReadValue> DeserializeSeed<'de> for ValueSeed<'_, '_, T> {
    type Value = T;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
        self.ctx.attempt(|| T::read(self.ctx, deserializer))
    }
}
