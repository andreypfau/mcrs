use bevy_ecs::resource::Resource;
use std::collections::HashMap;

/// Stable `u32` network IDs assigned to all dynamic registry entries once
/// `AppState::WorldgenFreeze` is entered.
///
/// These IDs are sent to clients in the Configuration phase via
/// `ClientboundRegistryDataPacket`.  They stay constant for the lifetime of
/// the running server (re-assigned on reconfiguration).
///
/// This is currently a stub — full population logic lives in `mc_server`.
#[derive(Resource, Debug, Default)]
pub struct RegistrySnapshot {
    /// registry key (e.g. "minecraft:worldgen/biome") → (entry RL → protocol id)
    pub entries: HashMap<String, HashMap<String, u32>>,
}

impl RegistrySnapshot {
    pub fn new() -> Self {
        RegistrySnapshot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_location::ResourceLocation;
    use bevy_asset::{AssetId, Assets};
    use mcrs_nbt::compound::NbtCompound;
    use std::sync::Arc;

    #[derive(bevy_asset::Asset, bevy_reflect::TypePath)]
    struct TestBiome;

    fn make_pair(
        rl: &str,
        assets: &mut Assets<TestBiome>,
    ) -> (ResourceLocation<Arc<str>>, AssetId<TestBiome>) {
        let id = assets.add(TestBiome);
        (ResourceLocation::parse(rl).unwrap(), id)
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

        let snap_a = RegistrySnapshot::<TestBiome>::build(
            pairs_a,
            &assets,
            |_| Ok(NbtCompound::new()),
        );
        let snap_b = RegistrySnapshot::<TestBiome>::build(
            pairs_b,
            &assets,
            |_| Ok(NbtCompound::new()),
        );

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

        let snapshot = RegistrySnapshot::<TestBiome>::build(
            vec![p1, p2],
            &assets,
            |_| {
                let mut nbt = NbtCompound::new();
                nbt.put_string("name", "test_value".to_owned());
                Ok(nbt)
            },
        );

        for (_, entry) in snapshot.iter() {
            let name = entry.nbt.get_string("name");
            assert_eq!(name, Some("test_value"), "serializer did not run for {}", entry.location.as_str());
        }
    }
}
