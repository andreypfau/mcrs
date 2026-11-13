use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::tag_deserializer::from_tag;
use mcrs_minecraft_nbt::tag_serializer::to_nbt_tag;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

/// Where the state of a registered component goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistScope {
    /// The component goes into the player file and into a dimension handover.
    Persisted,
    /// The component goes into a dimension handover only. The player file never holds it and
    /// never overwrites it.
    TravelOnly,
}

/// A saved component held as NBT, under the root keys of the player file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PersistedSnapshot {
    fields: BTreeMap<&'static str, NbtTag>,
    unreadable: bool,
}

impl IntoIterator for PersistedSnapshot {
    type Item = (&'static str, NbtTag);
    type IntoIter = std::collections::btree_map::IntoIter<&'static str, NbtTag>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl PersistedSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of a player file that could not be read. It holds no file key, and applying
    /// it marks the player so that no save overwrites the file.
    #[must_use]
    pub fn unreadable() -> Self {
        Self {
            unreadable: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_unreadable(&self) -> bool {
        self.unreadable
    }

    pub fn insert(&mut self, key: &'static str, tag: NbtTag) -> Option<NbtTag> {
        self.fields.insert(key, tag)
    }

    pub fn remove(&mut self, key: &str) -> Option<NbtTag> {
        self.fields.remove(key)
    }

    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        self.fields.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &NbtTag)> {
        self.fields.iter().map(|(key, tag)| (*key, tag))
    }

    /// The value under `keys`, `None` when the snapshot holds none of them. A value over one key
    /// is that key's tag. A value over several keys is a compound of the keys that are present.
    pub fn get_as<T: DeserializeOwned>(
        &self,
        keys: &[&'static str],
    ) -> Result<Option<T>, PersistedError> {
        let Some(tag) = self.joined(keys) else {
            return Ok(None);
        };
        decode(keys.first().copied().unwrap_or_default(), tag).map(Some)
    }

    /// Stores `value` under `keys`: a value over several keys must serialize to a compound that
    /// has those keys as fields.
    pub fn put<T: Serialize>(
        &mut self,
        keys: &[&'static str],
        value: &T,
    ) -> Result<(), PersistedError> {
        let key = keys.first().copied().unwrap_or_default();
        let tag = to_nbt_tag(value).map_err(|source| PersistedError::Encode { key, source })?;
        self.put_tag(keys, tag)
    }

    fn put_tag(&mut self, keys: &[&'static str], tag: NbtTag) -> Result<(), PersistedError> {
        match (keys, tag) {
            ([key], tag) => {
                self.fields.insert(key, tag);
            }
            (keys, NbtTag::Compound(compound)) => {
                for (name, tag) in compound.child_tags {
                    let key = keys
                        .iter()
                        .copied()
                        .find(|key| *key == name)
                        .ok_or(PersistedError::UnknownKey(name))?;
                    self.fields.insert(key, tag);
                }
            }
            _ => return Err(PersistedError::NotACompound(keys.join(","))),
        }
        Ok(())
    }

    fn joined(&self, keys: &[&'static str]) -> Option<NbtTag> {
        match keys {
            [] => None,
            [key] => self.fields.get(key).cloned(),
            keys => self.joined_compound(keys),
        }
    }

    fn joined_compound(&self, keys: &[&'static str]) -> Option<NbtTag> {
        let mut compound = NbtCompound::new();
        for key in keys {
            if let Some(tag) = self.fields.get(key) {
                compound.put(key, tag.clone());
            }
        }
        (!compound.child_tags.is_empty()).then_some(NbtTag::Compound(compound))
    }
}

fn filter_tag(keys: &[&'static str], tag: NbtTag) -> Result<NbtTag, PersistedError> {
    let NbtTag::Compound(compound) = tag else {
        return Err(PersistedError::NotACompound(keys.join(",")));
    };
    let mut filtered = NbtCompound::new();
    for (name, tag) in compound.child_tags {
        if keys.contains(&name.as_str()) {
            filtered.put(&name, tag);
        }
    }
    match keys {
        [] => Ok(NbtTag::Compound(filtered)),
        [key] => filtered
            .child_tags
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, tag)| tag.clone())
            .map_or_else(|| Ok(NbtTag::Compound(filtered)), Ok),
        _ => Ok(NbtTag::Compound(filtered)),
    }
}

fn decode<T: DeserializeOwned>(key: &'static str, tag: NbtTag) -> Result<T, PersistedError> {
    from_tag(tag).map_err(|source| PersistedError::Decode { key, source })
}

/// The persisted form of a component that does not derive serde itself, or whose saved shape is
/// not its in-memory shape.
pub trait PersistedForm<T>: Serialize + DeserializeOwned {
    fn from_value(value: &T) -> Self;

    fn into_value(self) -> Result<T, String>;
}

type Capture = fn(&mut World, Entity) -> Result<Option<NbtTag>, PersistedError>;
type Apply = fn(&mut World, Entity, &NbtTag) -> Result<(), PersistedError>;

#[derive(Clone, Copy)]
struct PersistedCodec {
    keys: &'static [&'static str],
    scope: PersistScope,
    fallback: bool,
    capture: Capture,
    apply: Apply,
}

/// The components that a snapshot of a player holds, each under its root keys of the player file.
/// A plugin adds its own with [`PersistedAppExt::register_persisted`] in the main app and in each
/// dimension app.
#[derive(Default, Resource)]
pub struct PersistedRegistry {
    codecs: Vec<PersistedCodec>,
}

impl PersistedRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a codec over `keys`. A codec over exactly the same keys is replaced.
    ///
    /// # Errors
    ///
    /// [`PersistedError::OverlappingKeys`] when a codec shares some but not all keys with `keys`.
    /// The registry stays as it was.
    pub fn register_codec(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
        capture: Capture,
        apply: Apply,
    ) -> Result<(), PersistedError> {
        self.register_with_fallback(keys, scope, false, capture, apply)
    }

    /// Registers a codec that owns the keys no other codec claims. A later codec over any of its
    /// keys takes those keys over, so one component can hold the keys of the file that no
    /// component of the game simulates yet.
    ///
    /// # Errors
    ///
    /// [`PersistedError::OverlappingKeys`] when another fallback codec is registered.
    pub fn register_fallback<T>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> Result<(), PersistedError>
    where
        T: Component + Serialize + DeserializeOwned,
    {
        self.register_with_fallback(
            keys,
            scope,
            true,
            capture_component::<T>,
            apply_component::<T>,
        )
    }

    fn register_with_fallback(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
        fallback: bool,
        capture: Capture,
        apply: Apply,
    ) -> Result<(), PersistedError> {
        for codec in &self.codecs {
            let overlaps = codec.keys.iter().any(|key| keys.contains(key));
            if fallback && codec.fallback && overlaps {
                return Err(PersistedError::OverlappingKeys {
                    keys,
                    existing: codec.keys,
                });
            }
            if !fallback && !codec.fallback && overlaps && codec.keys != keys {
                return Err(PersistedError::OverlappingKeys {
                    keys,
                    existing: codec.keys,
                });
            }
        }
        self.codecs
            .retain(|codec| codec.fallback != fallback || codec.keys != keys);
        self.codecs.push(PersistedCodec {
            keys,
            scope,
            fallback,
            capture,
            apply,
        });
        Ok(())
    }

    fn specific_keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.codecs
            .iter()
            .filter(|codec| !codec.fallback)
            .flat_map(|codec| codec.keys.iter().copied())
    }

    fn available_keys(&self, codec: &PersistedCodec) -> Vec<&'static str> {
        if codec.fallback {
            let claimed: Vec<_> = self.specific_keys().collect();
            codec
                .keys
                .iter()
                .copied()
                .filter(|key| !claimed.contains(key))
                .collect()
        } else {
            codec.keys.to_vec()
        }
    }

    /// See [`Self::register_codec`] for the errors.
    pub fn register<T>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> Result<(), PersistedError>
    where
        T: Component + Serialize + DeserializeOwned,
    {
        self.register_codec(keys, scope, capture_component::<T>, apply_component::<T>)
    }

    /// See [`Self::register_codec`] for the errors.
    pub fn register_as<T, F>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> Result<(), PersistedError>
    where
        T: Component,
        F: PersistedForm<T>,
    {
        self.register_codec(keys, scope, capture_form::<T, F>, apply_form::<T, F>)
    }

    /// The root keys of the player file that a [`PersistScope::Persisted`] codec claims.
    pub fn file_keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.codecs
            .iter()
            .filter(|codec| codec.scope == PersistScope::Persisted)
            .flat_map(|codec| self.available_keys(codec))
    }

    pub fn file_key(&self, key: &str) -> Option<&'static str> {
        self.file_keys().find(|claimed| *claimed == key)
    }

