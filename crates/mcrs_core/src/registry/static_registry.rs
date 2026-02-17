use crate::resource_location::ResourceLocation;
use bevy_ecs::resource::Resource;
use std::collections::HashMap;
use std::marker::PhantomData;

/// A typed index into a `StaticRegistry<T>`.
///
/// `PhantomData<fn() -> T>` makes the ID covariant, `Send + Sync`, and non-`Drop`.
/// All trait impls are manual so that `T` does not need to satisfy any bounds.
pub struct StaticId<T> {
    pub(crate) id: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for StaticId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for StaticId<T> {}
impl<T> PartialEq for StaticId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T> Eq for StaticId<T> {}
impl<T> std::hash::Hash for StaticId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl<T> std::fmt::Debug for StaticId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StaticId({})", self.id)
    }
}

impl<T> StaticId<T> {
    pub fn raw(self) -> u32 {
        self.id
    }
}

/// A compile-time registry mapping `ResourceLocation` → `&'static T`.
///
/// All entries must live for `'static` (they are typically `static` variables or
/// struct fields in `lazy_static!`/`once_cell` statics).
///
/// IDs are assigned sequentially in insertion order and are stable for the lifetime
/// of the process.
#[derive(Resource)]
pub struct StaticRegistry<T: 'static> {
    entries: Vec<(ResourceLocation, &'static T)>,
    index: HashMap<ResourceLocation, u32>,
}

impl<T: 'static> StaticRegistry<T> {
    pub fn new() -> Self {
        StaticRegistry {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Register a new entry; returns its `StaticId`.
    ///
    /// Panics if `loc` is already registered.
    pub fn register(&mut self, loc: ResourceLocation, value: &'static T) -> StaticId<T> {
        let id = self.entries.len() as u32;
        assert!(
            self.index.insert(loc.clone(), id).is_none(),
            "duplicate registration: {loc}"
        );
        self.entries.push((loc, value));
        StaticId {
            id,
            _marker: PhantomData,
        }
    }

    pub fn get_by_id(&self, id: StaticId<T>) -> Option<&'static T> {
        self.entries.get(id.id as usize).map(|(_, v)| *v)
    }

    pub fn get_by_loc(&self, loc: &ResourceLocation) -> Option<&'static T> {
        let id = *self.index.get(loc)?;
        self.entries.get(id as usize).map(|(_, v)| *v)
    }

    pub fn id_of(&self, loc: &ResourceLocation) -> Option<StaticId<T>> {
        self.index.get(loc).copied().map(|id| StaticId {
            id,
            _marker: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (StaticId<T>, &ResourceLocation, &'static T)> + '_ {
        self.entries.iter().enumerate().map(|(i, (loc, v))| {
            (
                StaticId {
                    id: i as u32,
                    _marker: PhantomData,
                },
                loc,
                *v,
            )
        })
    }
}

impl<T: 'static> Default for StaticRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}
