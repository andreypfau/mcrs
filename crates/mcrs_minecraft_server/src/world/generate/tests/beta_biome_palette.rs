use mcrs_minecraft_block::palette::NetworkPalette;
use std::sync::Arc;

use bevy_asset::Assets;
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::source::{BiomeSource, build_beta_lookup_table};

use super::build_beta_router;
use crate::world::chunk::CancellationToken;
use crate::world::generate::generate_column;

fn make_beta_biome() -> Biome {
    Biome {
        temperature: 0.5,
        downfall: 0.5,
        has_precipitation: true,
        temperature_modifier: None,
        effects: mcrs_minecraft_world::biome::BiomeEffects {
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

/// Load the named density function JSON assets the beta router references.

/// Verify that a Beta-router column produces non-default BiomePalette cells.
///
/// For a column at (0, 0) with sea level = 64:
/// - Cells in sections below sea level (section y <= 3) must have ocean biome ids.
/// - Cells in sections above sea level (section y >= 5) must have land biome ids.
/// - Ocean and land ids for the same XZ position must differ.
/// - Modern path (no biome_context) must produce all-zero default palettes.
#[test]
fn generate_column_beta_biome_not_default() {
    let router = build_beta_router();
    let sea_level = router.sea_level();
    assert_eq!(sea_level, 64);

    // Build 16 unique biome assets — 11 land + 5 ocean.
    let mut assets = Assets::<Biome>::default();

    let land_handles: Vec<_> = (0..11).map(|_| assets.add(make_beta_biome())).collect();
    let ocean_handles: Vec<_> = (0..5).map(|_| assets.add(make_beta_biome())).collect();

    let land_ids: Vec<_> = land_handles.iter().map(|h| h.id()).collect();
    let ocean_ids: Vec<_> = ocean_handles.iter().map(|h| h.id()).collect();

    // Build a RegistrySnapshot with unique ids per biome (dense, starting at 0).
    // Names chosen so land_biome_* sort before ocean_biome_* → land = 0..10, ocean = 11..15.
    let all_pairs: Vec<(ResourceLocation<Arc<str>>, _)> = (0..11)
        .map(|i| {
            let rl = ResourceLocation::parse(&format!("minecraft:land_biome_{i}")).unwrap();
            (rl, land_ids[i])
        })
        .chain((0..5).map(|i| {
            let rl = ResourceLocation::parse(&format!("minecraft:ocean_biome_{i}")).unwrap();
            (rl, ocean_ids[i])
        }))
        .collect();

    let snapshot = RegistrySnapshot::<Biome>::build(all_pairs, &assets, |_| {
        Ok(mcrs_minecraft_nbt::compound::NbtCompound::new())
    });
    assert_eq!(
        snapshot.len(),
        16,
        "RegistrySnapshot must contain all 16 biomes"
    );

    let land_biome_ids: [ResourceLocation<Arc<str>>; 11] = std::array::from_fn(|i| {
        ResourceLocation::parse(&format!("minecraft:land_biome_{i}")).unwrap()
    });
    let ocean_biome_ids: [ResourceLocation<Arc<str>>; 5] = std::array::from_fn(|i| {
        ResourceLocation::parse(&format!("minecraft:ocean_biome_{i}")).unwrap()
    });
    let biome_source = BiomeSource::Beta {
        land_biomes: land_handles.try_into().expect("11 land handles"),
        ocean_biomes: ocean_handles.try_into().expect("5 ocean handles"),
        land_biome_ids,
        ocean_biome_ids,
        lookup: Box::new(build_beta_lookup_table()),
    };

    let (temp_0, hum_0) =
        router.sample_beta_climate(&mut mcrs_minecraft_worldgen::program::Workspace::new(), 0, 0);

    // Ocean biome id for (temp_0, hum_0) at below-sea-level cell.
    let ocean_asset_id = biome_source.beta_biome_id(temp_0, hum_0, true);
    let ocean_net_id = snapshot.by_asset_id(ocean_asset_id).unwrap() as u8;
    // land_biome_* sort before ocean_biome_*, so ocean ids ≥ 11.
    assert!(
        ocean_net_id >= 11,
        "ocean biome network id {} must be ≥ 11 (ocean names sort after land names)",
        ocean_net_id
    );

    // Land biome id for (temp_0, hum_0) at above-sea-level cell.
    let land_asset_id = biome_source.beta_biome_id(temp_0, hum_0, false);
    let land_net_id = snapshot.by_asset_id(land_asset_id).unwrap() as u8;
    assert!(
        land_net_id <= 10,
        "land biome network id {} must be ≤ 10 (land names sort before ocean names)",
        land_net_id
    );

    // Production path resolves by resource location (stable across AssetServers).
    // It must agree with the asset-id lookup in this single-AssetServer test.
    let land_loc = biome_source.beta_biome_location(temp_0, hum_0, false);
    let land_net_id_by_loc = snapshot.by_location(land_loc.as_str()).unwrap() as u8;
    assert_eq!(
        land_net_id_by_loc, land_net_id,
        "location-based biome resolution must match asset-based resolution"
    );

    // Ocean and land must be different ids for the same XZ position.
    assert_ne!(
        ocean_net_id, land_net_id,
        "ocean and land biome ids must differ for same XZ position"
    );

    // Generate a column straddling sea level and verify the results are Some.
    let y_sections: Vec<i32> = (-3..=7).collect();
    let cancel = CancellationToken::new();

    let results = generate_column(
        0,
        0,
        &y_sections,
        &router,
        Some((&biome_source, &snapshot)),
        super::corpus(),
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
    let results_modern = generate_column(
        0,
        0,
        &[0, 1, 2, 3, 4, 5],
        &router,
        None,
        super::corpus(),
        &cancel,
    );
    for (idx, r) in results_modern.iter().enumerate() {
        let (_, biomes) = r.as_ref().expect("modern section must not be cancelled");
        let net = biomes.convert_network();
        // Default BiomePalette is Homogeneous(0) which serializes as Single(0).
        assert!(
            matches!(
                net.palette,
                mcrs_minecraft_protocol::chunk::Palette::Single(0)
            ),
            "modern path section y={} must produce default (all-zero) BiomePalette",
            idx
        );
    }
}
