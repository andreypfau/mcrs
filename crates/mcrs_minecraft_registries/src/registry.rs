use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use serde::Deserializer;

use crate::id::{Id, MAX_ENTRIES};
use crate::load::LoadCtx;
use crate::pack::PackIndex;

pub trait RegistryType: 'static {
    type Value: Send + Sync + 'static;
}

pub trait ReadValue: Sized {
    fn read<'de, D: Deserializer<'de>>(
        ctx: &LoadCtx<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error>;
}

#[derive(Debug)]
pub(crate) struct Names {
    names: Box<[ResourceLocation]>,
    index: HashMap<ResourceLocation, u16>,
}

impl Names {
    pub(crate) fn new(names: Vec<ResourceLocation>) -> Result<Self, usize> {
        if names.len() > MAX_ENTRIES {
            return Err(names.len());
        }
        let index = names.iter().cloned().zip(0..=u16::MAX).collect();
        Ok(Self {
            names: names.into_boxed_slice(),
            index,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn get(&self, name: &ResourceLocation) -> Option<u16> {
        self.index.get(name).copied()
    }

    pub(crate) fn name(&self, number: u16) -> Option<&ResourceLocation> {
        self.names.get(usize::from(number))
    }

    pub(crate) fn all(&self) -> &[ResourceLocation] {
        &self.names
    }
}

pub struct Registry<R: RegistryType> {
    key: ResourceLocation,
    names: Arc<Names>,
    values: Box<[R::Value]>,
    origins: Box<[PackIndex]>,
}

impl<R: RegistryType> Registry<R> {
    pub(crate) fn new(
        key: ResourceLocation,
        names: Arc<Names>,
        values: Vec<R::Value>,
        origins: Vec<PackIndex>,
    ) -> Self {
        Self {
            key,
            names,
            values: values.into_boxed_slice(),
            origins: origins.into_boxed_slice(),
        }
    }

    pub fn key(&self) -> &ResourceLocation {
        &self.key
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, id: Id<R>) -> Option<&R::Value> {
        self.values.get(id.index())
    }

    pub fn name(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.names.name(id.number())
    }

    pub fn by_name(&self, name: &ResourceLocation) -> Option<Id<R>> {
        self.names.get(name).map(Id::new)
    }

    pub(crate) fn origin(&self, id: Id<R>) -> Option<PackIndex> {
        self.origins.get(id.index()).copied()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (Id<R>, &ResourceLocation, &R::Value)> {
        self.names
            .all()
            .iter()
            .zip(self.values.iter())
            .zip(0..=u16::MAX)
            .map(|((name, value), number)| (Id::new(number), name, value))
    }
}

impl<R: RegistryType> fmt::Debug for Registry<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("key", &self.key)
            .field("len", &self.len())
            .finish()
    }
}
