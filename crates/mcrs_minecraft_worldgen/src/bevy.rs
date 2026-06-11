use crate::density_function::proto::{
    DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction, Visitor,
};
use crate::density_function::{NoiseRouter, build_functions};
use crate::proto::{Either, NoiseGeneratorSettings};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetEvent, AssetLoader, AssetServer, Assets, Handle, LoadContext,
    LoadDirectError,
};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Res, Resource};
use bevy_reflect::TypePath;
use mcrs_protocol::Ident;
use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

/// Configures which world preset to load and the world seed to use for generation.
///
/// Insert this resource before `Startup` so that the noise-router build can
/// read it. Defaults to the `normal` preset (overworld noise settings) and
/// seed 0.  Override by setting `MCRS_WORLD_PRESET` and `MCRS_WORLD_SEED`
/// environment variables.
#[derive(Resource, Clone, Debug)]
pub struct WorldGenConfig {
    /// The `namespace:path` identifier of the active world preset.
    pub preset_namespace: Arc<str>,
    pub preset_path: Arc<str>,
    /// The noise-settings id for the overworld dimension of this preset,
    /// derived from `generator.settings` in the preset JSON.
    /// For `minecraft:normal` this is `minecraft:overworld`; for
    /// `minecraft:beta` this is `minecraft:beta`.
    pub noise_settings_namespace: Arc<str>,
    pub noise_settings_path: Arc<str>,
    /// World seed forwarded to `build_functions`.
    pub seed: u64,
    /// Registry-resolved default block state ID (stone) for `build_functions`.
    /// Populated by the mcrs_minecraft layer using minecraft::STONE.default_state_id.
    pub default_block_state_id: mcrs_protocol::BlockStateId,
    /// Registry-resolved default fluid state ID (water level 0) for `build_functions`.
    /// Populated by the mcrs_minecraft layer using minecraft::WATER.default_state_id.
    pub default_fluid_state_id: mcrs_protocol::BlockStateId,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            preset_namespace: Arc::from("minecraft"),
            preset_path: Arc::from("normal"),
            noise_settings_namespace: Arc::from("minecraft"),
            noise_settings_path: Arc::from("overworld"),
            seed: 0,
            default_block_state_id: mcrs_protocol::BlockStateId(1),
            default_fluid_state_id: mcrs_protocol::BlockStateId(86),
        }
    }
}

impl WorldGenConfig {
    /// Build from environment variables:
    /// - `MCRS_WORLD_PRESET`: `"normal"` or `"minecraft:normal"` (default: `"minecraft:normal"`)
    /// - `MCRS_WORLD_SEED`: decimal `u64` (default: `0`)
    pub fn from_env() -> Self {
        let (preset_namespace, preset_path) = match env::var("MCRS_WORLD_PRESET") {
            Ok(raw) => {
                let trimmed = raw.trim().to_lowercase();
                if let Some(colon) = trimmed.find(':') {
                    (
                        Arc::from(&trimmed[..colon]),
                        Arc::from(&trimmed[colon + 1..]),
                    )
                } else if !trimmed.is_empty() {
                    (Arc::from("minecraft"), Arc::from(trimmed.as_str()))
                } else {
                    (Arc::from("minecraft"), Arc::from("normal"))
                }
            }
            Err(_) => (Arc::from("minecraft"), Arc::from("normal")),
        };

        let seed = match env::var("MCRS_WORLD_SEED") {
            Ok(raw) => raw.trim().parse::<u64>().unwrap_or(0),
            Err(_) => 0,
        };

        let (noise_settings_namespace, noise_settings_path) =
            resolve_overworld_noise_settings(&preset_namespace, &preset_path);

        Self {
            preset_namespace,
            preset_path,
            noise_settings_namespace,
            noise_settings_path,
            seed,
            default_block_state_id: mcrs_protocol::BlockStateId(1),
            default_fluid_state_id: mcrs_protocol::BlockStateId(86),
        }
    }

    /// Returns the Bevy asset path for the active world preset JSON.
    ///
    /// Format: `{namespace}/worldgen/world_preset/{path}.json`
    pub fn preset_asset_path(&self) -> String {
        format!(
            "{}/worldgen/world_preset/{}.json",
            self.preset_namespace, self.preset_path
        )
    }

    /// Returns the Bevy asset path for the overworld noise settings JSON.
    ///
    /// Format: `{namespace}/worldgen/noise_settings/{path}.json`
    pub fn noise_settings_asset_path(&self) -> String {
        format!(
            "{}/worldgen/noise_settings/{}.json",
            self.noise_settings_namespace, self.noise_settings_path
        )
    }
}

