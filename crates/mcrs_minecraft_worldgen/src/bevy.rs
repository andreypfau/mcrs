use crate::compile::build_router;
use crate::proto::{DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction};
use crate::router::{GeneratorSettings, NoiseRouter};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetEvent, AssetLoader, AssetServer, Assets, Handle, LoadContext,
    LoadDirectError,
};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Res, Resource};
use bevy_reflect::TypePath;
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
    /// The noise settings the preset's overworld dimension names, from its
    /// `generator.settings`: `minecraft:overworld` for `minecraft:normal`,
    /// `minecraft:beta` for `minecraft:beta`.
    pub noise_settings_namespace: Arc<str>,
    pub noise_settings_path: Arc<str>,
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
            noise_settings_namespace: Arc::from("minecraft"),
            noise_settings_path: Arc::from("overworld"),
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

        let (noise_settings_namespace, noise_settings_path) =
            resolve_overworld_noise_settings(&preset_namespace, &preset_path);

        Self {
            preset_namespace,
            preset_path,
            noise_settings_namespace,
            noise_settings_path,
            seed,
            default_block_state_id: None,
            default_fluid_state_id: None,
        }
    }

    pub fn noise_settings_asset_path(&self) -> String {
        format!(
            "{}/worldgen/noise_settings/{}.json",
            self.noise_settings_namespace, self.noise_settings_path
        )
    }
}

#[derive(serde::Deserialize)]
struct WorldPreset {
    dimensions: BTreeMap<String, LevelStem>,
}

#[derive(serde::Deserialize)]
struct LevelStem {
    generator: ChunkGenerator,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
enum ChunkGenerator {
    #[serde(rename = "minecraft:noise", alias = "noise")]
    Noise { settings: String },
    #[serde(other)]
    Unsupported,
}

/// Where the asset system reads from, resolved exactly as `AssetPlugin` does so
/// the two never disagree. Chunk generation needs the preset before any
/// `AssetServer` exists, which is the only reason this path is read directly.
fn asset_root() -> std::path::PathBuf {
    bevy_asset::io::file::FileAssetReader::get_base_path()
        .join(bevy_asset::AssetPlugin::default().file_path)
}

/// The `generator.settings` id the preset states for `minecraft:overworld`.
pub(crate) fn resolve_overworld_noise_settings(
    preset_ns: &str,
    preset_path: &str,
) -> (Arc<str>, Arc<str>) {
    let preset_file = asset_root().join(format!(
        "{preset_ns}/worldgen/world_preset/{preset_path}.json"
    ));
    let json_path = preset_file.display();

    let data = std::fs::read_to_string(&preset_file)
        .unwrap_or_else(|e| panic!("cannot read world preset {json_path}: {e}"));
    let preset: WorldPreset = serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("cannot parse world preset {json_path}: {e}"));

    let overworld = preset
        .dimensions
        .get("minecraft:overworld")
        .or_else(|| preset.dimensions.get("overworld"))
        .unwrap_or_else(|| panic!("world preset {json_path} has no minecraft:overworld dimension"));

    let settings = match &overworld.generator {
        ChunkGenerator::Noise { settings } => settings.as_str(),
        ChunkGenerator::Unsupported => panic!(
            "world preset {json_path}: the overworld generator is not minecraft:noise, \
             which is the only generator this worldgen supports"
        ),
    };

    match settings.split_once(':') {
        Some((namespace, path)) => (Arc::from(namespace), Arc::from(path)),
        None => (Arc::from("minecraft"), Arc::from(settings)),
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
            .register_asset_loader(DensityFunctionLoader)
            .register_asset_loader(NoiseGeneratorSettingsLoader)
            .register_asset_loader(NoiseParamLoader);
    }
}

impl Plugin for NoiseGeneratorSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldgenAssetsPlugin)
            .add_systems(Startup, request_overworld_noise_settings)
            .add_systems(Update, build_noise_router_on_load.in_set(BuildNoiseRouter));
    }
}

/// The router reads the block state ids out of [`WorldGenConfig`], so whoever
/// resolves them against the block registry runs before this.
#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildNoiseRouter;

/// Retains the handle so the asset is not dropped before
/// `build_noise_router_on_load` can react to its load event.
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
        Some(config) => config.noise_settings_asset_path(),
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
    for event in messages.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        // Only the handle this plugin requested, not any other settings asset
        // that happens to be loaded.
        if noise_handle.as_ref().is_some_and(|h| h.0.id() != *id) {
            continue;
        }
        let Some(asset) = noise_settings.get(*id) else {
            continue;
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

        let config = world_gen_config.as_deref();
        let seed = config.map(|c| c.seed).unwrap_or(0);
        let noise_settings_id = config
            .map(|c| format!("{}:{}", c.noise_settings_namespace, c.noise_settings_path))
            .unwrap_or_else(|| "minecraft:overworld".to_string());
        info!(
            noise_settings = %noise_settings_id,
            seed = seed,
            "Building OverworldNoiseRouter"
        );
        let (default_block, default_fluid) = config
            .and_then(|c| Some((c.default_block_state_id?, c.default_fluid_state_id?)))
            .expect(
                "the noise settings default block and fluid were never resolved; \
                 a system before BuildNoiseRouter has to state them",
            );

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
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
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
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
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
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
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
    use super::resolve_overworld_noise_settings;

    #[test]
    fn noise_settings_for_normal_preset_is_overworld() {
        let (namespace, path) = resolve_overworld_noise_settings("minecraft", "normal");
        assert_eq!(namespace.as_ref(), "minecraft");
        assert_eq!(path.as_ref(), "overworld");
    }

    #[test]
    fn noise_settings_for_beta_preset_is_beta() {
        let (namespace, path) = resolve_overworld_noise_settings("minecraft", "beta");
        assert_eq!(namespace.as_ref(), "minecraft");
        assert_eq!(path.as_ref(), "beta");
    }
}
