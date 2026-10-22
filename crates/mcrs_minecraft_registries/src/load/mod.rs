mod ctx;
mod declaration;
mod index;
mod loader;
mod network;
mod received;
mod tag_build;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

pub use ctx::{LoadCtx, ValueSeed};
pub use declaration::{DataDeclaration, Declaration};
pub use loader::load;
pub use network::NetworkSnapshot;
pub use received::{Received, load_received};
pub use tag_build::TagEntry;

use crate::id::Key;
use crate::pack::{KnownPack, PackStack, StackedPack};
use crate::refs::{Namer, Resolver};
use crate::registry::{Names, Registry, RegistryType};
use crate::tags::{TagTable, Tags};
use mcrs_minecraft_core::{Id, ResourceLocation, TagId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    World,
    Dimension,
    Reloadable,
}

#[derive(Clone)]
pub(crate) struct Table {
    pub(crate) key: ResourceLocation,
    pub(crate) names: Arc<Names>,
    pub(crate) registry: Arc<dyn Any + Send + Sync>,
}

pub(crate) type Tables = HashMap<ResourceLocation, Table>;

pub(crate) type TagTables = HashMap<ResourceLocation, Arc<TagTable>>;

#[derive(Clone)]
pub struct Registries {
    world: Arc<Tables>,
    reloadable: Arc<Tables>,
    tags: Arc<TagTables>,
    keys: Arc<HashMap<TypeId, ResourceLocation>>,
    world_stack: Arc<PackStack>,
    reload_stack: Arc<PackStack>,
    generation: u64,
    network: Arc<NetworkSnapshot>,
}

impl Registries {
    /// What the configuration phase sends of these registries.
    pub fn network(&self) -> &NetworkSnapshot {
        &self.network
    }

    pub fn registry<R: RegistryType>(&self) -> Option<&Registry<R>> {
        self.typed::<R>()?.registry.downcast_ref::<Registry<R>>()
    }

    /// The key of the registry `R` names, whether `R` holds its values or is a marker for it.
    pub fn key_of<R: RegistryType>(&self) -> Option<&ResourceLocation> {
        self.keys.get(&TypeId::of::<R>())
    }

    /// The entry names of the registry `key`, in id order, for a caller that has no type for it.
    pub fn names_of(&self, key: &ResourceLocation) -> Option<&[ResourceLocation]> {
        self.table(key).map(|table| table.names.all())
    }

    /// Each resolved tag of the registry `key` with its member ids, for a caller that has no type
    /// for it.
    pub fn tags_of(
        &self,
        key: &ResourceLocation,
    ) -> Option<impl Iterator<Item = (&ResourceLocation, &[u16])>> {
        let tags = self.tag_table(key)?;
        Some(tags.resolved().map(|(_, name, members)| (name, members)))
    }

    pub fn tags<R: RegistryType>(&self) -> Option<Tags<R>> {
        let table = self.tag_table(self.key_of::<R>()?)?;
        Some(Tags::new(Arc::clone(table)))
    }

    pub fn origin<R: RegistryType>(&self, id: Id<R>) -> Option<&StackedPack> {
        let key = self.key_of::<R>()?;
        let (table, stack) = match self.world.get(key) {
            Some(table) => (table, &self.world_stack),
            None => (self.reloadable.get(key)?, &self.reload_stack),
        };
        let pack = table.registry.downcast_ref::<Registry<R>>()?.origin(id)?;
        stack.get(pack)
    }

    pub fn known_packs(&self) -> impl Iterator<Item = &KnownPack> {
        self.world_stack.known_packs()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn table(&self, key: &ResourceLocation) -> Option<&Table> {
        self.world.get(key).or_else(|| self.reloadable.get(key))
    }

    fn typed<R: RegistryType>(&self) -> Option<&Table> {
        self.table(self.key_of::<R>()?)
    }

    pub(crate) fn tag_table(&self, key: &ResourceLocation) -> Option<&Arc<TagTable>> {
        self.tags.get(key)
    }

    fn typed_tags<R: RegistryType>(&self) -> Option<&Arc<TagTable>> {
        self.tag_table(self.key_of::<R>()?)
    }
}

impl Namer for Registries {
    fn name<R: RegistryType>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.typed::<R>()?.names.name(id.number())
    }

    fn tag_name<R: RegistryType>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.typed_tags::<R>()?.name(tag.index())
    }
}

impl Resolver for Registries {
    fn id<R: RegistryType>(&self, key: &Key<R>) -> Result<Id<R>, String> {
        let table = self.typed::<R>().ok_or_else(|| unbound::<R>())?;
        table
            .names
            .get(key.location())
            .and_then(|number| Id::checked(number, table.names.len()))
            .ok_or_else(|| format!("unknown entry {} of {}", key.location(), table.key))
    }

    fn tag<R: RegistryType>(&self, tag: &Key<R>) -> Result<TagId<R>, String> {
        let tags = self.typed_tags::<R>().ok_or_else(|| unbound::<R>())?;
        let registry = self.key_of::<R>().ok_or_else(|| unbound::<R>())?;
        tags.get(tag.location())
            .and_then(|number| TagId::checked(number, tags.len()))
            .ok_or_else(|| format!("unknown tag #{} of {registry}", tag.location()))
    }
}

fn unbound<R>() -> String {
    format!("no registry is bound to {}", std::any::type_name::<R>())
}

impl fmt::Debug for Registries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registries")
            .field("world", &self.world.len())
            .field("reloadable", &self.reloadable.len())
            .field("generation", &self.generation)
            .finish()
    }
}
