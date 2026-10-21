use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use mcrs_minecraft_core::{Id, ResourceLocation};
use serde::Deserializer;
use serde::de::{DeserializeOwned, Error as _};

use crate::id::MAX_ENTRIES;
use crate::json;
use crate::load::LoadCtx;
use crate::pack::PackIndex;
use crate::refs::Resolve;

pub trait RegistryType: 'static {
    type Value: Send + Sync + 'static;
}

pub struct RegistryOf<V>(PhantomData<fn() -> V>);

impl<V: Send + Sync + 'static> RegistryType for RegistryOf<V> {
    type Value = V;
}

pub trait ReadValue: Sized {
    fn read<'de, D: Deserializer<'de>>(
        ctx: &LoadCtx<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error>;

    /// Reads a whole file, returning the failing field apart from the message.
    #[doc(hidden)]
    fn read_file(ctx: &LoadCtx<'_>, bytes: &[u8]) -> Result<Self, (Option<String>, String)> {
        json::read_seed(bytes, ctx.value::<Self>()).map_err(|e| (e.field, e.message))
    }
}

impl<T: Resolve> ReadValue for T
where
    T::Named: DeserializeOwned,
{
    fn read<'de, D: Deserializer<'de>>(
        ctx: &LoadCtx<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let named = <T::Named as serde::Deserialize>::deserialize(deserializer)?;
        T::resolve(named, ctx).map_err(D::Error::custom)
    }

    fn read_file(ctx: &LoadCtx<'_>, bytes: &[u8]) -> Result<Self, (Option<String>, String)> {
        let named = json::read::<T::Named>(bytes).map_err(|e| (e.field, e.message))?;
        T::resolve(named, ctx).map_err(|e| {
            let field = (!e.path().is_empty()).then(|| e.path().to_owned());
            (field, e.message().to_owned())
        })
    }
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
    values: Arc<[R::Value]>,
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
            values: values.into(),
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

    pub fn values(&self) -> &Arc<[R::Value]> {
        &self.values
    }

    pub fn get(&self, id: Id<R>) -> Option<&R::Value> {
        self.values.get(id.index())
    }

    pub fn name(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.names.name(id.number())
    }

    pub fn by_name(&self, name: &ResourceLocation) -> Option<Id<R>> {
        self.names
            .get(name)
            .and_then(|number| Id::checked(number, self.names.len()))
    }

    pub(crate) fn origin(&self, id: Id<R>) -> Option<PackIndex> {
        self.origins.get(id.index()).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<R>, &ResourceLocation, &R::Value)> {
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

    pub fn entry_failures(&self, failures: Vec<(usize, String)>) -> Result<(), String> {
        if failures.is_empty() {
            return Ok(());
        }
        let names = self.names.all();
        let lines: Vec<String> = failures
            .into_iter()
            .map(|(index, message)| match names.get(index) {
                Some(name) => format!("{name}: {message}"),
                None => message,
            })
            .collect();
        Err(lines.join("; "))
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