/// Read the world preset JSON from disk and extract the `generator.settings`
/// id for the `minecraft:overworld` dimension.  Falls back to using the preset
/// namespace/path unchanged when the file cannot be read or parsed.
pub(crate) fn resolve_overworld_noise_settings(
    preset_ns: &str,
    preset_path: &str,
) -> (Arc<str>, Arc<str>) {
    let asset_root = env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
    let json_path = format!(
        "{}/assets/{}/worldgen/world_preset/{}.json",
        asset_root, preset_ns, preset_path
    );

    let fallback = || (Arc::from(preset_ns), Arc::from(preset_path));

    let data = match std::fs::read_to_string(&json_path) {
        Ok(d) => d,
        Err(_) => return fallback(),
    };

    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return fallback(),
    };

    let settings_str = json
        .get("dimensions")
        .and_then(|d| d.get("minecraft:overworld"))
        .and_then(|ow| ow.get("generator"))
        .and_then(|g| g.get("settings"))
        .and_then(|s| s.as_str());

    match settings_str {
        Some(s) => {
            if let Some(colon) = s.find(':') {
                (Arc::from(&s[..colon]), Arc::from(&s[colon + 1..]))
            } else {
                (Arc::from("minecraft"), Arc::from(s))
            }
        }
        None => fallback(),
    }
}

pub struct NoiseGeneratorSettingsPlugin;

impl Plugin for NoiseGeneratorSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DensityFunctionAsset>()
            .init_asset::<NoiseGeneratorSettingsAsset>()
            .init_asset::<NoiseParamAsset>()
            .register_asset_loader(DensityFunctionLoader)
            .register_asset_loader(NoiseGeneratorSettingsLoader)
            .register_asset_loader(NoiseParamLoader)
            .add_systems(Startup, request_overworld_noise_settings)
            .add_systems(Update, build_noise_router_on_load);
    }
}

/// Retains the `Handle<NoiseGeneratorSettingsAsset>` so the asset is not
/// dropped before `build_noise_router_on_load` can react to its load event.
#[derive(Resource)]
pub struct NoiseSettingsHandle(pub Handle<NoiseGeneratorSettingsAsset>);

#[derive(Resource)]
pub struct OverworldNoiseRouter(pub Arc<NoiseRouter>);

fn request_overworld_noise_settings(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    world_gen_config: Option<Res<WorldGenConfig>>,
) {
    let asset_path = match &world_gen_config {
        Some(cfg) => cfg.noise_settings_asset_path(),
        None => "minecraft/worldgen/noise_settings/overworld.json".to_string(),
    };

    info!(asset_path = %asset_path, "Loading overworld noise settings");

    let handle: Handle<NoiseGeneratorSettingsAsset> = asset_server.load(asset_path);
    commands.insert_resource(NoiseSettingsHandle(handle));
}

