use bevy_asset::{Asset, AssetId, Assets};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_nbt::tag::NbtTag;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_registry::NameTable;
use mcrs_minecraft_registry::shared::SharedResource;

/// A single entry in a frozen [`RegistrySnapshot`], carrying the
/// pre-serialized NBT and the original `AssetId` for reverse lookup.
#[derive(Debug, Clone)]
pub struct SnapshotEntry<T: Asset> {
    pub location: ResourceLocation<Arc<str>>,
    pub asset_id: AssetId<T>,
    pub nbt: NbtTag,
}

/// Stable network IDs assigned to all entries of a single dynamic
/// registry type once `AppState::WorldgenFreeze` is entered.
///
/// Entries are numbered by the registry loader's [`NameTable`]. The expensive
/// NBT serialization runs once at build time; per-client cost is a cheap
/// borrow.
#[derive(Resource, Debug)]
pub struct RegistrySnapshot<T: Asset> {
    entries: Arc<[SnapshotEntry<T>]>,
    by_asset: Arc<HashMap<AssetId<T>, u16>>,
    table: Option<Arc<NameTable>>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset> Clone for RegistrySnapshot<T> {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            by_asset: Arc::clone(&self.by_asset),
            table: self.table.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> SharedResource for RegistrySnapshot<T> {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.entries, &other.entries)
    }
}

