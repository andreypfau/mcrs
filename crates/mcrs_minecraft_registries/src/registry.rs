use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use derive_where::derive_where;
use mcrs_minecraft_core::{Id, Registered, RegistryValue, ResourceLocation};
use mcrs_minecraft_datapack::PackIndex;
use serde::de::DeserializeOwned;

use crate::refs::Resolve;

// chisle: an id is a u16, so a registry holds at most 65 536 entries; widening `Id` lifts it.
pub(crate) const MAX_ENTRIES: usize = u16::MAX as usize + 1;

/// What a data registry holds: a value read from the named form its files and packets carry.
pub trait DataValue:
    RegistryValue + Resolve<Named: DeserializeOwned + 'static> + Send + Sync + 'static
{
}

impl<T> DataValue for T where
    T: RegistryValue + Resolve<Named: DeserializeOwned + 'static> + Send + Sync + 'static
{
}

#[derive(Debug)]
pub(crate) struct Names {
    names: Box<[ResourceLocation]>,
    index: HashMap<ResourceLocation, u16>,
    origins: Box<[PackIndex]>,
}

impl Names {
    /// `origins` holds the pack of each entry, or nothing for a registry no pack fills.
    pub(crate) fn new(
        names: Vec<ResourceLocation>,
        origins: Vec<PackIndex>,
    ) -> Result<Self, usize> {
        if names.len() > MAX_ENTRIES {
            return Err(names.len());
        }
        let index = names.iter().cloned().zip(0..=u16::MAX).collect();
        Ok(Self {
            names: names.into_boxed_slice(),
            index,
            origins: origins.into_boxed_slice(),
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn get<Q>(&self, name: &Q) -> Option<u16>
    where
        ResourceLocation: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(name).copied()
    }

    pub(crate) fn id<R, Q>(&self, name: &Q) -> Option<Id<R>>
    where
        ResourceLocation: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        Id::checked(self.get(name)?, self.len())
    }

    pub(crate) fn name(&self, number: u16) -> Option<&ResourceLocation> {
        self.names.get(usize::from(number))
    }

    pub(crate) fn all(&self) -> &[ResourceLocation] {
        &self.names
    }

    pub(crate) fn origin(&self, number: u16) -> Option<PackIndex> {
        self.origins.get(usize::from(number)).copied()
    }
}

/// The entries of one registry in id order, whatever values it holds: a static registry, a data
/// registry or one read by its names alone.
pub struct Entries<'a, R> {
    names: &'a Names,
    registry: PhantomData<fn() -> R>,
}

impl<'a, R> Entries<'a, R> {
    pub(crate) fn new(names: &'a Names) -> Self {
        Self {
            names,
            registry: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.len() == 0
    }

    pub fn by_name(&self, name: &ResourceLocation) -> Option<Id<R>> {
        self.names.id(name)
    }

    pub fn name(&self, id: Id<R>) -> Option<&'a ResourceLocation> {
        self.names.name(id.number())
    }

    pub fn names(&self) -> &'a [ResourceLocation] {
        self.names.all()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<R>, &'a ResourceLocation)> + 'a {
        let len = self.names.len();
        (self.names.all().iter().zip(0..=u16::MAX))
            .filter_map(move |(name, number)| Some((Id::checked(number, len)?, name)))
    }
}

#[derive_where(Clone)]
pub struct Registry<V: RegistryValue> {
    names: Arc<Names>,
    values: Arc<[V]>,
}

impl<V: RegistryValue> Registry<V> {
    pub(crate) fn new(names: Arc<Names>, values: Vec<V>) -> Self {
        Self {
            names,
            values: values.into(),
        }
    }

    pub fn key(&self) -> ResourceLocation<&'static str> {
        <V::Registry as Registered>::REGISTRY.location()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &Arc<[V]> {
        &self.values
    }

    pub fn get(&self, id: Id<V::Registry>) -> Option<&V> {
        self.values.get(id.index())
    }

    pub fn name(&self, id: Id<V::Registry>) -> Option<&ResourceLocation> {
        self.names.name(id.number())
    }

    pub fn by_name(&self, name: &ResourceLocation) -> Option<Id<V::Registry>> {
        self.names.id(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<V::Registry>, &ResourceLocation, &V)> {
        let len = self.names.len();
        self.names
            .all()
            .iter()
            .zip(self.values.iter())
            .zip(0..=u16::MAX)
            .filter_map(move |((name, value), number)| {
                Some((Id::checked(number, len)?, name, value))
            })
    }
}

impl<V: RegistryValue> fmt::Debug for Registry<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("key", &self.key())
            .field("len", &self.len())
            .finish()
    }
}