fn build_noise_router_on_load(
    mut commands: Commands,
    mut messages: MessageReader<AssetEvent<NoiseGeneratorSettingsAsset>>,
    noise_settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
    density_functions: Res<Assets<DensityFunctionAsset>>,
    noises: Res<Assets<NoiseParamAsset>>,
    world_gen_config: Option<Res<WorldGenConfig>>,
    noise_handle: Option<Res<NoiseSettingsHandle>>,
) {
    messages.read().for_each(|event| match event {
        AssetEvent::LoadedWithDependencies { id } => {
            // Only react to the handle we explicitly requested, not to any
            // incidentally loaded NoiseGeneratorSettingsAsset.
            let expected_id = noise_handle.as_ref().map(|h| h.0.id());
            if expected_id.is_some_and(|eid| eid != *id) {
                return;
            }

            if let Some(settings) = noise_settings.get(*id) {
                let mut all_functions: BTreeMap<_, _> = settings
                    .density_functions
                    .iter()
                    .map(|(id, handle)| {
                        density_functions
                            .get(handle)
                            .map(|func_asset| (id.clone(), func_asset.clone()))
                    })
                    .flatten()
                    .collect();

                let mut all_noises = settings
                    .noises
                    .iter()
                    .map(|(id, handle)| {
                        noises
                            .get(handle)
                            .map(|noise_asset| (id.clone(), noise_asset.clone()))
                    })
                    .flatten()
                    .collect::<BTreeMap<_, _>>();

                fn load_functions_recursively(
                    func_asset: &DensityFunctionAsset,
                    density_functions: &Res<Assets<DensityFunctionAsset>>,
                    noises: &Res<Assets<NoiseParamAsset>>,
                    all_functions: &mut BTreeMap<Ident<String>, DensityFunctionAsset>,
                    all_noises: &mut BTreeMap<Ident<String>, NoiseParamAsset>,
                ) {
                    for (dep_id, dep_handle) in &func_asset.deps {
                        if !all_functions.contains_key(dep_id) {
                            if let Some(dep_asset) = density_functions.get(dep_handle) {
                                all_functions.insert(dep_id.clone(), dep_asset.clone());
                                load_functions_recursively(
                                    dep_asset,
                                    density_functions,
                                    noises,
                                    all_functions,
                                    all_noises,
                                );
                            }
                        }
                    }
                    func_asset.noise_deps.iter().for_each(|(id, handle)| {
                        if !all_noises.contains_key(id) {
                            if let Some(noise_asset) = noises.get(handle) {
                                all_noises.insert(id.clone(), noise_asset.clone());
                            }
                        }
                    })
                }

                all_functions.clone().values().for_each(|func_asset| {
                    load_functions_recursively(
                        func_asset,
                        &density_functions,
                        &noises,
                        &mut all_functions,
                        &mut all_noises,
                    );
                });

                let mut functions_proto = BTreeMap::new();

                fn register_function(
                    id: &Ident<String>,
                    func_asset: &DensityFunctionAsset,
                    all_functions: &BTreeMap<Ident<String>, DensityFunctionAsset>,
                    functions_proto: &mut BTreeMap<Ident<String>, ProtoDensityFunction>,
                ) {
                    match &func_asset.function {
                        DensityFunctionHolder::Value(x) => {
                            functions_proto
                                .insert(id.clone(), ProtoDensityFunction::Constant(x.clone()));
                        }
                        DensityFunctionHolder::Reference(r) => {
                            all_functions.get(r).map(|dep_asset| {
                                register_function(id, dep_asset, all_functions, functions_proto);
                            });
                        }
                        DensityFunctionHolder::Owned(x) => {
                            functions_proto.insert(id.clone(), *x.clone());
                        }
                    }
                }

                all_functions.iter().for_each(|(id, func_asset)| {
                    register_function(id, func_asset, &all_functions, &mut functions_proto);
                });

                let mut noises_proto = BTreeMap::new();
                all_noises.iter().for_each(|(id, handle)| {
                    noises_proto.insert(id.clone(), handle.noise.clone());
                });

                let seed = world_gen_config.as_ref().map(|c| c.seed).unwrap_or(0);
                let noise_settings_id = world_gen_config
                    .as_ref()
                    .map(|c| {
                        format!(
                            "{}:{}",
                            c.noise_settings_namespace, c.noise_settings_path
                        )
                    })
                    .unwrap_or_else(|| "minecraft:overworld".to_string());
                info!(
                    noise_settings = %noise_settings_id,
                    seed = seed,
                    "Building OverworldNoiseRouter"
                );
                let (default_block, default_fluid) = world_gen_config
                    .as_ref()
                    .map(|c| (c.default_block_state_id, c.default_fluid_state_id))
                    .unwrap_or((
                        mcrs_protocol::BlockStateId(1),
                        mcrs_protocol::BlockStateId(86),
                    ));
                let overworld = OverworldNoiseRouter(Arc::new(build_functions(
                    &functions_proto,
                    &noises_proto,
                    &settings.settings,
                    seed,
                    default_block,
                    default_fluid,
                )));
                commands.insert_resource(overworld);
            }
        }
        _ => {}
    });
}

#[derive(TypePath, Debug)]
pub struct NoiseGeneratorSettingsAsset {
    pub settings: NoiseGeneratorSettings,
    pub density_functions: BTreeMap<Ident<String>, Handle<DensityFunctionAsset>>,
    pub noises: BTreeMap<Ident<String>, Handle<NoiseParamAsset>>,
}

impl bevy_asset::Asset for NoiseGeneratorSettingsAsset {}
impl bevy_asset::VisitAssetDependencies for NoiseGeneratorSettingsAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.density_functions.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(TypePath, Debug, Clone)]
pub struct DensityFunctionAsset {
    pub function: DensityFunctionHolder,
    pub deps: BTreeMap<Ident<String>, Handle<DensityFunctionAsset>>,
    pub noise_deps: BTreeMap<Ident<String>, Handle<NoiseParamAsset>>,
}

#[derive(TypePath, Asset, Debug, Clone)]
pub struct NoiseParamAsset {
    pub noise: NoiseParam,
}

impl bevy_asset::Asset for DensityFunctionAsset {}
impl bevy_asset::VisitAssetDependencies for DensityFunctionAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.deps.values() {
            visit(handle.id().untyped());
        }
        for handle in self.noise_deps.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(Default, TypePath)]
pub struct NoiseGeneratorSettingsLoader;

#[derive(Debug, Error)]
pub enum NoiseGeneratorSettingsLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

