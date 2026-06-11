use mcrs_minecraft::world::chunk::CancellationToken;
use mcrs_minecraft::world::generate::generate_column;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_protocol::Ident;
use std::collections::BTreeMap;

fn load_noise_settings(name: &str) -> NoiseGeneratorSettings {
    let path = format!(
        "{}/../../assets/minecraft/worldgen/noise_settings/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("noise_settings/{}.json must exist", name));
    serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("noise_settings/{}.json must deserialize: {}", name, e))
}

fn load_beta_density_functions() -> BTreeMap<Ident<String>, ProtoDensityFunction> {
    let dir = format!(
        "{}/../../assets/minecraft/worldgen/density_function/beta",
        env!("CARGO_MANIFEST_DIR"),
    );
    let mut map = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("density_function/beta must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let json = std::fs::read_to_string(&path).expect("density function must be readable");
        let DensityFunctionHolder::Owned(pdf) = serde_json::from_str::<DensityFunctionHolder>(
            &json,
        )
        .unwrap_or_else(|e| panic!("{} must deserialize: {}", path.display(), e)) else {
            panic!("{} must be an owned density function", path.display());
        };
        let stem = path.file_stem().unwrap().to_string_lossy();
        let ident = format!("minecraft:beta/{}", stem)
            .parse::<Ident<String>>()
            .expect("valid ident");
        map.insert(ident, *pdf);
    }
    map
}

/// Under the beta noise settings (min_y=0, height=128), `generate_column` must
/// return all-air for every section that falls entirely outside [0, 128).
/// The beta noise_router references the `minecraft:beta/*` density functions;
/// their `mcrs:beta/*` legacy noises are id-keyed and built internally, so no
/// noise assets are needed.  Sections 0-7 cover Y 0..128; section 8+ (Y>=128)
/// and any negative sections must be all air — no stone above Y 128.
#[test]
fn beta_sections_outside_noise_range_are_air() {
    let settings = load_noise_settings("beta");
    assert_eq!(settings.noise.min_y, 0, "beta min_y must be 0");
    assert_eq!(settings.noise.height, 128, "beta height must be 128");

    let functions = load_beta_density_functions();
    let noises = BTreeMap::new();
    let router = build_functions(&functions, &noises, &settings, 42, mcrs_protocol::BlockStateId(1), mcrs_protocol::BlockStateId(86));

    assert_eq!(router.noise_min_y(), 0);
    assert_eq!(router.noise_height(), 128);

    // Request the full overworld section range a 1.19+ client sends (-4..=19)
    let y_sections: Vec<i32> = (-4..=19).collect();
    let cancel = CancellationToken::new();

    let results = generate_column(0, 0, &y_sections, &router, None, &cancel);
    assert_eq!(results.len(), y_sections.len());

    for (&sy, result) in y_sections.iter().zip(results.iter()) {
        let section_min_y = sy * 16;
        let section_max_y = section_min_y + 16;
        let inside_range = section_min_y < 128 && section_max_y > 0;

        let (blocks, _biomes) = result.as_ref().expect("section must not be cancelled");

        if !inside_range {
            assert_eq!(
                blocks.non_air_block_count(),
                0,
                "section sy={} (Y {}..{}) is outside beta noise range [0,128) but contains {} non-air blocks",
                sy, section_min_y, section_max_y,
                blocks.non_air_block_count(),
            );
        }
    }
}

/// The modern overworld noise settings (min_y=-64, height=384) span [-64, 320).
/// The full client section range [-4..=19] sits entirely inside this band, so
/// the noise-range clamp would be a no-op — no section is spuriously clamped
/// to air on the modern path.  This test verifies the math without calling
/// `generate_column` (the overworld density functions require disk assets).
#[test]
fn modern_overworld_noise_range_covers_all_client_sections() {
    let settings = load_noise_settings("overworld");
    assert_eq!(settings.noise.min_y, -64, "overworld min_y must be -64");
    assert_eq!(settings.noise.height, 384, "overworld height must be 384");

    let noise_min_y = settings.noise.min_y;
    let noise_max_y = noise_min_y + settings.noise.height as i32;

    // Every section in the standard client range must be inside the noise band.
    for sy in -4..=19i32 {
        let section_min_y = sy * 16;
        let section_max_y = section_min_y + 16;
        assert!(
            section_min_y < noise_max_y && section_max_y > noise_min_y,
            "section sy={} (Y {}..{}) should be inside modern noise range [{}, {})",
            sy, section_min_y, section_max_y, noise_min_y, noise_max_y,
        );
    }
}
