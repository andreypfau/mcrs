use crate::compile::build_router;
use crate::proto::{DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction};
use crate::router::{GeneratorSettings, NoiseRouter};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetLoader, AssetServer, Assets, Handle, LoadContext, LoadDirectError,
    RecursiveDependencyLoadState, VisitAssetDependencies,
};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Local, Res, Resource};
use bevy_reflect::TypePath;
use mcrs_minecraft_core::asset::read_all;
use mcrs_minecraft_core::ResourceLocation;
use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

/// Which world preset to load and the seed to generate with.
///
/// Insert before `Startup` so the router build can read it. Defaults to the
/// `normal` preset and seed 0; `MCRS_WORLD_PRESET` and `MCRS_WORLD_SEED`
/// override both.
#[derive(Resource, Clone, Debug)]
pub struct WorldGenConfig {
    pub preset_namespace: Arc<str>,
    pub preset_path: Arc<str>,
    pub seed: u64,
    /// The terrain block and the sea fluid the active noise settings state,
    /// resolved against the block registry by whoever runs before
    /// [`BuildNoiseRouter`]. Unset until then, and the router refuses to build
    /// on a guess.
    pub default_block_state_id: Option<mcrs_voxel_storage::VoxelId>,
    pub default_fluid_state_id: Option<mcrs_voxel_storage::VoxelId>,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            preset_namespace: Arc::from("minecraft"),
            preset_path: Arc::from("normal"),
            seed: 0,
            default_block_state_id: None,
            default_fluid_state_id: None,
        }
    }
}

impl WorldGenConfig {
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

        Self {
            preset_namespace,
            preset_path,
            seed,
            default_block_state_id: None,
            default_fluid_state_id: None,
        }
    }

    pub fn preset_asset_path(&self) -> String {
        format!(
            "{}/worldgen/world_preset/{}.json",
            self.preset_namespace, self.preset_path
        )
    }
}

#[derive(serde::Deserialize)]
struct ProtoWorldPreset {
    dimensions: BTreeMap<String, ProtoLevelStem>,
}

#[derive(serde::Deserialize)]
struct ProtoLevelStem {
    generator: ProtoChunkGenerator,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
enum ProtoChunkGenerator {
    #[serde(rename = "minecraft:noise", alias = "noise")]
    Noise { settings: String },
    #[serde(other)]
    Unsupported,
}

/// The overworld noise settings a world preset names, as a handle rather than
/// an id, so the whole density-function graph is pulled in with it.
#[derive(TypePath, Debug)]
pub struct WorldPresetAsset {
    pub overworld_noise_settings: Handle<NoiseGeneratorSettingsAsset>,
}

impl Asset for WorldPresetAsset {}

impl VisitAssetDependencies for WorldPresetAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        visit(self.overworld_noise_settings.id().untyped());
    }
}

#[derive(Default, TypePath)]
pub struct WorldPresetLoader;

#[derive(Debug, Error)]
pub enum WorldPresetLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("the world preset names no minecraft:overworld dimension")]
    NoOverworld,
    #[error(
        "the overworld generator is not minecraft:noise, which is the only generator this \
         worldgen supports"
    )]
    NotNoiseGenerator,
}

impl AssetLoader for WorldPresetLoader {
    type Asset = WorldPresetAsset;
    type Settings = ();
    type Error = WorldPresetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let preset = serde_json::from_slice::<ProtoWorldPreset>(&bytes)?;

        let overworld = preset
            .dimensions
            .get("minecraft:overworld")
            .or_else(|| preset.dimensions.get("overworld"))
            .ok_or(WorldPresetLoaderError::NoOverworld)?;

        let ProtoChunkGenerator::Noise { settings } = &overworld.generator else {
            return Err(WorldPresetLoaderError::NotNoiseGenerator);
        };
        let (namespace, path) = settings
            .split_once(':')
            .unwrap_or(("minecraft", settings.as_str()));

        Ok(WorldPresetAsset {
            overworld_noise_settings: load_context
                .load(format!("{namespace}/worldgen/noise_settings/{path}.json")),
        })
    }
}

pub struct NoiseGeneratorSettingsPlugin;

/// Registers the worldgen asset types and their loaders, and nothing else.
///
/// A world preset names its noise settings, so loading one allocates a
/// `NoiseGeneratorSettingsAsset` handle. Any app that reads a preset therefore
/// needs these types even when it never builds a noise router itself.
pub struct WorldgenAssetsPlugin;

impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DensityFunctionAsset>()
            .init_asset::<NoiseGeneratorSettingsAsset>()
            .init_asset::<NoiseParamAsset>()
            .init_asset::<WorldPresetAsset>()
            .register_asset_loader(DensityFunctionLoader)
            .register_asset_loader(NoiseGeneratorSettingsLoader)
            .register_asset_loader(NoiseParamLoader)
            .register_asset_loader(WorldPresetLoader);
    }
}