impl AssetLoader for NoiseGeneratorSettingsLoader {
    type Asset = NoiseGeneratorSettingsAsset;
    type Settings = ();
    type Error = NoiseGeneratorSettingsLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let noise = serde_json::de::from_slice::<NoiseGeneratorSettings>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut visitor = DensityFunctionVisitor {
            load_context,
            density_functions: BTreeMap::new(),
            noises: BTreeMap::new(),
        };
        visitor.visit_density_function_holder(&noise.noise_router.barrier);
        visitor.visit_density_function_holder(&noise.noise_router.fluid_level_floodedness);
        visitor.visit_density_function_holder(&noise.noise_router.fluid_level_spread);
        visitor.visit_density_function_holder(&noise.noise_router.lava);
        visitor.visit_density_function_holder(&noise.noise_router.temperature);
        visitor.visit_density_function_holder(&noise.noise_router.vegetation);
        visitor.visit_density_function_holder(&noise.noise_router.continents);
        visitor.visit_density_function_holder(&noise.noise_router.erosion);
        visitor.visit_density_function_holder(&noise.noise_router.depth);
        visitor.visit_density_function_holder(&noise.noise_router.ridges);
        visitor.visit_density_function_holder(&noise.noise_router.preliminary_surface_level);
        visitor.visit_density_function_holder(&noise.noise_router.final_density);
        visitor.visit_density_function_holder(&noise.noise_router.vein_toggle);
        visitor.visit_density_function_holder(&noise.noise_router.vein_ridged);
        visitor.visit_density_function_holder(&noise.noise_router.vein_gap);

        Ok(NoiseGeneratorSettingsAsset {
            settings: noise,
            density_functions: visitor.density_functions,
            noises: visitor.noises,
        })
    }
}

#[derive(Default, TypePath)]
pub struct NoiseParamLoader;

#[derive(Debug, Error)]
pub enum NoiseParamLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

impl AssetLoader for NoiseParamLoader {
    type Asset = NoiseParamAsset;
    type Settings = ();
    type Error = NoiseParamLoaderError;

    async fn load<'a>(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'a>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let noise = serde_json::de::from_slice::<NoiseParam>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(NoiseParamAsset { noise })
    }
}

#[derive(Default, TypePath)]
pub struct DensityFunctionLoader;

#[derive(Debug, Error)]
pub enum DensityFunctionLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

impl AssetLoader for DensityFunctionLoader {
    type Asset = DensityFunctionAsset;
    type Settings = ();
    type Error = DensityFunctionLoaderError;

    async fn load<'a>(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'a>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let holder = serde_json::de::from_slice::<DensityFunctionHolder>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut collector = DensityFunctionVisitor {
            load_context,
            density_functions: BTreeMap::new(),
            noises: BTreeMap::new(),
        };
        collector.visit_density_function_holder(&holder);

        Ok(DensityFunctionAsset {
            function: holder,
            deps: collector.density_functions,
            noise_deps: collector.noises,
        })
    }
}

struct DensityFunctionVisitor<'a, 'b> {
    pub load_context: &'a mut LoadContext<'b>,
    pub density_functions: BTreeMap<Ident<String>, Handle<DensityFunctionAsset>>,
    pub noises: BTreeMap<Ident<String>, Handle<NoiseParamAsset>>,
}

impl<'a, 'b> Visitor for DensityFunctionVisitor<'a, 'b> {
    fn visit_reference(&mut self, value: &Ident<String>) {
        if self.density_functions.contains_key(value) {
            return;
        }
        let handle = self.load_context.load(format!(
            "{}/worldgen/density_function/{}.json",
            value.namespace(),
            value.path()
        ));
        self.density_functions.insert(value.clone(), handle);
    }

    fn visit_noise_holder(&mut self, noise: &NoiseHolder) {
        let NoiseHolder::Reference(r) = noise else {
            return;
        };
        if self.noises.contains_key(r) {
            return;
        }
        let handle = self.load_context.load(format!(
            "{}/worldgen/noise/{}.json",
            r.namespace(),
            r.path()
        ));
        self.noises.insert(r.clone(), handle);
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_overworld_noise_settings;

    /// Verify that the normal preset resolves its overworld noise settings to
    /// `minecraft:overworld` (not `minecraft:normal`), by reading the actual
    /// preset JSON from disk.
    #[test]
    fn noise_settings_for_normal_preset_is_overworld() {
        let (ns, path) = resolve_overworld_noise_settings("minecraft", "normal");
        assert_eq!(ns.as_ref(), "minecraft");
        assert_eq!(path.as_ref(), "overworld");
    }

    /// Verify that the beta preset resolves its overworld noise settings to
    /// `minecraft:beta`.
    #[test]
    fn noise_settings_for_beta_preset_is_beta() {
        let (ns, path) = resolve_overworld_noise_settings("minecraft", "beta");
        assert_eq!(ns.as_ref(), "minecraft");
        assert_eq!(path.as_ref(), "beta");
    }
}
