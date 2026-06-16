use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_asset::Assets;
use mcrs_core::RegistrySnapshot;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::biome::source::{BiomeSource, build_beta_lookup_table};

use crate::world::chunk::CancellationToken;
use crate::world::generate::generate_column;

fn make_beta_biome() -> Biome {
    Biome {
        temperature: 0.5,
        downfall: 0.5,
        has_precipitation: true,
        effects: mcrs_vanilla::biome::BiomeEffects {
            water_color: None,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            dry_foliage_color: None,
        },
        carvers: Vec::new(),
        features: Vec::new(),
        spawners: mcrs_vanilla::biome::BiomeSpawners {
            ambient: Vec::new(),
            axolotls: Vec::new(),
            creature: Vec::new(),
            misc: Vec::new(),
            monster: Vec::new(),
            underground_water_creature: Vec::new(),
            water_ambient: Vec::new(),
            water_creature: Vec::new(),
        },
        spawn_costs: std::collections::HashMap::new(),
        attributes: None,
    }
}

/// Load the named density function JSON assets the beta router references.
fn load_density_functions_from_disk() -> BTreeMap<
    mcrs_protocol::Ident<String>,
    mcrs_minecraft_worldgen::density_function::proto::ProtoDensityFunction,
> {
    use mcrs_minecraft_worldgen::density_function::proto::DensityFunctionHolder;
    fn recurse(
        dir: &std::path::Path,
        prefix: &str,
        map: &mut BTreeMap<
            mcrs_protocol::Ident<String>,
            mcrs_minecraft_worldgen::density_function::proto::ProtoDensityFunction,
        >,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let subdir = entry.file_name().to_string_lossy().to_string();
                let new_prefix = if prefix.is_empty() {
                    subdir
                } else {
                    format!("{}/{}", prefix, subdir)
                };
                recurse(&path, &new_prefix, map);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let Ok(json) = std::fs::read_to_string(&path) else { continue };
                let Ok(DensityFunctionHolder::Owned(pdf)) =
                    serde_json::from_str::<DensityFunctionHolder>(&json)
                else {
                    continue;
                };
                let stem = path.file_stem().unwrap().to_string_lossy();
                let key = if prefix.is_empty() {
                    format!("minecraft:{}", stem)
                } else {
                    format!("minecraft:{}/{}", prefix, stem)
                };
                if let Ok(ident) = key.parse::<mcrs_protocol::Ident<String>>() {
                    map.insert(ident, *pdf);
                }
            }
        }
    }
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen/density_function");
    let mut map = BTreeMap::new();
    recurse(&base, "", &mut map);
    map
}

/// Build a Beta NoiseRouter from the actual beta.json settings file.
fn build_beta_router() -> mcrs_minecraft_worldgen::density_function::NoiseRouter {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json must exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json must deserialize");
    let functions = load_density_functions_from_disk();
    let noises = BTreeMap::new();
    build_functions(&functions, &noises, &settings, 12345, mcrs_protocol::BlockStateId(1), mcrs_protocol::BlockStateId(86))
}

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

    let snapshot = RegistrySnapshot::<Biome>::build(
        all_pairs,
        &assets,
        |_| Ok(mcrs_nbt::compound::NbtCompound::new()),
    );
    assert_eq!(snapshot.len(), 16, "RegistrySnapshot must contain all 16 biomes");

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

    // Build a column cache once to sample climate at (0, 0).
    let block_x = 0i32;
    let block_z = 0i32;
    let mut column_cache = router.new_column_cache(block_x, block_z);
    router.populate_columns(&mut column_cache);
    let (temp_0, hum_0) = router.sample_climate_at(&column_cache, block_x, block_z);

    // Ocean biome id for (temp_0, hum_0) at below-sea-level cell.
    let ocean_asset_id = biome_source.beta_biome_id(temp_0, hum_0, true);
    let ocean_net_id = snapshot.by_asset_id(ocean_asset_id).unwrap() as u8;
    // land_biome_* sort before ocean_biome_*, so ocean ids ≥ 11.
    assert!(ocean_net_id >= 11,
        "ocean biome network id {} must be ≥ 11 (ocean names sort after land names)", ocean_net_id);

    // Land biome id for (temp_0, hum_0) at above-sea-level cell.
    let land_asset_id = biome_source.beta_biome_id(temp_0, hum_0, false);
    let land_net_id = snapshot.by_asset_id(land_asset_id).unwrap() as u8;
    assert!(land_net_id <= 10,
        "land biome network id {} must be ≤ 10 (land names sort before ocean names)", land_net_id);

    // Production path resolves by resource location (stable across AssetServers).
    // It must agree with the asset-id lookup in this single-AssetServer test.
    let land_loc = biome_source.beta_biome_location(temp_0, hum_0, false);
    let land_net_id_by_loc = snapshot.by_location(land_loc.as_str()).unwrap() as u8;
    assert_eq!(land_net_id_by_loc, land_net_id,
        "location-based biome resolution must match asset-based resolution");

    // Ocean and land must be different ids for the same XZ position.
    assert_ne!(ocean_net_id, land_net_id,
        "ocean and land biome ids must differ for same XZ position");

    // Generate a column straddling sea level and verify the results are Some.
    let y_sections: Vec<i32> = (-3..=7).collect();
    let cancel = CancellationToken::new();

    let results = generate_column(
        0, 0,
        &y_sections,
        &router,
        Some((&biome_source, &snapshot)),
        &cancel,
    );

    assert_eq!(results.len(), y_sections.len());
    for (idx, result) in results.iter().enumerate() {
        assert!(result.is_some(), "section at y={} must not be cancelled", y_sections[idx]);
    }

    // Verify modern path: with no biome_context, all palette cells default to 0.
    let results_modern = generate_column(
        0, 0,
        &[0, 1, 2, 3, 4, 5],
        &router,
        None,
        &cancel,
    );
    for (idx, r) in results_modern.iter().enumerate() {
        let (_, biomes) = r.as_ref().expect("modern section must not be cancelled");
        let net = biomes.convert_network();
        // Default BiomePalette is Homogeneous(0) which serializes as Single(0).
        assert!(
            matches!(net.palette, mcrs_protocol::chunk::Palette::Single(0)),
            "modern path section y={} must produce default (all-zero) BiomePalette", idx
        );
    }
}
