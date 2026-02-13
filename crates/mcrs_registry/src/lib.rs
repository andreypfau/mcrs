use bevy_ecs::prelude::Resource;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use std::marker::PhantomData;
use valence_ident::Ident;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RegistryId<E> {
    Index {
        index: usize,
        marker: PhantomData<E>,
    },
    Identifier {
        identifier: Ident<String>,
    },
    StaticIdentifier {
        identifier: Ident<&'static str>,
    },
}

impl<E> From<Ident<String>> for RegistryId<E> {
    fn from(value: Ident<String>) -> Self {
        RegistryId::Identifier { identifier: value }
    }
}

impl<E> From<Ident<&'static str>> for RegistryId<E> {
    fn from(value: Ident<&'static str>) -> Self {
        RegistryId::StaticIdentifier { identifier: value }
    }
}

#[derive(Resource)]
pub struct Registry<E> {
    items: IndexMap<Ident<String>, E>,
}

impl<E> Default for Registry<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Registry<E> {
    pub fn new() -> Self {
        Self {
            items: IndexMap::new(),
        }
    }

    pub fn get(&self, id: impl Into<RegistryId<E>>) -> Option<&E> {
        match id.into() {
            RegistryId::Index { index, .. } => self.items.get_index(index).map(|(_, v)| v),
            RegistryId::Identifier { identifier } => self.items.get(&identifier),
            RegistryId::StaticIdentifier { identifier } => self.items.get(identifier.as_str()),
        }
    }

    pub fn get_full(&self, id: impl Into<RegistryId<E>>) -> Option<(usize, &E)> {
        match id.into() {
            RegistryId::Index { index, .. } => self.items.get_index(index).map(|(_, v)| (index, v)),
            RegistryId::Identifier { identifier } => self
                .items
                .get_full(&identifier)
                .map(|(index, _, v)| (index, v)),
            RegistryId::StaticIdentifier { identifier } => self
                .items
                .get_full(identifier.as_str())
                .map(|(index, _, v)| (index, v)),
        }
    }

    pub fn insert(&mut self, id: impl Into<Ident<String>>, entry: E) -> RegistryRef<E> {
        let index = self.items.len();
        let id = id.into();
        self.items.insert(id.clone(), entry);

        RegistryRef {
            index,
            identifier: id,
            marker: PhantomData,
        }
    }

    pub fn shift_insert(
        &mut self,
        index: usize,
        id: impl Into<Ident<String>>,
        entry: E,
    ) -> RegistryRef<E> {
        let id = id.into();
        self.items.shift_insert(index, id.clone(), entry);

        RegistryRef {
            index,
            identifier: id,
            marker: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = RegistryId<E>> + '_ {
        self.items
            .iter()
            .enumerate()
            .map(|(i, (_k, _))| RegistryId::Index {
                index: i,
                marker: PhantomData,
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RegistryRef<E> {
    index: usize,
    identifier: Ident<String>,
    marker: PhantomData<E>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Holder<E: Debug + Clone + PartialEq> {
    Reference(Ident<Cow<'static, str>>),
    Direct(E),
}

impl<E> From<RegistryRef<E>> for RegistryId<E> {
    fn from(value: RegistryRef<E>) -> Self {
        RegistryId::Index {
            index: value.index,
            marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence_ident::ident;

    fn test_registry() {
        struct TestEntry {}

        const A: Ident<&'static str> = ident!("test_entry");

        let mut reg = Registry::<TestEntry>::new();
        let r = reg.insert(A, TestEntry {});
        let option = reg.get(r);
        let f = reg.get(ident!("not_found"));
    }
}
