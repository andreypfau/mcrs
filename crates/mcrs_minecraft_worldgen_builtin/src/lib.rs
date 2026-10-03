mod beta;
mod density;
mod noises;
mod settings;
mod terrain;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::proto::build::Functions;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::Serialize;
use std::collections::BTreeMap;

pub fn noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    let mut noises = noises::noises();
    noises.extend(beta::noises());
    noises
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

/// Every built-in entry of one `worldgen` folder as the JSON it would ship as.
/// A folder with no built-ins is empty.
pub fn assets(folder: &str) -> BTreeMap<ResourceLocation, Vec<u8>> {
    fn encode<T: Serialize>(
        registry: BTreeMap<ResourceLocation, T>,
    ) -> BTreeMap<ResourceLocation, Vec<u8>> {
        registry
            .into_iter()
            .map(|(id, value)| {
                let json = serde_json::to_vec(&value).expect("a built-in encodes as JSON");
                (id, json)
            })
            .collect()
    }

    match folder {
        "density_function" => encode(density_functions()),
        "noise_settings" => encode(noise_settings()),
        "noise" => encode(noises()),
        _ => BTreeMap::new(),
    }
}

/// One built-in entry, addressed the way the asset server addresses the file:
/// `<namespace>/worldgen/<folder>/<path>.json`.
// chisle: builds the whole folder to answer for one entry, which is paid once
// per entry at load. A per-entry builder lifts it if a pack ever loads thousands.
pub fn asset(path: &str) -> Option<Vec<u8>> {
    let (namespace, rest) = path.split_once("/worldgen/")?;
    let (folder, name) = rest.strip_suffix(".json")?.split_once('/')?;
    let id = ResourceLocation::parse(&format!("{namespace}:{name}")).ok()?;
    assets(folder).remove(&id)
}
