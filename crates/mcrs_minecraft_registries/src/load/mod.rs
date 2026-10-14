mod ctx;
mod declaration;
mod index;
mod loader;
mod tag_build;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

pub use ctx::{IdSeed, IdSetSeed, LoadCtx, RefSeed, ValueSeed};
pub use declaration::{DataDeclaration, Declaration};
pub use loader::load;

use crate::id::Id;
use crate::pack::{KnownPack, PackStack, StackedPack};
use crate::registry::{Names, Registry, RegistryType};
use crate::tags::{TagTable, Tags};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    World,
    Dimension,
    Reloadable,
}

impl Stage {
    pub(crate) fn rank(self) -> u8 {
        match self {
            Stage::World => 1,
            Stage::Dimension => 2,
            Stage::Reloadable => 3,
        }
    }
}

pub(crate) const STATIC_RANK: u8 = 0;
pub(crate) const RELOADABLE_RANK: u8 = 3;

#[derive(Clone)]
pub(crate) struct Table {
    pub(crate) names: Arc<Names>,
    pub(crate) registry: Arc<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub(crate) struct Tables {
    by_type: HashMap<TypeId, Table>,
}

pub(crate) struct TagEntry {
    pub(crate) table: Arc<TagTable>,
    typed: Box<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub(crate) struct TagSets {
    by_type: HashMap<TypeId, TagEntry>,
}

#[derive(Clone)]
pub struct Registries {
    world: Arc<Tables>,
    reloadable: Arc<Tables>,
    tags: Arc<TagSets>,
    world_stack: Arc<PackStack>,
    reload_stack: Arc<PackStack>,
    generation: u64,
}

impl Registries {
    pub fn registry<R: RegistryType>(&self) -> Option<&Registry<R>> {
        self.table(TypeId::of::<R>())?
            .registry
            .downcast_ref::<Registry<R>>()
    }

    pub fn tags<R: RegistryType>(&self) -> Option<&Tags<R>> {
        self.tags
            .by_type
            .get(&TypeId::of::<R>())?
            .typed
            .downcast_ref::<Tags<R>>()
    }

    pub fn origin<R: RegistryType>(&self, id: Id<R>) -> Option<&StackedPack> {
        let type_id = TypeId::of::<R>();
        let (table, stack) = match self.world.by_type.get(&type_id) {
            Some(table) => (table, &self.world_stack),
            None => (self.reloadable.by_type.get(&type_id)?, &self.reload_stack),
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

    pub(crate) fn table(&self, type_id: TypeId) -> Option<&Table> {
        self.world
            .by_type
            .get(&type_id)
            .or_else(|| self.reloadable.by_type.get(&type_id))
    }

    pub(crate) fn tag_table(&self, type_id: TypeId) -> Option<&Arc<TagTable>> {
        self.tags.by_type.get(&type_id).map(|entry| &entry.table)
    }
}

impl fmt::Debug for Registries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registries")
            .field("world", &self.world.by_type.len())
            .field("reloadable", &self.reloadable.by_type.len())
            .field("generation", &self.generation)
            .finish()
    }
}
