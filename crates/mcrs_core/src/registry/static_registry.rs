use crate::resource_location::ResourceLocation;
use bevy_ecs::resource::Resource;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// A typed index into a `StaticRegistry<T>`.
///
/// `PhantomData<fn() -> T>` makes the ID covariant, `Send + Sync`, and non-`Drop`.
/// All trait impls are manual so that `T` does not need satisfy any bounds.
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
    pub(crate) fn new(id: u32) -> Self {
        StaticId {
            id,
            _marker: PhantomData,
        }
    }

    pub fn raw(self) -> u32 {
        self.id
    }
}

/// A compile-time registry mapping `ResourceLocation` → `&'static T`.
///
/// Internal storage uses `ResourceLocation<Arc<str>>` keys. Lookups accept
/// `&str` via `Borrow<str>` for zero-allocation access.
#[derive(Resource)]
pub struct StaticRegistry<T: 'static> {
    entries: Vec<(ResourceLocation<Arc<str>>, &'static T)>,
    index: HashMap<ResourceLocation<Arc<str>>, u32>,
    reverse: HashMap<usize, u32>,
    frozen: bool,
}

impl<T: 'static> StaticRegistry<T> {
    pub fn new() -> Self {
        StaticRegistry {
            entries: Vec::new(),
            index: HashMap::new(),
            reverse: HashMap::new(),
            frozen: false,
        }
    }

    /// Register a new entry; returns its `StaticId`.
    ///
    /// Accepts any `ResourceLocation` variant via `Into<ResourceLocation<Arc<str>>>`.
    /// Panics if `loc` is already registered.
    pub fn register(
        &mut self,
        loc: impl Into<ResourceLocation<Arc<str>>>,
        value: &'static T,
    ) -> StaticId<T> {
        assert!(!self.frozen, "register() called after freeze()");
        let loc = loc.into();
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

    /// Look up by string key. Zero-alloc via `Borrow<str>`.
    pub fn get_by_loc(&self, loc: &str) -> Option<&'static T> {
        let id = *self.index.get(loc)?;
        self.entries.get(id as usize).map(|(_, v)| *v)
    }

    /// Get the `StaticId` for a resource location string. Zero-alloc via `Borrow<str>`.
    pub fn id_of(&self, loc: &str) -> Option<StaticId<T>> {
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

    pub fn freeze(&mut self) {
        assert!(!self.frozen, "freeze() called twice");
        for (i, (_, value)) in self.entries.iter().enumerate() {
            self.reverse
                .insert(*value as *const T as usize, i as u32);
        }
        self.frozen = true;
        tracing::info!(count = self.entries.len(), "frozen StaticRegistry");
    }

    pub fn id_of_value(&self, value: &'static T) -> Option<StaticId<T>> {
        self.reverse
            .get(&(value as *const T as usize))
            .map(|&id| StaticId::new(id))
    }

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (StaticId<T>, &ResourceLocation<Arc<str>>, &'static T)> + '_ {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy(u32);

    static DUMMY_A: Dummy = Dummy(1);
    static DUMMY_B: Dummy = Dummy(2);
    static DUMMY_C: Dummy = Dummy(3);
    static DUMMY_UNKNOWN: Dummy = Dummy(999);

    fn loc(s: &'static str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::from_str_const(s)
    }

    fn make_registry() -> StaticRegistry<Dummy> {
        let mut reg = StaticRegistry::new();
        reg.register(loc("minecraft:a"), &DUMMY_A);
        reg.register(loc("minecraft:b"), &DUMMY_B);
        reg.register(loc("minecraft:c"), &DUMMY_C);
        reg
    }

    #[test]
    fn test_freeze_builds_reverse_index() {
        let mut reg = make_registry();
        reg.freeze();
        assert_eq!(reg.id_of_value(&DUMMY_A).unwrap().raw(), 0);
        assert_eq!(reg.id_of_value(&DUMMY_B).unwrap().raw(), 1);
        assert_eq!(reg.id_of_value(&DUMMY_C).unwrap().raw(), 2);
    }

    #[test]
    #[should_panic]
    fn test_register_after_freeze_panics() {
        let mut reg = make_registry();
        reg.freeze();
        static EXTRA: Dummy = Dummy(4);
        reg.register(loc("minecraft:extra"), &EXTRA);
    }

    #[test]
    #[should_panic]
    fn test_double_freeze_panics() {
        let mut reg = make_registry();
        reg.freeze();
        reg.freeze();
    }

    #[test]
    fn test_get_by_id_after_freeze() {
        let mut reg = make_registry();
        let id_a = reg.id_of("minecraft:a").unwrap();
        let id_b = reg.id_of("minecraft:b").unwrap();
        let id_c = reg.id_of("minecraft:c").unwrap();
        reg.freeze();
        assert_eq!(reg.get_by_id(id_a).unwrap().0, 1);
        assert_eq!(reg.get_by_id(id_b).unwrap().0, 2);
        assert_eq!(reg.get_by_id(id_c).unwrap().0, 3);
    }

    #[test]
    fn test_get_by_loc_after_freeze() {
        let mut reg = make_registry();
        reg.freeze();
        assert_eq!(reg.get_by_loc("minecraft:a").unwrap().0, 1);
        assert_eq!(reg.get_by_loc("minecraft:b").unwrap().0, 2);
        assert_eq!(reg.get_by_loc("minecraft:c").unwrap().0, 3);
    }

    #[test]
    fn test_id_of_value_returns_none_for_unknown() {
        let mut reg = make_registry();
        reg.freeze();
        assert!(reg.id_of_value(&DUMMY_UNKNOWN).is_none());
    }

    #[test]
    fn test_insertion_order_preserved() {
        let mut reg = make_registry();
        reg.freeze();
        let items: Vec<_> = reg.iter().collect();
        assert_eq!(items[0].0.raw(), 0);
        assert_eq!(items[0].2.0, 1);
        assert_eq!(items[1].0.raw(), 1);
        assert_eq!(items[1].2.0, 2);
        assert_eq!(items[2].0.raw(), 2);
        assert_eq!(items[2].2.0, 3);
    }

    #[test]
    fn test_frozen_returns_false_before_freeze() {
        let reg = StaticRegistry::<Dummy>::new();
        assert!(!reg.frozen());
    }

    #[test]
    fn test_frozen_returns_true_after_freeze() {
        let mut reg = make_registry();
        reg.freeze();
        assert!(reg.frozen());
    }
}