    /// Every registered component of `entity`, for a handover.
    pub fn capture(
        &self,
        world: &mut World,
        entity: Entity,
    ) -> Result<PersistedSnapshot, PersistedError> {
        self.capture_scoped(world, entity, |_| true)
    }

    /// The [`PersistScope::Persisted`] components of `entity`, for the player file.
    pub fn capture_file(
        &self,
        world: &mut World,
        entity: Entity,
    ) -> Result<PersistedSnapshot, PersistedError> {
        self.capture_scoped(world, entity, |scope| scope == PersistScope::Persisted)
    }

    fn capture_scoped(
        &self,
        world: &mut World,
        entity: Entity,
        wanted: impl Fn(PersistScope) -> bool,
    ) -> Result<PersistedSnapshot, PersistedError> {
        let mut snapshot = PersistedSnapshot::new();
        for codec in self.codecs.iter().filter(|codec| wanted(codec.scope)) {
            let keys = self.available_keys(codec);
            if keys.is_empty() {
                continue;
            }
            if let Some(tag) = (codec.capture)(world, entity)? {
                let tag = if codec.fallback {
                    filter_tag(&keys, tag)?
                } else {
                    tag
                };
                snapshot.put_tag(&keys, tag)?;
            }
        }
        Ok(snapshot)
    }

