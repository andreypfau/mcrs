use std::any::{TypeId, type_name};
use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::{Id, ResourceLocation, TagId};
use mcrs_minecraft_registries::{Key, Namer, RegistryOf, RegistryType, Resolve, Resolver};

use serde::{Deserialize, Deserializer, Serialize};

use crate::set::RegistrySet;

// chisle: a data pack registry of the new loader is typed `RegistryOf<V>` and is bound here under
// `V`, so a lookup tries both; it goes with this set once the client loads through the new loader.
impl RegistrySet {
    fn bound<R: RegistryType>(&self) -> Option<&ResourceLocation> {
        let data_registry = TypeId::of::<R>() == TypeId::of::<RegistryOf<R::Value>>();
        self.bound_name(TypeId::of::<R>()).or_else(|| {
            data_registry
                .then(|| self.bound_name(TypeId::of::<R::Value>()))
                .flatten()
        })
    }

    /// The name of the registry `R` names entries of.
    pub fn registry_name<R: RegistryType>(&self) -> Option<&ResourceLocation> {
        self.bound::<R>()
    }

    fn bound_or_err<R: RegistryType>(&self) -> Result<&ResourceLocation, String> {
        self.bound::<R>()
            .ok_or_else(|| format!("no registry is bound to {}", type_name::<R>()))
    }
}

impl Resolver for RegistrySet {
    fn id<R: RegistryType>(&self, key: &Key<R>) -> Result<Id<R>, String> {
        let registry = self.bound_or_err::<R>()?;
        let unknown = || format!("unknown entry {} of {registry}", key.location());
        let table = self.table(registry.as_str()).ok_or_else(unknown)?;
        table
            .number(key.location().as_str())
            .and_then(|number| Id::checked(number, table.len()))
            .ok_or_else(unknown)
    }

    // chisle: a linear scan of the tag names per reference; only loading reaches it, and it goes
    // with this set.
    fn tag<R: RegistryType>(&self, tag: &Key<R>) -> Result<TagId<R>, String> {
        let registry = self.bound_or_err::<R>()?;
        let unknown = || format!("unknown tag #{} of {registry}", tag.location());
        let tags = self.tag_table(registry.as_str()).ok_or_else(unknown)?;
        tags.names()
            .iter()
            .position(|name| name == tag.location())
            .and_then(|at| TagId::checked(u16::try_from(at).ok()?, tags.len()))
            .ok_or_else(unknown)
    }
}

impl Namer for RegistrySet {
    fn name<R: RegistryType>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.table(self.bound::<R>()?.as_str())?.name(id.index())
    }

    fn tag_name<R: RegistryType>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.tag_table(self.bound::<R>()?.as_str())?
            .names()
            .get(tag.index())
    }
}

/// Resolves a named value against the registry set in scope, if one is, so that a value read
/// through that scope fails on a name the set does not hold. Outside a scope it accepts any name.
// chisle: values that still load through this scope check their named references here; the check
// goes with the scope once those values load through the new loader.
pub fn check_in_scope<T: Resolve>(named: T::Named) -> Result<(), String> {
    match crate::set::current() {
        Some(set) => T::resolve(named, &set).map(drop).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Names a resolved value against the registry set in scope; `None` outside a scope or when the
/// set does not hold a referenced entry.
// chisle: readers whose named data is compared against resolved values name them here; this goes
// with the scope once those readers resolve their data through the new loader.
pub fn name_in_scope<T: Resolve>(value: &T) -> Option<T::Named> {
    value.name(&crate::set::current()?).ok()
}

/// The named form of `T`, checked against the registries in scope when read.
// chisle: values that load through the scope hold names until they load through the registry
// loader, which resolves them; the check goes with the scope.
// The named type is a parameter, not a projection, so that a recursive value holding one stays
// `Sync` without the solver normalising through `Resolve`.
#[derive(Serialize)]
#[serde(transparent)]
pub struct Checked<T, N = <T as Resolve>::Named>(pub N, #[serde(skip)] PhantomData<fn() -> T>);

impl<T, N> Checked<T, N> {
    pub fn unchecked(named: N) -> Self {
        Checked(named, PhantomData)
    }
}

impl<T, N: Default> Default for Checked<T, N> {
    fn default() -> Self {
        Checked(N::default(), PhantomData)
    }
}

impl<T, N: Clone> Clone for Checked<T, N> {
    fn clone(&self) -> Self {
        Checked(self.0.clone(), PhantomData)
    }
}

impl<T, N: PartialEq> PartialEq for Checked<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T, N: fmt::Debug> fmt::Debug for Checked<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de, T: Resolve<Named = N>, N: Deserialize<'de> + Clone> Deserialize<'de> for Checked<T, N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let named = N::deserialize(d)?;
        check_in_scope::<T>(named.clone()).map_err(serde::de::Error::custom)?;
        Ok(Checked(named, PhantomData))
    }
}
