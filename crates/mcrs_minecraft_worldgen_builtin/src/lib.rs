#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
// chisle: every reference in a built template pool is a
// `ResourceLocation<Arc<str>>`, one allocation each, and a structure template is
// built as a typed value, encoded to NBT and parsed back by the asset loader. A
// `Holder` that is `Reference(Id<T>)` and a loader that takes typed templates
// lift both.
mod beta;
mod biome;
mod density;
mod noises;
mod settings;
mod structure;
mod template_pool;
mod terrain;

use mcrs_minecraft_biome_file::BiomeDraft;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec;
use mcrs_minecraft_noise::proto::NoiseParam;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::proto::build::Functions;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::template::Template;
use std::collections::BTreeMap;

pub fn vanilla_noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    noises::noises()
}

pub fn beta_noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    beta::noises().collect()
}

pub fn vanilla_density_functions() -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    let mut functions = Functions::default();
    density::define(&mut functions);
    functions.0
}

pub fn beta_density_functions() -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    let mut functions = Functions::default();
    beta::define(&mut functions);
    functions.0
}

pub fn vanilla_noise_settings() -> BTreeMap<ResourceLocation, NoiseGeneratorSettings> {
    settings::noise_settings()
}

pub fn beta_noise_settings() -> BTreeMap<ResourceLocation, NoiseGeneratorSettings> {
    BTreeMap::from([beta::noise_settings()])
}

pub fn noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    let mut noises = vanilla_noises();
    noises.extend(beta_noises());
    noises
}

pub fn biomes() -> BTreeMap<ResourceLocation, BiomeDraft> {
    biome::drafts().collect()
}

pub fn density_functions() -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
    let mut functions = vanilla_density_functions();
    functions.extend(beta_density_functions());
    functions
}

pub fn noise_settings() -> BTreeMap<ResourceLocation, NoiseGeneratorSettings> {
    let mut settings = vanilla_noise_settings();
    settings.extend(beta_noise_settings());
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

/// The path of every built-in structure template under one namespace's
/// `<namespace>/structure` directory.
pub fn template_paths(directory: &str) -> Vec<String> {
    let Some(namespace) = directory.strip_suffix("/structure") else {
        return Vec::new();
    };
    structure::keys()
        .filter(|id| id.namespace() == namespace)
        .map(|id| format!("{directory}/{}.nbt", id.path()))
        .collect()
}

/// One built-in structure template, addressed the way the asset server
/// addresses the file: `<namespace>/structure/<path>.nbt`. Registry entries are
/// built by the registry loader and never answered here.
pub fn asset(path: &str) -> Option<Vec<u8>> {
    let (namespace, name) = path.strip_suffix(".nbt")?.split_once("/structure/")?;
    let template = structure::build(&ResourceLocation::new(namespace, name).ok()?)?;
    Some(to_gzip_bytes_vec(&template).expect("a built-in template encodes as NBT"))
}
