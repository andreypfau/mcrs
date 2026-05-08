use bevy_asset::{Asset, AssetId, Assets};
use bevy_ecs::resource::Resource;
use mcrs_nbt::compound::NbtCompound;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::resource_location::ResourceLocation;

/// A single entry in a frozen [`RegistrySnapshot`], carrying the
/// pre-serialized NBT and the original `AssetId` for reverse lookup.
#[derive(Debug, Clone)]
pub struct SnapshotEntry<T: Asset> {
    pub location: ResourceLocation<Arc<str>>,
    pub asset_id: AssetId<T>,
    pub nbt: NbtCompound,
}

/// Stable `u32` network IDs assigned to all entries of a single dynamic
/// registry type once `AppState::WorldgenFreeze` is entered.
///
/// Entries are sorted alphabetically by [`ResourceLocation`] and assigned
/// dense IDs `0..N`. The expensive NBT serialization runs once at build time;
/// per-client cost is a cheap borrow.
#[derive(Resource, Debug)]
pub struct RegistrySnapshot<T: Asset> {
    entries: Vec<SnapshotEntry<T>>,
    by_asset: HashMap<AssetId<T>, u32>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset> Default for RegistrySnapshot<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            by_asset: HashMap::new(),
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> RegistrySnapshot<T> {
    /// Build from an already-resolved `(ResourceLocation, AssetId)` iterator
    /// and an `&Assets<T>` for value lookup.  Alphabetically sorts by
    /// `ResourceLocation`, assigns dense u32 IDs `0..N`, and invokes
    /// `serialize` once per entry at build time.
    pub fn build<I, F>(pairs: I, assets: &Assets<T>, mut serialize: F) -> Self
    where
        I: IntoIterator<Item = (ResourceLocation<Arc<str>>, AssetId<T>)>,
        F: FnMut(&T) -> Result<NbtCompound, mcrs_nbt::Error>,
    {
        let mut pairs: Vec<_> = pairs.into_iter().collect();
        pairs.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        let mut entries = Vec::with_capacity(pairs.len());
        let mut by_asset = HashMap::with_capacity(pairs.len());

        for (network_id, (location, asset_id)) in pairs.into_iter().enumerate() {
            let Some(value) = assets.get(asset_id) else {
                tracing::warn!(
                    rl = %location.as_str(),
                    "RegistrySnapshot::build skipping missing asset"
                );
                continue;
            };
            let nbt = match serialize(value) {
                Ok(nbt) => nbt,
                Err(e) => {
                    tracing::error!(
                        rl = %location.as_str(),
                        error = %e,
                        "RegistrySnapshot::build serializer failed"
                    );
                    NbtCompound::new()
                }
            };
            by_asset.insert(asset_id, network_id as u32);
            entries.push(SnapshotEntry {
                location,
                asset_id,
                nbt,
            });
        }

        Self {
            entries,
            by_asset,
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn by_asset_id(&self, id: AssetId<T>) -> Option<u32> {
        self.by_asset.get(&id).copied()
    }

    pub fn by_id(&self, network_id: u32) -> Option<&SnapshotEntry<T>> {
        self.entries.get(network_id as usize)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &SnapshotEntry<T>)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i as u32, e))
    }

    pub fn entries(&self) -> &[SnapshotEntry<T>] {
        &self.entries
    }
}

/// Convert a Bevy asset path into a `ResourceLocation`.
///
/// Asset path shape: `"<namespace>/<registry_dir...>/<name>.json"`.
/// The namespace is the first path component, the name is the file stem.
/// Registry directory segments (variable depth) are stripped.
pub fn rl_from_asset_path(path: &std::path::Path) -> Option<ResourceLocation<Arc<str>>> {
    let namespace = path.iter().next()?.to_str()?;
    let stem = path.file_stem()?.to_str()?;
    ResourceLocation::parse(&format!("{namespace}:{stem}")).ok()
}

