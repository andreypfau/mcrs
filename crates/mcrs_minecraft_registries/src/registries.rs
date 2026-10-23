use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use mcrs_minecraft_core::{
    Id, Registered, RegistryValue, ResourceKey, ResourceLocation, TagId, TagKey,
};
use mcrs_minecraft_datapack::{KnownPack, PackStack, StackedPack};

use crate::refs::{Namer, ResolveError, Resolver};
use crate::registry::{Entries, Names, Registry};
use crate::tags::{TagTable, Tags};

pub(crate) fn key_of<R: Registered>() -> &'static str {
    R::REGISTRY.location().as_static_str()
}

/// When a registry loads. A registry sees the entries of its own stage and of every earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// The static registries, whose entries the game itself defines.
    Static,
    World,
    Dimension,
    Reloadable,
}

impl Stage {
    pub(crate) const ALL: [Stage; 4] = [
        Stage::Static,
        Stage::World,
        Stage::Dimension,
        Stage::Reloadable,
    ];
}

pub(crate) type Erased = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Table {
    pub(crate) stage: Stage,
    pub(crate) names: Arc<Names>,
    pub(crate) tags: Arc<TagTable>,
    /// `None` for a static registry and one read by its names alone.
    pub(crate) registry: Option<Erased>,
}

#[derive(Clone)]
pub struct Registries {
    pub(crate) tables: Arc<HashMap<ResourceLocation, Table>>,
    pub(crate) world_stack: Arc<PackStack>,
    pub(crate) reload_stack: Arc<PackStack>,
}

impl Registries {
    pub fn registry<V: RegistryValue + 'static>(&self) -> Option<&Registry<V>> {
        self.table(key_of::<V::Registry>())?
            .registry
            .as_ref()?
            .downcast_ref::<Registry<V>>()
    }

    /// The entries of the registry `R` names, whether it holds values or not.
    pub fn entries<R: Registered>(&self) -> Option<Entries<'_, R>> {
        Some(Entries::new(&self.table(key_of::<R>())?.names))
    }

    /// The entry names of the registry `key`, in id order, for a caller that has no type for it.
    pub fn names_of(&self, key: &ResourceLocation) -> Option<&[ResourceLocation]> {
        self.table(key.as_str()).map(|table| table.names.all())
    }

    /// Each resolved tag of the registry `key` with its member ids, for a caller that has no type
    /// for it.
    pub fn tags_of(
        &self,
        key: &ResourceLocation,
    ) -> Option<impl Iterator<Item = (&ResourceLocation, &[u16])>> {
        let table = self.table(key.as_str())?;
        Some(table.tags.resolved().map(|(_, name, members)| (name, members)))
    }

    pub fn tags<R: Registered>(&self) -> Option<Tags<R>> {
        let table = self.table(key_of::<R>())?;
        Some(Tags::new(Arc::clone(&table.tags)))
    }

    pub fn origin<R: Registered>(&self, id: Id<R>) -> Option<&StackedPack> {
        let table = self.table(key_of::<R>())?;
        let stack = match table.stage {
            Stage::Reloadable => &self.reload_stack,
            _ => &self.world_stack,
        };
        stack.get(table.names.origin(id.number())?)
    }

    pub fn known_packs(&self) -> impl Iterator<Item = &KnownPack> {
        self.world_stack.known_packs()
    }

    /// Whether `a` and `b` are the same load, rather than two loads of equal data.
    pub fn ptr_eq(a: &Registries, b: &Registries) -> bool {
        Arc::ptr_eq(&a.tables, &b.tables)
    }

    pub(crate) fn table(&self, key: &str) -> Option<&Table> {
        self.tables.get(key)
    }
}

impl Namer for Registries {
    fn name<R: Registered>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.table(key_of::<R>())?.names.name(id.number())
    }

    fn tag_name<R: Registered>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.table(key_of::<R>())?.tags.name(tag.index())
    }
}

impl Resolver for Registries {
    fn id<R: Registered>(&self, key: &ResourceKey<R>) -> Result<Id<R>, ResolveError> {
        let table = self.table(key_of::<R>()).ok_or_else(unbound::<R>)?;
        table
            .names
            .id(key.location())
            .ok_or_else(|| ResolveError::new(format!("unknown entry {key} of {}", R::REGISTRY)))
    }

    fn tag<R: Registered>(&self, tag: &TagKey<R>) -> Result<TagId<R>, ResolveError> {
        let table = self.table(key_of::<R>()).ok_or_else(unbound::<R>)?;
        table.tags.id(tag.as_str()).ok_or_else(|| {
            ResolveError::new(format!("unknown tag #{} of {}", tag.as_str(), R::REGISTRY))
        })
    }
}

fn unbound<R: Registered>() -> ResolveError {
    ResolveError::new(format!("registry {} is not loaded", R::REGISTRY))
}

impl fmt::Debug for Registries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registries")
            .field("registries", &self.tables.len())
            .finish()
    }
}
