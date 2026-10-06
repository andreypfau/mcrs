// chisle: every folder but the biomes is built as a typed value, encoded to
// JSON and parsed back by the loader, and its references are
// `ResourceLocation<Arc<str>>`, one allocation each. A loader that takes typed
// values and a `Holder` that is `Reference(Id<T>)` lift both.
mod beta;
mod biome;
mod density;
mod noises;
mod settings;
mod structure;
mod template_pool;
mod terrain;

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec;
use mcrs_minecraft_registry::{Built, RegistrySet};
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::proto::build::Functions;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::template::Template;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_structure::TemplatePool;
use serde::Serialize;
use std::collections::BTreeMap;

pub fn noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    let mut noises = noises::noises();
    noises.extend(beta::noises());
    noises
}

pub fn built_biomes() -> Built {
    biome::built()
}

pub fn biomes(
    set: &RegistrySet,
) -> Result<BTreeMap<ResourceLocation, BiomeFile>, Vec<(usize, String)>> {
    Ok(biome::names().into_iter().zip(biome::build(set)?).collect())
}

pub fn density_functions() -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    let mut functions = Functions::default();
    density::define(&mut functions);
    beta::define(&mut functions);
    functions.0
}

pub fn noise_settings() -> BTreeMap<ResourceLocation, NoiseGeneratorSettings> {
    let mut settings = settings::noise_settings();
    settings.extend([beta::noise_settings()]);
    settings
}

pub fn template_pools() -> BTreeMap<ResourceLocation, TemplatePool> {
    template_pool::all().collect()
}

pub fn templates() -> BTreeMap<ResourceLocation, Template> {
    structure::keys()
        .map(|id| {
            let template = structure::build(&id).expect("a listed template builds");
            (id, template)
        })
        .collect()
}

fn json<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("a built-in encodes as JSON")
}

fn encode<T: Serialize>(
    registry: BTreeMap<ResourceLocation, T>,
) -> BTreeMap<ResourceLocation, Vec<u8>> {
    registry
        .into_iter()
        .map(|(id, value)| (id, json(&value)))
        .collect()
}

/// Every built-in entry of one `worldgen` folder as the JSON it would ship as.
/// A folder with no built-ins is empty.
pub fn assets(folder: &str) -> BTreeMap<ResourceLocation, Vec<u8>> {
    match folder {
        "density_function" => encode(density_functions()),
        "noise_settings" => encode(noise_settings()),
        "noise" => encode(noises()),
        "template_pool" => encode(template_pools()),
        _ => BTreeMap::new(),
    }
}

/// The asset path of every built-in entry of one registry directory:
/// `<namespace>/worldgen/<folder>`, or `<namespace>/structure` for templates.
pub fn paths(directory: &str) -> Vec<String> {
    if let Some(namespace) = directory.strip_suffix("/structure") {
        return structure::keys()
            .filter(|id| id.namespace() == namespace)
            .map(|id| format!("{directory}/{}.nbt", id.path()))
            .collect();
    }
    let Some((namespace, folder)) = directory.split_once("/worldgen/") else {
        return Vec::new();
    };
    let ids: Vec<ResourceLocation> = match folder {
        "template_pool" => template_pool::keys().collect(),
        _ => assets(folder).into_keys().collect(),
    };
    ids.into_iter()
        .filter(|id| id.namespace() == namespace)
        .map(|id| format!("{directory}/{}.json", id.path()))
        .collect()
}

fn template(path: &str) -> Option<Vec<u8>> {
    let (namespace, name) = path.strip_suffix(".nbt")?.split_once("/structure/")?;
    let template = structure::build(&ResourceLocation::new(namespace, name).ok()?)?;
    Some(to_gzip_bytes_vec(&template).expect("a built-in template encodes as NBT"))
}

/// One built-in entry, addressed the way the asset server addresses the file:
/// `<namespace>/worldgen/<folder>/<path>.json`, or
/// `<namespace>/structure/<path>.nbt` for a template.
// chisle: the density, noise and noise settings folders build whole to answer
// for one entry or to list their ids, because their definitions share
// sub-expressions: 65 density functions served one by one take about 50 ms in
// release. Loading each registry once lifts it.
pub fn asset(path: &str) -> Option<Vec<u8>> {
    if path.ends_with(".nbt") {
        return template(path);
    }
    let (namespace, rest) = path.split_once("/worldgen/")?;
    let (folder, name) = rest.strip_suffix(".json")?.split_once('/')?;
    let id = ResourceLocation::new(namespace, name).ok()?;
    match folder {
        "template_pool" => template_pool::build(&id).map(|pool| json(&pool)),
        _ => assets(folder).remove(&id),
    }
}
