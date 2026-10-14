use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::ResourceLocation;

use crate::id::{Id, TagId};
use crate::registry::RegistryType;

#[derive(Debug)]
struct Tag {
    name: ResourceLocation,
    members: Option<Box<[u16]>>,
    row: FixedBitSet,
}

#[derive(Debug)]
pub(crate) struct TagTable {
    tags: Box<[Tag]>,
    index: HashMap<ResourceLocation, u32>,
}

impl TagTable {
    pub(crate) fn new(tags: Vec<(ResourceLocation, Option<Vec<u16>>)>) -> Self {
        let mut index = HashMap::new();
        let tags = tags
            .into_iter()
            .zip(0..=u32::MAX)
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
        Self { tags, index }
    }

    pub(crate) fn names(&self) -> impl Iterator<Item = &ResourceLocation> {
        self.tags.iter().map(|tag| &tag.name)
    }

    pub(crate) fn resolved(&self) -> impl Iterator<Item = (u32, &ResourceLocation, &[u16])> {
        self.tags
            .iter()
            .zip(0..=u32::MAX)
            .filter_map(|(tag, number)| Some((number, &tag.name, tag.members.as_deref()?)))
    }

    pub(crate) fn get(&self, name: &ResourceLocation) -> Option<u32> {
        self.index.get(name).copied()
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
        self.table.get(name).map(TagId::new)
    }

    pub fn name(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.table.tag(tag.index()).map(|tag| &tag.name)
    }

    pub fn contains(&self, tag: TagId<R>, id: Id<R>) -> bool {
        self.table.contains(tag.index(), id.index())
    }

    pub fn members(&self, tag: TagId<R>) -> impl ExactSizeIterator<Item = Id<R>> + '_ {
        self.table
            .members(tag.index())
            .iter()
            .map(|&id| Id::new(id))
    }

    pub fn iter(&self) -> impl Iterator<Item = (TagId<R>, &ResourceLocation)> {
        self.table
            .resolved()
            .map(|(number, name, _)| (TagId::new(number), name))
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

pub enum Ref<R: RegistryType> {
    Id(Id<R>),
    Inline(Arc<R::Value>),
}

impl<R: RegistryType> Clone for Ref<R> {
    fn clone(&self) -> Self {
        match self {
            Ref::Id(id) => Ref::Id(*id),
            Ref::Inline(value) => Ref::Inline(Arc::clone(value)),
        }
    }
}

impl<R: RegistryType> fmt::Debug for Ref<R>
where
    R::Value: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ref::Id(id) => f.debug_tuple("Id").field(id).finish(),
            Ref::Inline(value) => f.debug_tuple("Inline").field(value).finish(),
        }
    }
}