impl<T: Asset> Default for RegistrySnapshot<T> {
    fn default() -> Self {
        Self {
            entries: Arc::default(),
            by_asset: Arc::default(),
            table: None,
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> RegistrySnapshot<T> {
    /// Build from an already-resolved `(ResourceLocation, AssetId)` iterator
    /// and an `&Assets<T>` for value lookup. Ids are the positions in `table`,
    /// and `serialize` runs once per entry.
    pub fn build<I, F>(
        table: &Arc<NameTable>,
        pairs: I,
        assets: &Assets<T>,
        mut serialize: F,
    ) -> Self
    where
        I: IntoIterator<Item = (ResourceLocation<Arc<str>>, AssetId<T>)>,
        F: FnMut(&T) -> Result<NbtTag, mcrs_minecraft_nbt::Error>,
    {
        let pairs: Vec<_> = pairs.into_iter().collect();
        assert_listing_matches(table, pairs.iter().map(|(location, _)| location));
        let mut slots = vec![None; table.len()];
        for (location, asset_id) in pairs {
            let id = table
                .number(location.as_str())
                .expect("the listing matches");
            if let Some((location, _)) = slots[usize::from(id)].replace((location, asset_id)) {
                panic!("{}: {location} is loaded twice", table.registry());
            }
        }

        let mut entries = Vec::with_capacity(slots.len());
        let mut by_asset = HashMap::with_capacity(slots.len());

        for (network_id, slot) in (0..=u16::MAX).zip(slots) {
            let (location, asset_id) = slot.expect("the listing matches");
            let Some(value) = assets.get(asset_id) else {
                panic!(
                    "{} is paired with an asset that is not present",
                    location.as_str()
                );
            };
            let nbt = serialize(value).unwrap_or_else(|e| {
                panic!("{} does not encode for the network: {e}", location.as_str())
            });
            by_asset.insert(asset_id, network_id);
            entries.push(SnapshotEntry {
                location,
                asset_id,
                nbt,
            });
        }

        Self {
            entries: entries.into(),
            by_asset: Arc::new(by_asset),
            table: Some(Arc::clone(table)),
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn by_asset_id(&self, id: AssetId<T>) -> Option<u16> {
        self.by_asset.get(&id).copied()
    }

    pub fn by_id(&self, network_id: u16) -> Option<&SnapshotEntry<T>> {
        self.entries.get(usize::from(network_id))
    }

    /// Resolve a network ID by resource location. Unlike [`by_asset_id`], this is
    /// stable across `AssetServer` instances (e.g. the host world vs. a per-dim
    /// sub-app), where the same biome carries different `AssetId`s.
    pub fn by_location(&self, location: &str) -> Option<u16> {
        self.table.as_ref()?.number(location)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u16, &SnapshotEntry<T>)> {
        (0..=u16::MAX).zip(self.entries.iter())
    }

    pub fn entries(&self) -> &[SnapshotEntry<T>] {
        &self.entries
    }
}

/// The id an asset at `<namespace>/<registry>/<name>.json` carries: everything
/// under the registry directory, folders included, so `worldgen/feature/coral/
/// tube_block.json` is `minecraft:coral/tube_block`. `registry` is the
/// directory or the registry key (`minecraft:worldgen/feature`); a path outside
/// it is `None`.
pub fn rl_from_asset_path(
    path: &std::path::Path,
    registry: &str,
) -> Option<ResourceLocation<Arc<str>>> {
    let registry = registry.split_once(':').map_or(registry, |(_, dir)| dir);
    let (namespace, rest) = path.to_str()?.split_once('/')?;
    let under = rest.strip_prefix(registry)?.strip_prefix('/')?;
    let name = under.strip_suffix(".json").unwrap_or(under);
    ResourceLocation::read(&format!("{namespace}:{name}")).ok()
}

pub fn assert_listing_matches<'a>(
    table: &NameTable,
    listed: impl IntoIterator<Item = &'a ResourceLocation<Arc<str>>>,
) {
    let registry = table.registry();
    let mut seen = vec![false; table.len()];
    for name in listed {
        let Some(id) = table.number(name.as_str()) else {
            panic!("{registry}: {name} is loaded but the registry loader has no such entry");
        };
        seen[usize::from(id)] = true;
    }
    if let Some(id) = seen.iter().position(|seen| !seen) {
        let name = table.name(id).expect("ids are dense");
        panic!("{registry}: the registry loader lists {name} but no asset is loaded for it");
    }
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
    ($app:expr, [ $( ($ty:ty, $registry_key:expr, $ser:expr, $pack_source:expr) ),* $(,)? ]) => {
        $(
            $app.init_resource::<$crate::RegistrySnapshot<$ty>>();
            $app.add_systems(
                ::bevy_state::state::OnEnter($crate::AppState::WorldgenFreeze),
                (
                    |
                        mut snapshot: ::bevy_ecs::system::ResMut<$crate::RegistrySnapshot<$ty>>,
                        assets: ::bevy_ecs::system::Res<::bevy_asset::Assets<$ty>>,
                        asset_server: ::bevy_ecs::system::Res<::bevy_asset::AssetServer>,
                        set: ::bevy_ecs::system::Res<::mcrs_minecraft_registry::RegistrySet>,
                    | {
                        let table = set.table($registry_key).unwrap_or_else(|| {
                            panic!("{} is not a loaded registry", $registry_key)
                        });
                        let pairs: Vec<(
                            ::mcrs_minecraft_core::ResourceLocation<::std::sync::Arc<str>>,
                            ::bevy_asset::AssetId<$ty>,
                        )> = assets
                            .iter()
                            .filter_map(|(asset_id, _)| {
                                let path = asset_server.get_path(asset_id)?;
                                let rl = $crate::snapshot::rl_from_asset_path(path.path(), $registry_key)?;
                                Some((rl, asset_id))
                            })
                            .collect();
                        let count = pairs.len();
                        *snapshot = $crate::RegistrySnapshot::<$ty>::build(table, pairs, &assets, $ser);
                        ::tracing::info!(
                            kind = ::std::any::type_name::<$ty>(),
                            entries = count,
                            "built RegistrySnapshot"
                        );
                    },
                    |
                        snapshot: ::bevy_ecs::system::Res<$crate::RegistrySnapshot<$ty>>,
                        mut access: ::bevy_ecs::system::ResMut<$crate::RegistryAccess>,
                        set: ::bevy_ecs::system::Res<::mcrs_minecraft_registry::RegistrySet>,
                    | {
                        let erased = $crate::RegistrySnapshotErased::from_dynamic(
                            $registry_key,
                            &snapshot,
                            &set,
                            $pack_source,
                        );
                        access.register(erased);
                    },
                ).chain(),
            );
        )*
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::compound::NbtCompound;

    #[derive(bevy_asset::Asset, bevy_reflect::TypePath)]
    struct TestBiome;

    fn make_pair(
        rl: &str,
        assets: &mut Assets<TestBiome>,
    ) -> (ResourceLocation<Arc<str>>, AssetId<TestBiome>) {
        let handle = assets.add(TestBiome);
        (ResourceLocation::read(rl).unwrap(), handle.id())
    }

    fn table(names: &[&str]) -> Arc<NameTable> {
        Arc::new(
            NameTable::new(
                ResourceLocation::read("minecraft:worldgen/biome").unwrap(),
                names
                    .iter()
                    .map(|name| ResourceLocation::read(name).unwrap()),
            )
            .unwrap(),
        )
    }

    #[test]
    fn build_follows_the_table_and_ignores_the_order_of_the_pairs() {
        let mut assets = Assets::<TestBiome>::default();
        let p_plains = make_pair("minecraft:plains", &mut assets);
        let p_desert = make_pair("minecraft:desert", &mut assets);
        let p_forest = make_pair("minecraft:forest", &mut assets);
        let table = table(&["minecraft:plains", "minecraft:forest", "minecraft:desert"]);

        let pairs_a = vec![p_plains.clone(), p_desert.clone(), p_forest.clone()];
        let pairs_b = vec![p_forest.clone(), p_plains.clone(), p_desert.clone()];

        let snap_a = RegistrySnapshot::<TestBiome>::build(&table, pairs_a, &assets, |_| {
            Ok(NbtCompound::new().into())
        });
        let snap_b = RegistrySnapshot::<TestBiome>::build(&table, pairs_b, &assets, |_| {
            Ok(NbtCompound::new().into())
        });

        assert_eq!(snap_a.by_asset_id(p_plains.1).unwrap(), 0);
        assert_eq!(snap_a.by_asset_id(p_forest.1).unwrap(), 1);
        assert_eq!(snap_a.by_asset_id(p_desert.1).unwrap(), 2);
        assert_eq!(snap_a.by_location("minecraft:forest"), Some(1));
        assert_eq!(snap_a.by_location("minecraft:taiga"), None);

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
        assert_eq!(
            locs_a,
            ["minecraft:plains", "minecraft:forest", "minecraft:desert"]
        );
        assert_eq!(locs_a, locs_b);
    }

    #[test]
    #[should_panic(expected = "minecraft:worldgen/biome: minecraft:taiga is loaded but")]
    fn a_pair_the_loader_does_not_list_does_not_build() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);
        let p2 = make_pair("minecraft:taiga", &mut assets);

        RegistrySnapshot::<TestBiome>::build(
            &table(&["minecraft:plains"]),
            vec![p1, p2],
            &assets,
            |_| Ok(NbtCompound::new().into()),
        );
    }

    #[test]
    #[should_panic(
        expected = "minecraft:worldgen/biome: the registry loader lists minecraft:forest"
    )]
    fn a_loader_name_without_an_asset_does_not_build() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);

