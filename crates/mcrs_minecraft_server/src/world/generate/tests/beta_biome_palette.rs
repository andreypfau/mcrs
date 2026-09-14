use std::sync::Arc;

use bevy_asset::Assets;
use mcrs_minecraft_assets::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::source::{BiomeSource, build_beta_lookup_table};
use mcrs_minecraft_core::resource_location::ResourceLocation;

use super::build_beta_router;
use crate::world::chunk::CancellationToken;
use crate::world::generate::generate_column;

pub(super) fn make_beta_biome() -> Biome {
    Biome {
        temperature: 0.5,
        downfall: 0.5,
        has_precipitation: true,
        temperature_modifier: None,
        effects: mcrs_minecraft_biome::BiomeEffects {
            water_color: None,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            dry_foliage_color: None,
        },
        carvers: Vec::new(),
        features: Vec::new(),
        attributes: Default::default(),
    }
}

/// A Beta column's biome is one of the source's eleven land biomes, resolved by
/// location to the same network id its asset has; a column with no biome
/// context keeps the default palette.
#[test]
fn generate_column_beta_biome_not_default() {
    let router = build_beta_router();

    let mut assets = Assets::<Biome>::default();
    let land_handles: Vec<_> = (0..11).map(|_| assets.add(make_beta_biome())).collect();
    let land_ids: Vec<_> = land_handles.iter().map(|h| h.id()).collect();
    let land_biome_ids: [ResourceLocation<Arc<str>>; 11] = std::array::from_fn(|i| {
        ResourceLocation::parse(&format!("minecraft:land_biome_{i}")).unwrap()
    });
    let snapshot = RegistrySnapshot::<Biome>::build(
        land_biome_ids
            .iter()
            .cloned()
            .zip(land_ids.iter().copied())
            .collect::<Vec<_>>(),
        &assets,
        |_| Ok(mcrs_minecraft_nbt::compound::NbtCompound::new()),
    );
    assert_eq!(
        snapshot.len(),
        11,
        "RegistrySnapshot must contain the 11 land biomes"
    );

    let biome_source = BiomeSource::Beta {
        land_biomes: land_handles.try_into().expect("11 land handles"),
        land_biome_ids: land_biome_ids.clone(),
        lookup: Box::new(build_beta_lookup_table()),
    };

    let (temp_0, hum_0) = router.sample_beta_climate(
        &mut mcrs_minecraft_worldgen::program::Workspace::new(),
        0,
        0,
    );
    let land_loc = biome_source.beta_biome_location(temp_0, hum_0);
    let bucket = land_biome_ids
        .iter()
        .position(|id| id == land_loc)
        .expect("the location is one of the land biomes");
    assert_eq!(
        snapshot.by_location(land_loc.as_str()),
        snapshot.by_asset_id(land_ids[bucket]),
        "location-based biome resolution must match asset-based resolution"
    );

    let y_sections: Vec<i32> = (-3..=7).collect();
    let cancel = CancellationToken::new();

    let results = generate_column(
        0,
        0,
        &y_sections,
        &router,
        Some((&biome_source, &snapshot)),
        None,
        &cancel,
    );

    assert_eq!(results.len(), y_sections.len());
    for (idx, result) in results.iter().enumerate() {
        assert!(
            result.is_some(),
            "section at y={} must not be cancelled",
            y_sections[idx]
        );
    }

    // Verify modern path: with no biome_context, all palette cells default to 0.
    let results_modern = generate_column(0, 0, &[0, 1, 2, 3, 4, 5], &router, None, None, &cancel);
    for (idx, r) in results_modern.iter().enumerate() {
        let (_, biomes) = r.as_ref().expect("modern section must not be cancelled");
        assert!(
            matches!(
                biomes.0,
                mcrs_minecraft_chunk::PalettedContainer::Homogeneous(0)
            ),
            "modern path section y={} must produce default (all-zero) BiomePalette",
            idx
        );
    }
}