    pub fn apply(
        &self,
        world: &mut World,
        entity: Entity,
        snapshot: &PersistedSnapshot,
    ) -> Result<(), PersistedError> {
        for key in snapshot.fields.keys() {
            if !self
                .codecs
                .iter()
                .any(|codec| self.available_keys(codec).contains(key))
            {
                return Err(PersistedError::UnknownKey((*key).to_owned()));
            }
        }
        for codec in &self.codecs {
            let keys = self.available_keys(codec);
            let tag = if codec.fallback {
                snapshot.joined_compound(&keys)
            } else {
                snapshot.joined(&keys)
            };
            let Some(tag) = tag else {
                continue;
            };
            (codec.apply)(world, entity, &tag)?;
        }
        Ok(())
    }
}

/// Registers persisted components on an app. A plugin calls these in the main app and in each
/// dimension app that holds the component.
///
/// A plugin build cannot return an error, so a registration that [`PersistedRegistry::register_codec`]
/// refuses is logged and skipped. The keys of the skipped codec then fail the load of a player
/// file as unclaimed. Call the registry directly to handle the error.
pub trait PersistedAppExt {
    fn register_persisted<T>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> &mut Self
    where
        T: Component + Serialize + DeserializeOwned;

    fn register_persisted_fallback<T>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> &mut Self
    where
        T: Component + Serialize + DeserializeOwned;

    fn register_persisted_as<T, F>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> &mut Self
    where
        T: Component,
        F: PersistedForm<T>;

    fn register_persisted_codec(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
        capture: Capture,
        apply: Apply,
    ) -> &mut Self;

