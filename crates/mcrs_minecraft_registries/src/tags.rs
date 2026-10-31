use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use derive_where::derive_where;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_resource::{Id, ResourceLocation, ResourceLocationBuf, TagId, TagKey};

use crate::registry::MAX_ENTRIES;

#[derive(Debug)]
struct Tag {
    name: ResourceLocationBuf,
    members: Option<Box<[u16]>>,
    row: FixedBitSet,
}

#[derive(Debug)]
pub(crate) struct TagTable {
    entries: usize,
    tags: Box<[Tag]>,
    index: hashbrown::HashMap<ResourceLocationBuf, u16>,
}

impl TagTable {
    pub(crate) fn new(
        entries: usize,
        tags: Vec<(ResourceLocationBuf, Option<Vec<u16>>)>,
    ) -> Result<Self, usize> {
        if tags.len() > MAX_ENTRIES {
            return Err(tags.len());
        }
        let mut index = hashbrown::HashMap::new();
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

    pub(crate) fn names(&self) -> impl Iterator<Item = &ResourceLocationBuf> {
        self.tags.iter().map(|tag| &tag.name)
    }

    pub(crate) fn resolved(&self) -> impl Iterator<Item = (u16, &ResourceLocationBuf, &[u16])> {
        self.tags
            .iter()
            .zip(0..=u16::MAX)
            .filter_map(|(tag, number)| Some((number, &tag.name, tag.members.as_deref()?)))
    }

    pub(crate) fn id<R>(&self, name: ResourceLocation<'_>) -> Option<TagId<R>> {
        TagId::checked(*self.index.get(&name)?, self.len())
    }

    pub(crate) fn name(&self, number: usize) -> Option<&ResourceLocationBuf> {
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

#[derive_where(Clone)]
pub struct Tags<R> {
    table: Arc<TagTable>,
    registry: PhantomData<fn() -> R>,
}

impl<R> Tags<R> {
    pub(crate) fn new(table: Arc<TagTable>) -> Self {
        Self {
            table,
            registry: PhantomData,
        }
    }

    /// An absent tag is `None`, and code that names it treats it as empty.
    pub fn id<'k>(&self, key: impl Into<TagKey<'k, R>>) -> Option<TagId<R>> {
        self.table.id(key.into().location())
    }

    pub fn name(&self, tag: TagId<R>) -> Option<&ResourceLocationBuf> {
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

    pub fn iter(&self) -> impl Iterator<Item = (TagId<R>, &ResourceLocationBuf)> {
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

#[derive_where(Clone, Default, PartialEq, Debug)]
pub enum IdSet<R> {
    Tag(TagId<R>),
    #[derive_where(default)]
    List(Box<[Id<R>]>),
}

impl<R> IdSet<R> {
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
