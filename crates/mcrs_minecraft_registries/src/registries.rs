use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use mcrs_minecraft_datapack::{KnownPack, PackStack, StackedPack};
use mcrs_minecraft_resource::{
    Id, Registered, RegistryValue, ResourceKey, ResourceLocation, TagId, TagKey,
};

use crate::declaration::Codec;
use crate::refs::{MissingRef, Namer, ResolveError, Resolver};
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
pub(crate) enum Kind {
    Fixed,
    Names,
    Data(Arc<dyn Codec>),
}

#[derive(Clone)]
pub(crate) struct Table {
    pub(crate) key: ResourceLocation,
    pub(crate) stage: Stage,
    pub(crate) kind: Kind,
    pub(crate) names: Arc<Names>,
    pub(crate) tags: Arc<TagTable>,
    /// `None` for a static registry, one read by its names alone, and a data registry whose stage
    /// is still decoding.
    pub(crate) registry: Option<Erased>,
}

/// The tables of one load, in the order of their declarations. While a load runs, the tables of
/// the stages it has not reached are absent.
#[derive(Clone)]
pub struct Registries {
    tables: Arc<Vec<Table>>,
    index: Arc<HashMap<ResourceLocation, usize>>,
    pub(crate) world_stack: Arc<PackStack>,
    pub(crate) reload_stack: Arc<PackStack>,
}

impl Registries {
    pub(crate) fn new(
        tables: Vec<Table>,
        world_stack: Arc<PackStack>,
        reload_stack: Arc<PackStack>,
    ) -> Self {
        let index = tables
            .iter()
            .enumerate()
            .map(|(at, table)| (table.key.clone(), at))
            .collect();
        Self {
            tables: Arc::new(tables),
            index: Arc::new(index),
            world_stack,
            reload_stack,
        }
    }

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
    pub fn names_of(&self, key: &ResourceLocation<impl AsRef<str>>) -> Option<&[ResourceLocation]> {
        self.table(key.as_str()).map(|table| table.names.all())
    }

    /// The id of the entry `name` in the registry `key`, for a caller that has no type for it.
    pub fn id_of(&self, key: &ResourceLocation<impl AsRef<str>>, name: &str) -> Option<u16> {
        self.table(key.as_str())?.names.get(name)
    }

    /// Each resolved tag of the registry `key` with its member ids, for a caller that has no type
    /// for it.
    pub fn tags_of<'s, S: AsRef<str>>(
        &'s self,
        key: &ResourceLocation<S>,
    ) -> Option<impl Iterator<Item = (&'s ResourceLocation, &'s [u16])> + use<'s, S>> {
        let table = self.table(key.as_str())?;
        Some(
            table
                .tags
                .resolved()
                .map(|(_, name, members)| (name, members)),
        )
    }

    pub fn tags<R: Registered>(&self) -> Option<Tags<R>> {
        let table = self.table(key_of::<R>())?;
        Some(Tags::new(Arc::clone(&table.tags)))
    }

    pub fn origin<R: Registered>(&self, id: Id<R>) -> Option<&StackedPack> {
        let table = self.table(key_of::<R>())?;
        self.stack(table.stage)
            .get(table.names.origin(id.number())?)
    }

    pub fn known_packs(&self) -> impl Iterator<Item = &KnownPack> {
        self.world_stack.known_packs()
    }

    /// Whether `a` and `b` are the same load, rather than two loads of equal data.
    pub fn ptr_eq(a: &Registries, b: &Registries) -> bool {
        Arc::ptr_eq(&a.tables, &b.tables)
    }

    pub(crate) fn tables(&self) -> &[Table] {
        &self.tables
    }

    pub(crate) fn table(&self, key: &str) -> Option<&Table> {
        self.tables.get(*self.index.get(key)?)
    }

    pub(crate) fn table_mut(&mut self, key: &str) -> Option<&mut Table> {
        let at = *self.index.get(key)?;
        Arc::make_mut(&mut self.tables).get_mut(at)
    }

    pub(crate) fn insert(&mut self, table: Table) {
        Arc::make_mut(&mut self.index).insert(table.key.clone(), self.tables.len());
        Arc::make_mut(&mut self.tables).push(table);
    }

    pub(crate) fn stack(&self, stage: Stage) -> &PackStack {
        match stage {
            Stage::Reloadable => &self.reload_stack,
            _ => &self.world_stack,
        }
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
        let name = key.location();
        table.names.id(name).ok_or_else(|| {
            let message = format!("unknown entry {name} of {}", table.key);
            miss(table, name, false, message)
        })
    }

    fn tag<R: Registered>(&self, tag: &TagKey<R>) -> Result<TagId<R>, ResolveError> {
        let table = self.table(key_of::<R>()).ok_or_else(unbound::<R>)?;
        let name = tag.location();
        table.tags.id(tag.as_str()).ok_or_else(|| {
            let message = format!("unknown tag #{name} of {}", table.key);
            miss(table, name, true, message)
        })
    }
}

fn miss(table: &Table, name: &ResourceLocation, tag: bool, message: String) -> ResolveError {
    let missing = MissingRef {
        registry: table.key.clone(),
        name: name.clone(),
        tag,
    };
    ResolveError::missing(missing, message)
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