    /// Adds a plugin that registers persisted components to this app and to every dimension app.
    fn add_persistence_plugin(&mut self, plugin: impl Plugin + Clone) -> &mut Self;
}

impl PersistedAppExt for App {
    fn register_persisted<T>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> &mut Self
    where
        T: Component + Serialize + DeserializeOwned,
    {
        self.world_mut()
            .get_resource_or_init::<PersistedRegistry>()
            .register::<T>(keys, scope)
            .unwrap_or_else(|err| tracing::error!("persisted codec not registered: {err}"));
        self
    }

    fn register_persisted_fallback<T>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> &mut Self
    where
        T: Component + Serialize + DeserializeOwned,
    {
        self.world_mut()
            .get_resource_or_init::<PersistedRegistry>()
            .register_fallback::<T>(keys, scope)
            .unwrap_or_else(|err| tracing::error!("persisted codec not registered: {err}"));
        self
    }

    fn register_persisted_as<T, F>(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
    ) -> &mut Self
    where
        T: Component,
        F: PersistedForm<T>,
    {
        self.world_mut()
            .get_resource_or_init::<PersistedRegistry>()
            .register_as::<T, F>(keys, scope)
            .unwrap_or_else(|err| tracing::error!("persisted codec not registered: {err}"));
        self
    }

    fn register_persisted_codec(
        &mut self,
        keys: &'static [&'static str],
        scope: PersistScope,
        capture: Capture,
        apply: Apply,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<PersistedRegistry>()
            .register_codec(keys, scope, capture, apply)
            .unwrap_or_else(|err| tracing::error!("persisted codec not registered: {err}"));
        self
    }

    fn add_persistence_plugin(&mut self, plugin: impl Plugin + Clone) -> &mut Self {
        self.add_plugins(plugin.clone());
        self.add_dimension_plugin(plugin)
    }
}

fn capture_component<T>(world: &mut World, entity: Entity) -> Result<Option<NbtTag>, PersistedError>
where
    T: Component + Serialize,
{
    let Some(component) = world.get::<T>(entity) else {
        return Ok(None);
    };
    to_nbt_tag(component)
        .map(Some)
        .map_err(|source| PersistedError::Encode {
            key: std::any::type_name::<T>(),
            source,
        })
}

fn apply_component<T>(world: &mut World, entity: Entity, tag: &NbtTag) -> Result<(), PersistedError>
where
    T: Component + DeserializeOwned,
{
    let component: T = decode(std::any::type_name::<T>(), tag.clone())?;
    insert(world, entity, component)
}

fn capture_form<T, F>(world: &mut World, entity: Entity) -> Result<Option<NbtTag>, PersistedError>
where
    T: Component,
    F: PersistedForm<T>,
{
    let Some(component) = world.get::<T>(entity) else {
        return Ok(None);
    };
    to_nbt_tag(&F::from_value(component))
        .map(Some)
        .map_err(|source| PersistedError::Encode {
            key: std::any::type_name::<T>(),
            source,
        })
}

fn apply_form<T, F>(world: &mut World, entity: Entity, tag: &NbtTag) -> Result<(), PersistedError>
where
    T: Component,
    F: PersistedForm<T>,
{
    let key = std::any::type_name::<T>();
    let form: F = decode(key, tag.clone())?;
    let component = form
        .into_value()
        .map_err(|message| PersistedError::Decode {
            key,
            source: mcrs_minecraft_nbt::Error::SerdeError(message),
        })?;
    insert(world, entity, component)
}

