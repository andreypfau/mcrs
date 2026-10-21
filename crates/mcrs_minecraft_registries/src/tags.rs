use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::{Id, ResourceLocation, TagId, TagKey};

use crate::id::MAX_ENTRIES;
use crate::registry::RegistryType;

#[derive(Debug)]
struct Tag {
    name: ResourceLocation,
    members: Option<Box<[u16]>>,
    row: FixedBitSet,
}

#[derive(Debug)]
pub(crate) struct TagTable {
    entries: usize,
    tags: Box<[Tag]>,
    index: HashMap<ResourceLocation, u16>,
}

impl TagTable {
    pub(crate) fn new(
        entries: usize,
        tags: Vec<(ResourceLocation, Option<Vec<u16>>)>,
    ) -> Result<Self, usize> {
        if tags.len() > MAX_ENTRIES {
            return Err(tags.len());
        }
        let mut index = HashMap::new();
        let tags = tags
            .into_iter()
            .zip(0..=u16::MAX)
            .map(|((name, resolved), number)| {
                if resolved.is_some() {
                    index.insert(name.clone(), number);
                }
                let row = resolved.iter().flatten().map(|&id| usize::from(id));
                Tag {
                    row: row.collect(),
                    members: resolved.map(Vec::into_boxed_slice),
                    name,
                }
            })
            .collect();
        Ok(Self {
            entries,
            tags,
            index,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.tags.len()
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &ResourceLocation> {
        self.tags.iter().map(|tag| &tag.name)
    }

    pub(crate) fn resolved(&self) -> impl Iterator<Item = (u16, &ResourceLocation, &[u16])> {
        self.tags
            .iter()
            .zip(0..=u16::MAX)
            .filter_map(|(tag, number)| Some((number, &tag.name, tag.members.as_deref()?)))
    }

    pub(crate) fn get<Q>(&self, name: &Q) -> Option<u16>
    where
        ResourceLocation: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(name).copied()
    }

    pub(crate) fn name(&self, number: usize) -> Option<&ResourceLocation> {
        self.tag(number).map(|tag| &tag.name)
    }

    fn tag(&self, number: usize) -> Option<&Tag> {
        self.tags.get(number)
    }

    fn members(&self, number: usize) -> &[u16] {
        self.tag(number)
            .and_then(|tag| tag.members.as_deref())
            .unwrap_or_default()
    }

    fn contains(&self, number: usize, id: usize) -> bool {
        self.tag(number).is_some_and(|tag| tag.row.contains(id))
    }
}

pub struct Tags<R> {
    table: Arc<TagTable>,
    registry: PhantomData<fn() -> R>,
}

impl<R: RegistryType> Tags<R> {
    pub(crate) fn new(table: Arc<TagTable>) -> Self {
        Self {
            table,
            registry: PhantomData,
        }
    }

    pub fn by_name(&self, name: &ResourceLocation) -> Option<TagId<R>> {
        self.table
            .get(name)
            .and_then(|number| TagId::checked(number, self.table.len()))
    }

    /// An absent tag is `None`, and code that names it treats it as empty.
    pub fn id<S: AsRef<str>>(&self, key: &TagKey<R, S>) -> Option<TagId<R>> {
        self.table
            .get(key.as_str())
            .and_then(|number| TagId::checked(number, self.table.len()))
    }

    pub fn name(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.table.tag(tag.index()).map(|tag| &tag.name)
    }

    pub fn contains(&self, tag: TagId<R>, id: Id<R>) -> bool {
        self.table.contains(tag.index(), id.index())
    }

    pub fn members(&self, tag: TagId<R>) -> impl Iterator<Item = Id<R>> + '_ {
        let entries = self.table.entries;
        self.table
            .members(tag.index())
            .iter()
            .filter_map(move |&id| Id::checked(id, entries))
    }

    pub fn iter(&self) -> impl Iterator<Item = (TagId<R>, &ResourceLocation)> {
        let len = self.table.len();
        self.table
            .resolved()
            .filter_map(move |(number, name, _)| Some((TagId::checked(number, len)?, name)))
    }
}

impl<R> fmt::Debug for Tags<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tags")
            .field("tags", &self.table.index.len())
            .finish()
    }
}

pub enum IdSet<R> {
    Tag(TagId<R>),
    List(Box<[Id<R>]>),
}

impl<R: RegistryType> IdSet<R> {
    pub fn contains(&self, tags: &Tags<R>, id: Id<R>) -> bool {
        match self {
            IdSet::Tag(tag) => tags.contains(*tag, id),
            IdSet::List(ids) => ids.contains(&id),
        }
    }

    pub fn ids<'a>(&'a self, tags: &'a Tags<R>) -> impl Iterator<Item = Id<R>> + 'a {
        let (tag, list) = match self {
            IdSet::Tag(tag) => (Some(tags.members(*tag)), None),
            IdSet::List(ids) => (None, Some(ids.iter().copied())),
        };
        tag.into_iter().flatten().chain(list.into_iter().flatten())
    }
}

impl<R> Clone for IdSet<R> {
    fn clone(&self) -> Self {
        match self {
            IdSet::Tag(tag) => IdSet::Tag(*tag),
            IdSet::List(ids) => IdSet::List(ids.clone()),
        }
    }
}

impl<R> PartialEq for IdSet<R> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (IdSet::Tag(a), IdSet::Tag(b)) => a == b,
            (IdSet::List(a), IdSet::List(b)) => a == b,
            _ => false,
        }
    }
}

impl<R> fmt::Debug for IdSet<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdSet::Tag(tag) => f.debug_tuple("Tag").field(tag).finish(),
            IdSet::List(ids) => f.debug_tuple("List").field(ids).finish(),
        }
    }
}
