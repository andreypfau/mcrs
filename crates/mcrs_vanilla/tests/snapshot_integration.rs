use std::collections::HashMap;
use std::sync::Arc;

use bevy_asset::Assets;
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_vanilla::biome::{Biome, BiomeEffects, BiomeSpawners, NetworkBiome};

fn fixture_biome() -> Biome {
    Biome {
        temperature: 0.8,
        downfall: 0.4,
        has_precipitation: true,
        effects: BiomeEffects {
            water_color: None,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            dry_foliage_color: None,
        },
        carvers: Vec::new(),
        features: Vec::new(),
        spawners: BiomeSpawners {
            ambient: Vec::new(),
            axolotls: Vec::new(),
            creature: Vec::new(),
            misc: Vec::new(),
            monster: Vec::new(),
            underground_water_creature: Vec::new(),
            water_ambient: Vec::new(),
            water_creature: Vec::new(),
        },
        spawn_costs: HashMap::new(),
        attributes: None,
    }
}

#[test]
fn snapshot_integration_biome_populated_with_in_memory_fixtures() {
    let mut assets = Assets::<Biome>::default();
    let plains_id = assets.add(fixture_biome()).id();
    let desert_id = assets.add(fixture_biome()).id();
    let ocean_id = assets.add(fixture_biome()).id();

    let plains_rl: ResourceLocation<Arc<str>> =
        ResourceLocation::parse("minecraft:plains").unwrap();
    let desert_rl: ResourceLocation<Arc<str>> =
        ResourceLocation::parse("minecraft:desert").unwrap();
    let ocean_rl: ResourceLocation<Arc<str>> =
        ResourceLocation::parse("minecraft:ocean").unwrap();

    let pairs = vec![
        (plains_rl.clone(), plains_id),
        (ocean_rl.clone(), ocean_id),
        (desert_rl.clone(), desert_id),
    ];
    let snapshot = RegistrySnapshot::<Biome>::build(pairs, &assets, |b: &Biome| {
        mcrs_nbt::to_nbt_compound(&NetworkBiome::from(b))
    });

    // SNAP-01: all three entries present
    assert_eq!(snapshot.len(), 3, "expected 3 biome entries");

    // SNAP-02: alphabetical order, dense IDs 0..N
    let entry0 = snapshot.by_id(0).expect("id 0 must exist");
    let entry1 = snapshot.by_id(1).expect("id 1 must exist");
    let entry2 = snapshot.by_id(2).expect("id 2 must exist");
    assert_eq!(entry0.location.as_str(), "minecraft:desert");
    assert_eq!(entry1.location.as_str(), "minecraft:ocean");
    assert_eq!(entry2.location.as_str(), "minecraft:plains");

    // SNAP-03: bidirectional roundtrip via AssetId<T>
    assert_eq!(snapshot.by_asset_id(desert_id), Some(0));
    assert_eq!(snapshot.by_asset_id(ocean_id), Some(1));
    assert_eq!(snapshot.by_asset_id(plains_id), Some(2));
    assert_eq!(snapshot.by_id(0).unwrap().asset_id, desert_id);

    // SNAP-04: NBT was pre-serialized at build time and is non-empty
    let nbt = &snapshot.by_id(0).unwrap().nbt;
    assert!(
        !nbt.is_empty(),
        "SnapshotEntry.nbt must be a populated NbtCompound, got empty"
    );

    // Determinism: rebuild from different insertion order yields same result
    let shuffled = vec![
        (desert_rl, desert_id),
        (plains_rl, plains_id),
        (ocean_rl, ocean_id),
    ];
    let snapshot2 = RegistrySnapshot::<Biome>::build(shuffled, &assets, |b: &Biome| {
        mcrs_nbt::to_nbt_compound(&NetworkBiome::from(b))
    });
    for i in 0..3u32 {
        assert_eq!(
            snapshot.by_id(i).unwrap().location.as_str(),
            snapshot2.by_id(i).unwrap().location.as_str(),
            "rebuild changed ordering at id {i}",
        );
    }
}