fn insert<T: Component>(
    world: &mut World,
    entity: Entity,
    component: T,
) -> Result<(), PersistedError> {
    world
        .get_entity_mut(entity)
        .map_err(|_| PersistedError::NoEntity(entity))?
        .insert(component);
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PersistedError {
    #[error("no entity {0:?}")]
    NoEntity(Entity),
    #[error("the key {0} is not registered")]
    UnknownKey(String),
    #[error("the value over the keys {0} is not a compound")]
    NotACompound(String),
    #[error("no persisted-component registry")]
    NoRegistry,
    #[error("{path}: no component claims the key `{key}`")]
    UnclaimedFileKey { path: String, key: String },
    #[error(
        "the codec over {keys:?} shares some keys, but not all, with the codec over {existing:?}"
    )]
    OverlappingKeys {
        keys: &'static [&'static str],
        existing: &'static [&'static str],
    },
    #[error("the player has no {0}")]
    Incomplete(&'static str),
    #[error("encoding {key}: {source}")]
    Encode {
        key: &'static str,
        source: mcrs_minecraft_nbt::Error,
    },
    #[error("decoding {key}: {source}")]
    Decode {
        key: &'static str,
        source: mcrs_minecraft_nbt::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;
    use serde::{Deserialize, Serialize};

    #[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Counter(u32);

    #[test]
    fn a_codec_that_shares_some_keys_with_another_is_refused_and_changes_nothing() {
        let mut registry = PersistedRegistry::new();
        registry
            .register::<Counter>(&["X", "Y"], PersistScope::Persisted)
            .unwrap();
        let refused = registry.register::<Counter>(&["X"], PersistScope::Persisted);

        assert!(matches!(
            refused,
            Err(PersistedError::OverlappingKeys {
                keys: ["X"],
                existing: ["X", "Y"]
            })
        ));
        assert_eq!(registry.file_keys().collect::<Vec<_>>(), ["X", "Y"]);
    }

    #[test]
    fn a_codec_over_the_same_keys_replaces_the_first() {
        let mut registry = PersistedRegistry::new();
        registry
            .register::<Counter>(&["X", "Y"], PersistScope::TravelOnly)
            .unwrap();
        registry
            .register::<Counter>(&["X", "Y"], PersistScope::Persisted)
            .unwrap();
        assert_eq!(registry.file_keys().collect::<Vec<_>>(), ["X", "Y"]);
    }

    #[test]
    fn a_registered_component_survives_a_snapshot_round_trip() {
        let mut world = World::new();
        let entity = world.spawn(Counter(7)).id();
        let mut registry = PersistedRegistry::new();
        registry
            .register::<Counter>(&["test:counter"], PersistScope::Persisted)
            .unwrap();

        let snapshot = registry.capture(&mut world, entity).unwrap();
        world.despawn(entity);
        let entity = world.spawn_empty().id();
        registry.apply(&mut world, entity, &snapshot).unwrap();

        assert_eq!(world.get::<Counter>(entity), Some(&Counter(7)));
    }

    #[test]
    fn a_component_that_is_not_there_is_not_saved() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut registry = PersistedRegistry::new();
        registry
            .register::<Counter>(&["test:counter"], PersistScope::Persisted)
            .unwrap();

        let snapshot = registry.capture(&mut world, entity).unwrap();

        assert!(snapshot.is_empty());
    }

    #[test]
    fn an_unknown_snapshot_key_is_an_error() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let registry = PersistedRegistry::new();
        let mut snapshot = PersistedSnapshot::new();
        snapshot.insert("test:unknown", NbtTag::Int(1));

        let error = registry.apply(&mut world, entity, &snapshot).unwrap_err();

        assert!(matches!(error, PersistedError::UnknownKey(key) if key == "test:unknown"));
    }

    #[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Spot {
        #[serde(rename = "X")]
        x: i32,
        #[serde(rename = "Y")]
        y: i32,
    }

    #[test]
    fn an_unknown_snapshot_key_is_an_error_even_with_a_fallback() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut registry = PersistedRegistry::new();
        registry
            .register_fallback::<Fallback>(&["X", "Y"], PersistScope::Persisted)
            .unwrap();
        let mut snapshot = PersistedSnapshot::new();
        snapshot.insert("Z", NbtTag::Int(1));

        let error = registry.apply(&mut world, entity, &snapshot).unwrap_err();

        assert!(matches!(error, PersistedError::UnknownKey(key) if key == "Z"));
    }

    #[test]
    fn a_component_over_two_keys_is_stored_under_each_key() {
        let mut world = World::new();
        let entity = world.spawn(Spot { x: 1, y: 2 }).id();
        let mut registry = PersistedRegistry::new();
        registry
            .register::<Spot>(&["X", "Y"], PersistScope::Persisted)
            .unwrap();

        let snapshot = registry.capture(&mut world, entity).unwrap();
        assert_eq!(snapshot.get("X"), Some(&NbtTag::Int(1)));
        assert_eq!(snapshot.get("Y"), Some(&NbtTag::Int(2)));

        let other = world.spawn_empty().id();
        registry.apply(&mut world, other, &snapshot).unwrap();
        assert_eq!(world.get::<Spot>(other), Some(&Spot { x: 1, y: 2 }));
    }

    #[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Fallback {
        #[serde(rename = "X")]
        x: i32,
        #[serde(rename = "Y")]
        y: i32,
    }

    #[test]
    fn a_fallback_keeps_only_the_keys_no_other_codec_claims() {
        let mut world = World::new();
        let entity = world.spawn((Counter(1), Fallback { x: 2, y: 3 })).id();
        let mut registry = PersistedRegistry::new();
        registry
            .register_fallback::<Fallback>(&["X", "Y"], PersistScope::Persisted)
            .unwrap();
        registry
            .register::<Counter>(&["X"], PersistScope::Persisted)
            .unwrap();

        let snapshot = registry.capture(&mut world, entity).unwrap();
        assert_eq!(snapshot.get("X"), Some(&NbtTag::Int(1)));
        assert_eq!(snapshot.get("Y"), Some(&NbtTag::Int(3)));
        assert_eq!(registry.file_keys().collect::<Vec<_>>(), ["Y", "X"]);

        let other = world.spawn_empty().id();
        registry.apply(&mut world, other, &snapshot).unwrap();
        assert_eq!(world.get::<Counter>(other), Some(&Counter(1)));
        assert_eq!(world.get::<Fallback>(other), Some(&Fallback { x: 0, y: 3 }));
    }

    #[test]
    fn a_fallback_with_no_keys_left_does_not_write_its_component() {
        let mut world = World::new();
        let entity = world.spawn((Counter(1), Fallback { x: 2, y: 3 })).id();
        let mut registry = PersistedRegistry::new();
        registry
            .register_fallback::<Fallback>(&["X"], PersistScope::Persisted)
            .unwrap();
        registry
            .register::<Counter>(&["X"], PersistScope::Persisted)
            .unwrap();

        let snapshot = registry.capture(&mut world, entity).unwrap();
        assert!(snapshot.get("X").is_some());
        assert_eq!(registry.file_keys().collect::<Vec<_>>(), ["X"]);
    }

    #[test]
    fn two_fallbacks_over_the_same_key_are_refused() {
        let mut registry = PersistedRegistry::new();
        registry
            .register_fallback::<Counter>(&["X"], PersistScope::Persisted)
            .unwrap();
        let refused = registry.register_fallback::<Counter>(&["X"], PersistScope::Persisted);

        assert!(matches!(
            refused,
            Err(PersistedError::OverlappingKeys {
                keys: ["X"],
                existing: ["X"]
            })
        ));
    }

    #[test]
    fn a_travel_only_component_is_not_in_the_file_capture() {
        let mut world = World::new();
        let entity = world.spawn(Counter(7)).id();
        let mut registry = PersistedRegistry::new();
        registry
            .register::<Counter>(&["test:counter"], PersistScope::TravelOnly)
            .unwrap();

        assert!(
            registry
                .capture_file(&mut world, entity)
                .unwrap()
                .is_empty()
        );
        assert!(!registry.capture(&mut world, entity).unwrap().is_empty());
        assert_eq!(registry.file_key("test:counter"), None);
    }
}