impl Plugin for NoiseGeneratorSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldgenAssetsPlugin)
            .add_systems(Startup, request_world_preset)
            .add_systems(
                Update,
                build_overworld_noise_router.in_set(BuildNoiseRouter),
            );
    }
}

/// The router reads the block state ids out of [`WorldGenConfig`], so whoever
/// resolves them against the block registry runs before this.
#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildNoiseRouter;

/// Retains the handle so the preset and everything it names stay loaded.
#[derive(Resource)]
pub struct WorldPresetHandle(pub Handle<WorldPresetAsset>);

#[derive(Resource)]
pub struct OverworldNoiseRouter(pub Arc<NoiseRouter>);

fn request_world_preset(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    world_gen_config: Res<WorldGenConfig>,
) {
    let asset_path = world_gen_config.preset_asset_path();

    info!(asset_path = %asset_path, "Loading world preset");

    commands.insert_resource(WorldPresetHandle(asset_server.load(asset_path)));
}

fn build_overworld_noise_router(
    mut commands: Commands,
    mut settled: Local<bool>,
    asset_server: Res<AssetServer>,
    preset_handle: Option<Res<WorldPresetHandle>>,
    presets: Res<Assets<WorldPresetAsset>>,
    noise_settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
    density_functions: Res<Assets<DensityFunctionAsset>>,
    noises: Res<Assets<NoiseParamAsset>>,
    config: Res<WorldGenConfig>,
) {
    if *settled {
        return;
    }
    let Some(preset_handle) = preset_handle.as_deref() else {
        return;
    };
    match asset_server.recursive_dependency_load_state(preset_handle.0.id()) {
        RecursiveDependencyLoadState::Loaded => {}
        RecursiveDependencyLoadState::Failed(error) => {
            *settled = true;
            error!(%error, "the world preset did not load");
            return;
        }
        _ => return,
    }
    let Some(settings_handle) = presets
        .get(&preset_handle.0)
        .map(|p| &p.overworld_noise_settings)
    else {
        return;
    };
    let Some(asset) = noise_settings.get(settings_handle) else {
        return;
    };

    // `resolve_worldgen_default_states` reads these off the same asset one
    // system earlier, so an unresolved pair means its message has not landed
    // yet, not that nobody will ever state them.
    let (Some(default_block), Some(default_fluid)) =
        (config.default_block_state_id, config.default_fluid_state_id)
    else {
        return;
    };

    let mut registry = BTreeMap::new();
    let mut noise_params = BTreeMap::new();
    collect(
        &asset.density_functions,
        &asset.noises,
        &density_functions,
        &noises,
        &mut registry,
        &mut noise_params,
    );

    let seed = config.seed;
    info!(
        noise_settings = ?settings_handle.path(),
        seed = seed,
        "Building OverworldNoiseRouter"
    );

    *settled = true;
    match build_router(
        &asset.settings,
        &registry,
        &noise_params,
        seed,
        default_block,
        default_fluid,
    ) {
        Ok(router) => {
            for (name, error) in router.failed_roots() {
                error!(root = name, %error, "density root did not compile");
            }
            commands.insert_resource(OverworldNoiseRouter(Arc::new(router)));
        }
        Err(error) => error!(%error, "the noise router did not compile"),
    }
}

/// Walks the loaded handle graph into the two flat registries the compiler
/// takes. A density function reached only through another one still carries its
/// own dependency handles, so the walk follows them rather than assuming the
/// settings asset named everything.
fn collect(
    function_handles: &BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    noise_handles: &BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
    density_functions: &Assets<DensityFunctionAsset>,
    noises: &Assets<NoiseParamAsset>,
    registry: &mut BTreeMap<ResourceLocation, DensityFunctionHolder>,
    noise_params: &mut BTreeMap<ResourceLocation, NoiseParam>,
) {
    for (id, handle) in noise_handles {
        if let Some(asset) = noises.get(handle) {
            noise_params.insert(id.clone(), asset.noise.clone());
        }
    }
    for (id, handle) in function_handles {
        if registry.contains_key(id) {
            continue;
        }
        let Some(asset) = density_functions.get(handle) else {
            continue;
        };
        registry.insert(id.clone(), asset.function.clone());
        collect(
            &asset.deps,
            &asset.noise_deps,
            density_functions,
            noises,
            registry,
            noise_params,
        );
    }
}

#[derive(TypePath, Debug)]
pub struct NoiseGeneratorSettingsAsset {
    pub settings: GeneratorSettings,
    pub density_functions: BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    pub noises: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
}