        RegistrySnapshot::<TestBiome>::build(
            &table(&["minecraft:plains", "minecraft:forest"]),
            vec![p1],
            &assets,
            |_| Ok(NbtCompound::new().into()),
        );
    }

    #[test]
    fn bidirectional_mapping_roundtrips() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);
        let p2 = make_pair("minecraft:desert", &mut assets);
        let p3 = make_pair("minecraft:forest", &mut assets);

        let snapshot = RegistrySnapshot::<TestBiome>::build(
            &table(&["minecraft:desert", "minecraft:forest", "minecraft:plains"]),
            vec![p1.clone(), p2.clone(), p3.clone()],
            &assets,
            |_| Ok(NbtCompound::new().into()),
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

        let snapshot = RegistrySnapshot::<TestBiome>::build(
            &table(&["minecraft:desert", "minecraft:plains"]),
            vec![p1, p2],
            &assets,
            |_| {
                let mut nbt = NbtCompound::new();
                nbt.put_string("name", "test_value".to_owned());
                Ok(nbt.into())
            },
        );

        for (_, entry) in snapshot.iter() {
            let NbtTag::Compound(nbt) = &entry.nbt else {
                panic!("not a compound")
            };
            let name = nbt.get_string("name");
            assert_eq!(
                name,
                Some("test_value"),
                "serializer did not run for {}",
                entry.location.as_str()
            );
        }
    }

    #[test]
    #[should_panic(expected = "minecraft:forest")]
    fn a_snapshot_with_an_entry_missing_from_the_middle_does_not_build() {
        let mut assets = Assets::<TestBiome>::default();
        let p1 = make_pair("minecraft:plains", &mut assets);
        let p2 = make_pair("minecraft:desert", &mut assets);
        let p3 = make_pair("minecraft:forest", &mut assets);
        assets.remove(p3.1);

        RegistrySnapshot::<TestBiome>::build(
            &table(&["minecraft:desert", "minecraft:forest", "minecraft:plains"]),
            vec![p1, p2, p3],
            &assets,
            |_| Ok(NbtCompound::new().into()),
        );
    }

    #[test]
    fn rl_from_asset_path_keeps_the_folders_under_the_registry() {
        let p = std::path::Path::new("minecraft/worldgen/feature/coral/tube_block.json");
        let rl = rl_from_asset_path(p, "worldgen/feature").unwrap();
        assert_eq!(rl.as_str(), "minecraft:coral/tube_block");
    }
}