/// Register `RegistrySnapshot<T>` resources and their WorldgenFreeze builder
/// systems for a list of dynamic registry types.
///
/// Each tuple `($ty, $registry_key, $ser)` expands to:
/// 1. `init_resource::<RegistrySnapshot<$ty>>()`
/// 2. Two chained `OnEnter(AppState::WorldgenFreeze)` systems:
///    - **Build**: iterates `Assets<$ty>`, maps asset paths to
///      `ResourceLocation`s, and builds the snapshot with `$ser`.
///    - **Register**: reads the built snapshot, creates a
///      `RegistrySnapshotErased`, and inserts it into `RegistryAccess`.
#[macro_export]
macro_rules! snapshot_registry {
    ($app:expr, [ $( ($ty:ty, $registry_key:expr, $ser:expr) ),* $(,)? ]) => {
        $(
            $app.init_resource::<$crate::RegistrySnapshot<$ty>>();
            $app.add_systems(
                ::bevy_state::state::OnEnter($crate::AppState::WorldgenFreeze),
                (
                    |
                        mut snapshot: ::bevy_ecs::system::ResMut<$crate::RegistrySnapshot<$ty>>,
                        assets: ::bevy_ecs::system::Res<::bevy_asset::Assets<$ty>>,
                        asset_server: ::bevy_ecs::system::Res<::bevy_asset::AssetServer>,
                    | {
                        let pairs: Vec<(
                            $crate::ResourceLocation<::std::sync::Arc<str>>,
                            ::bevy_asset::AssetId<$ty>,
                        )> = assets
                            .iter()
                            .filter_map(|(asset_id, _)| {
                                let path = asset_server.get_path(asset_id)?;
                                let rl = $crate::registry::snapshot::rl_from_asset_path(path.path())?;
                                Some((rl, asset_id))
                            })
                            .collect();
                        let count = pairs.len();
                        *snapshot = $crate::RegistrySnapshot::<$ty>::build(pairs, &assets, $ser);
                        ::tracing::info!(
                            kind = ::std::any::type_name::<$ty>(),
                            entries = count,
                            "built RegistrySnapshot"
                        );
                    },
                    |
                        snapshot: ::bevy_ecs::system::Res<$crate::RegistrySnapshot<$ty>>,
                        mut access: ::bevy_ecs::system::ResMut<$crate::RegistryAccess>,
                    | {
                        let erased = $crate::RegistrySnapshotErased::from_dynamic(
                            $registry_key,
                            &snapshot,
                        );
                        access.register(::std::boxed::Box::new(erased));
                    },
                ).chain(),
            );
        )*
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(bevy_asset::Asset, bevy_reflect::TypePath)]
    struct TestBiome;

    fn make_pair(
        rl: &str,
        assets: &mut Assets<TestBiome>,
    ) -> (ResourceLocation<Arc<str>>, AssetId<TestBiome>) {
        let handle = assets.add(TestBiome);
        (ResourceLocation::parse(rl).unwrap(), handle.id())
    }

    #[test]
    fn build_assigns_ids_to_all_entries() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);
        let p2 = make_pair("minecraft:desert", &mut assets);
        let p3 = make_pair("minecraft:forest", &mut assets);

        let snapshot = RegistrySnapshot::<TestBiome>::build(
            vec![p1.clone(), p2.clone(), p3.clone()],
            &assets,
            |_| Ok(NbtCompound::new()),
        );

        assert_eq!(snapshot.len(), 3);
        assert!(snapshot.by_asset_id(p1.1).is_some());
        assert!(snapshot.by_asset_id(p2.1).is_some());
        assert!(snapshot.by_asset_id(p3.1).is_some());
    }

    #[test]
    fn build_is_alphabetical_and_repeatable() {
        let mut assets = Assets::<TestBiome>::default();
        let p_plains = make_pair("minecraft:plains", &mut assets);
        let p_desert = make_pair("minecraft:desert", &mut assets);
        let p_forest = make_pair("minecraft:forest", &mut assets);

        let pairs_a = vec![p_plains.clone(), p_desert.clone(), p_forest.clone()];
        let pairs_b = vec![p_forest.clone(), p_plains.clone(), p_desert.clone()];

        let snap_a =
            RegistrySnapshot::<TestBiome>::build(pairs_a, &assets, |_| Ok(NbtCompound::new()));
        let snap_b =
            RegistrySnapshot::<TestBiome>::build(pairs_b, &assets, |_| Ok(NbtCompound::new()));

        assert_eq!(snap_a.by_asset_id(p_desert.1).unwrap(), 0);
        assert_eq!(snap_a.by_asset_id(p_forest.1).unwrap(), 1);
        assert_eq!(snap_a.by_asset_id(p_plains.1).unwrap(), 2);

        let locs_a: Vec<_> = snap_a
            .entries()
            .iter()
            .map(|e| e.location.as_str().to_owned())
            .collect();
        let locs_b: Vec<_> = snap_b
            .entries()
            .iter()
            .map(|e| e.location.as_str().to_owned())
            .collect();
        assert_eq!(locs_a, locs_b);
    }

    #[test]
    fn bidirectional_mapping_roundtrips() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);
        let p2 = make_pair("minecraft:desert", &mut assets);
        let p3 = make_pair("minecraft:forest", &mut assets);

        let snapshot = RegistrySnapshot::<TestBiome>::build(
            vec![p1.clone(), p2.clone(), p3.clone()],
            &assets,
            |_| Ok(NbtCompound::new()),
        );

        for (rl, aid) in [p1, p2, p3] {
            let net_id = snapshot.by_asset_id(aid).unwrap();
            let entry = snapshot.by_id(net_id).unwrap();
            assert_eq!(entry.asset_id, aid, "roundtrip failed for {}", rl.as_str());
        }
    }

    #[test]
    fn build_preserializes_nbt() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);
        let p2 = make_pair("minecraft:desert", &mut assets);

        let snapshot = RegistrySnapshot::<TestBiome>::build(vec![p1, p2], &assets, |_| {
            let mut nbt = NbtCompound::new();
            nbt.put_string("name", "test_value".to_owned());
            Ok(nbt)
        });

        for (_, entry) in snapshot.iter() {
            let name = entry.nbt.get_string("name");
            assert_eq!(
                name,
                Some("test_value"),
                "serializer did not run for {}",
                entry.location.as_str()
            );
        }
    }

    #[test]
    fn rl_from_asset_path_parses_minecraft_path() {
        let p = std::path::Path::new("minecraft/worldgen/biome/plains.json");
        let rl = rl_from_asset_path(p).unwrap();
        assert_eq!(rl.as_str(), "minecraft:plains");
    }

    #[test]
    fn rl_from_asset_path_nested_path() {
        let p = std::path::Path::new("minecraft/chat_type/msg_command_incoming.json");
        let rl = rl_from_asset_path(p).unwrap();
        assert_eq!(rl.as_str(), "minecraft:msg_command_incoming");
    }

    #[test]
    fn rl_from_asset_path_no_extension_still_works() {
        let p = std::path::Path::new("minecraft/chat_type/chat");
        let rl = rl_from_asset_path(p).unwrap();
        assert_eq!(rl.as_str(), "minecraft:chat");
    }
}