impl Asset for NoiseGeneratorSettingsAsset {}

impl bevy_asset::VisitAssetDependencies for NoiseGeneratorSettingsAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.density_functions.values() {
            visit(handle.id().untyped());
        }
        for handle in self.noises.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(TypePath, Debug, Clone)]
pub struct DensityFunctionAsset {
    pub function: DensityFunctionHolder,
    pub deps: BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    pub noise_deps: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
}

impl Asset for DensityFunctionAsset {}

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

#[derive(TypePath, Asset, Debug, Clone)]
pub struct NoiseParamAsset {
    pub noise: NoiseParam,
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
        let bytes = read_all(reader).await?;
        let settings = serde_json::from_slice::<GeneratorSettings>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut deps = Dependencies::new(load_context);
        for root in settings.noise_router.roots() {
            deps.visit_holder(root);
        }

        Ok(NoiseGeneratorSettingsAsset {
            settings,
            density_functions: deps.density_functions,
            noises: deps.noises,
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

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let noise = serde_json::from_slice::<NoiseParam>(&bytes)
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

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let function = serde_json::from_slice::<DensityFunctionHolder>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut deps = Dependencies::new(load_context);
        deps.visit_holder(&function);

        Ok(DensityFunctionAsset {
            function,
            deps: deps.density_functions,
            noise_deps: deps.noises,
        })
    }
}

/// Turns the references one asset names into handles, one level deep: a
/// referenced function's own references are collected when that asset loads.
struct Dependencies<'a, 'b> {
    load_context: &'a mut LoadContext<'b>,
    density_functions: BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    noises: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
}

impl<'a, 'b> Dependencies<'a, 'b> {
    fn new(load_context: &'a mut LoadContext<'b>) -> Self {
        Self {
            load_context,
            density_functions: BTreeMap::new(),
            noises: BTreeMap::new(),
        }
    }

    fn visit_holder(&mut self, holder: &DensityFunctionHolder) {
        match holder {
            DensityFunctionHolder::Value(_) => {}
            DensityFunctionHolder::Reference(id) => self.visit_reference(id),
            DensityFunctionHolder::Owned(function) => self.visit_function(function),
        }
    }

    fn visit_function(&mut self, function: &ProtoDensityFunction) {
        if let Some(noise) = noise_holder(function) {
            self.visit_noise(noise);
        }
        function.visit_children(&mut |child| self.visit_holder(child));
    }

    fn visit_reference(&mut self, id: &ResourceLocation) {
        if self.density_functions.contains_key(id) {
            return;
        }
        let handle = self.load_context.load(format!(
            "{}/worldgen/density_function/{}.json",
            id.namespace(),
            id.path()
        ));
        self.density_functions.insert(id.clone(), handle);
    }

    fn visit_noise(&mut self, noise: &NoiseHolder) {
        let NoiseHolder::Reference(id) = noise else {
            return;
        };
        if self.noises.contains_key(id) {
            return;
        }
        let handle = self.load_context.load(format!(
            "{}/worldgen/noise/{}.json",
            id.namespace(),
            id.path()
        ));
        self.noises.insert(id.clone(), handle);
    }
}

fn noise_holder(function: &ProtoDensityFunction) -> Option<&NoiseHolder> {
    match function {
        ProtoDensityFunction::Noise { noise, .. }
        | ProtoDensityFunction::Shift { noise }
        | ProtoDensityFunction::ShiftA { noise }
        | ProtoDensityFunction::ShiftB { noise } => Some(noise),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ProtoChunkGenerator, ProtoWorldPreset, WorldGenConfig};

    fn overworld_settings(preset: &str) -> String {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(format!(
                    "assets/minecraft/worldgen/world_preset/{preset}.json"
                )),
        )
        .unwrap();
        let preset: ProtoWorldPreset = serde_json::from_slice(&bytes).unwrap();
        match &preset.dimensions["minecraft:overworld"].generator {
            ProtoChunkGenerator::Noise { settings } => settings.clone(),
            ProtoChunkGenerator::Unsupported => panic!("expected a noise generator"),
        }
    }

    #[test]
    fn noise_settings_for_normal_preset_is_overworld() {
        assert_eq!(overworld_settings("normal"), "minecraft:overworld");
    }

    #[test]
    fn noise_settings_for_beta_preset_is_beta() {
        assert_eq!(overworld_settings("beta"), "minecraft:beta");
    }

    #[test]
    fn default_config_names_the_normal_preset_asset() {
        assert_eq!(
            WorldGenConfig::default().preset_asset_path(),
            "minecraft/worldgen/world_preset/normal.json"
        );
    }
}
