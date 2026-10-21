mod ctx;
mod declaration;
mod index;
mod loader;
mod tag_build;
mod view;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

pub use ctx::{LoadCtx, ValueSeed};
pub use declaration::{DataDeclaration, Declaration};
pub use loader::{StageScope, load, load_with};
pub use view::{RegistryView, StageView};

use crate::pack::{KnownPack, PackStack, StackedPack};
use crate::refs::Namer;
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
    pub(crate) names: Arc<Names>,
    pub(crate) registry: Arc<dyn Any + Send + Sync>,
}

pub(crate) type Tables = HashMap<TypeId, Table>;

pub(crate) type TagTables = HashMap<TypeId, Arc<TagTable>>;

#[derive(Clone)]
pub struct Registries {
    world: Arc<Tables>,
    reloadable: Arc<Tables>,
    tags: Arc<TagTables>,
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

    pub fn tags<R: RegistryType>(&self) -> Option<Tags<R>> {
        let table = self.tag_table(TypeId::of::<R>())?;
        Some(Tags::new(Arc::clone(table)))
    }

    pub fn origin<R: RegistryType>(&self, id: Id<R>) -> Option<&StackedPack> {
        let type_id = TypeId::of::<R>();
        let (table, stack) = match self.world.get(&type_id) {
            Some(table) => (table, &self.world_stack),
            None => (self.reloadable.get(&type_id)?, &self.reload_stack),
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
            .get(&type_id)
            .or_else(|| self.reloadable.get(&type_id))
    }

    pub(crate) fn tag_table(&self, type_id: TypeId) -> Option<&Arc<TagTable>> {
        self.tags.get(&type_id)
    }
}

impl Namer for Registries {
    fn name<R: RegistryType>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.table(TypeId::of::<R>())?.names.name(id.number())
    }

    fn tag_name<R: RegistryType>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.tag_table(TypeId::of::<R>())?.name(tag.index())
    }
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
