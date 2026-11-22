use std::path::Path;
use std::sync::Arc;

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_game_registries::dimension::bake;
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_heightmap::heightmap_predicates;
use mcrs_minecraft_registries::{Diagnostics, RegistryAdditions};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_generator::SurfaceIds;
use mcrs_minecraft_worldgen_generator::ids::FillIds;
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::PresetBiomeTables;
use mcrs_minecraft_worldgen_generator::routers::build_router;
use mcrs_minecraft_worldgen_generator::stages::{
    FillContext, GeneratorSetup, dimension_y_sections,
};

use crate::{Depth, World};

fn pack_sources(
    assets: &Path,
    vanilla: &Path,
    pack: Option<&Path>,
) -> Result<(DirectoryPacks, RegistryAdditions), String> {
    let mut packs = DirectoryPacks::new(assets.to_owned(), vanilla.to_owned());
    let mut additions = RegistryAdditions {
        local_packs: vec!["mcrs".to_owned(), "beta".to_owned()],
        ..RegistryAdditions::default()
    };
    if let Some(dir) = pack {
        packs = packs.with_only_pack(dir).map_err(|e| e.to_string())?;
        let name = std::fs::canonicalize(dir)
            .ok()
            .and_then(|dir| Some(dir.file_name()?.to_str()?.to_owned()))
            .ok_or_else(|| format!("{}: not a pack directory", dir.display()))?;
        additions.local_packs.push(name);
    }
    Ok((packs, additions))
}

pub fn load(
    assets: &Path,
    vanilla: &Path,
    pack: Option<&Path>,
    seed: u64,
    depth: Depth,
) -> Result<World, String> {
    let (packs, mut additions) = pack_sources(assets, vanilla, pack)?;
    mcrs_minecraft_worldgen::registries::add_vanilla_worldgen(&mut additions)
        .map_err(|e| e.to_string())?;
    let (registries, diagnostics) =
        load_registries(&packs, additions).map_err(|e| e.to_string())?;
    for diagnostic in diagnostics.iter() {
        eprintln!("pack: {diagnostic}");
    }

    let presets = registries
        .registry::<WorldPreset>()
        .ok_or("the registries hold no world presets")?;
    let normal = ResourceLocationBuf::read("minecraft:normal").map_err(|e| e.to_string())?;
    let preset = presets
        .by_name(&normal)
        .and_then(|id| presets.get(id))
        .ok_or("no world preset minecraft:normal")?;
    let (_, overworld) = bake(&preset.dimensions, &registries)?
        .into_iter()
        .next()
        .ok_or("the preset has no dimension")?;
    let ChunkGenerator::Noise(generator) = overworld.generator else {
        return Err("the overworld generator is not a noise generator".into());
    };
    let settings: &NoiseGeneratorSettings = registries
        .registry::<NoiseGeneratorSettings>()
        .and_then(|settings| settings.get(generator.settings))
        .ok_or("the overworld names noise settings the registry does not hold")?;

    let blocks = mcrs_minecraft_worldgen_generator::tests::blocks();
    let (router, material) =
        build_router(&registries, settings, seed, &blocks.0).map_err(|e| e.to_string())?;
    let router = Arc::new(router);
    let y_sections = dimension_y_sections(&router, router.noise.min_y, router.noise.height / 16);

    let mut diagnostics = Diagnostics::default();
    let surface_ids = SurfaceIds::resolve(&registries, &mut diagnostics);
    let fill_ids = FillIds::resolve(&registries, &mut diagnostics);
    let preset_tables = PresetBiomeTables::resolve(&registries, &mut diagnostics);
    let (Some(surface_ids), Some(fill_ids), Some(preset_tables)) =
        (surface_ids, fill_ids, preset_tables)
    else {
        return Err(format!(
            "the registries lack a name worldgen needs: {diagnostics}"
        ));
    };
    let source = Arc::new(generator.biome_source.clone());
    let predicates = heightmap_predicates(blocks, &fill_ids.block_tags);
    let setup = GeneratorSetup::Noise {
        router: Arc::clone(&router),
        material: (depth != Depth::Shape).then(|| Arc::new(material)),
    };
    let ctx = FillContext::build(
        setup,
        blocks.0.clone(),
        y_sections,
        Some(source),
        &preset_tables,
        predicates,
        None,
        Arc::new(surface_ids),
        &fill_ids,
        None,
        None,
    );
    let biome_names = registries
        .registry::<BiomeFile>()
        .ok_or("the registries hold no biomes")?
        .iter()
        .map(|(_, name, _)| name.to_string())
        .collect();
    Ok(World {
        ctx,
        biome_names,
        sea_level: router.sea_level,
    })
}
