use crate::compile::build_router;
use crate::material::compile::SURFACE_NOISE_NAMES;
use crate::material::proto::{MaterialCondition, MaterialRule};
use crate::material::{MaterialConditionHolder, MaterialInputs, MaterialRuleHolder};
use crate::proto::{
    BlockState, DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction,
};
use crate::router::{NoiseGeneratorSettings, NoiseRouter};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetLoader, AssetServer, Assets, Handle, LoadContext, LoadDirectError,
    RecursiveDependencyLoadState, VisitAssetDependencies,
};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Local, Res, Resource};
use bevy_reflect::TypePath;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::asset::read_all;
use mcrs_voxel_storage::VoxelId;
use std::collections::{BTreeMap, BTreeSet};
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
            .init_asset::<CarverConfigAsset>()
            .init_asset::<MaterialRuleAsset>()
            .init_asset::<MaterialConditionAsset>()
            .register_asset_loader(DensityFunctionLoader)
            .register_asset_loader(NoiseGeneratorSettingsLoader)
            .register_asset_loader(NoiseParamLoader)
            .register_asset_loader(WorldPresetLoader)
            .register_asset_loader(CarverConfigLoader)
            .register_asset_loader(MaterialRuleLoader)
            .register_asset_loader(MaterialConditionLoader);
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

/// The two lookups the material rules need and this crate cannot perform: a
/// rule's `result_state` becomes a stored block id, and a `biome_is` id becomes
/// the integer the column's biome grid holds. Insert before [`BuildNoiseRouter`]
/// from the side that owns those registries, as the default block states are.
#[derive(Resource, Clone)]
pub struct MaterialResolvers {
    pub block: Arc<dyn Fn(&BlockState) -> Option<VoxelId> + Send + Sync>,
    pub biome: Arc<dyn Fn(&ResourceLocation) -> Option<u32> + Send + Sync>,
}

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
    rules: Res<Assets<MaterialRuleAsset>>,
    conditions: Res<Assets<MaterialConditionAsset>>,
    resolvers: Option<Res<MaterialResolvers>>,
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
    // Inserted by the side that owns the block and biome registries, which this
    // crate does not have; absent means that system has not run yet.
    let Some(resolvers) = resolvers.as_deref() else {
        return;
    };

    let tables = AssetTables {
        density_functions: &density_functions,
        noises: &noises,
        rules: &rules,
        conditions: &conditions,
    };
    let mut loaded = Loaded::default();
    loaded.collect_noises(&asset.noises, &tables);
    loaded.collect_functions(&asset.density_functions, &tables);
    loaded.collect_rules(&asset.material_rules, &tables);

    let seed = config.seed;
    info!(
        noise_settings = ?settings_handle.path(),
        seed = seed,
        "Building OverworldNoiseRouter"
    );

    *settled = true;
    let material = MaterialInputs {
        rules: &loaded.rules,
        conditions: &loaded.conditions,
        block: &*resolvers.block,
        biome: &*resolvers.biome,
    };
    match build_router(
        &asset.settings,
        &loaded.density_functions,
        &loaded.noises,
        seed,
        default_block,
        default_fluid,
        Some(&material),
    ) {
        Ok(router) => {
            for (name, error) in router.failed_roots() {
                error!(root = name, %error, "density root did not compile");
            }
            commands.insert_resource(OverworldNoiseRouter(Arc::new(router)));
        }
        Err(error) => error!(
            material_rule = %asset.settings.material_rule,
            %error,
            "the material rules did not compile; no columns will generate"
        ),
    }
}

/// Walks the loaded handle graph into the flat registries the compiler takes. A
/// referenced asset carries its own dependency handles, so the walk follows them
/// rather than assuming the settings asset named everything.
#[derive(Default)]
struct Loaded {
    density_functions: BTreeMap<ResourceLocation, DensityFunctionHolder>,
    noises: BTreeMap<ResourceLocation, NoiseParam>,
    rules: BTreeMap<ResourceLocation, MaterialRuleHolder>,
    conditions: BTreeMap<ResourceLocation, MaterialConditionHolder>,
}

struct AssetTables<'a> {
    density_functions: &'a Assets<DensityFunctionAsset>,
    noises: &'a Assets<NoiseParamAsset>,
    rules: &'a Assets<MaterialRuleAsset>,
    conditions: &'a Assets<MaterialConditionAsset>,
}

impl Loaded {
    fn collect_noises(
        &mut self,
        handles: &BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
        tables: &AssetTables<'_>,
    ) {
        for (id, handle) in handles {
            if let Some(asset) = tables.noises.get(handle) {
                self.noises.insert(id.clone(), asset.noise.clone());
            }
        }
    }

    fn collect_functions(
        &mut self,
        handles: &BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
        tables: &AssetTables<'_>,
    ) {
        for (id, handle) in handles {
            if self.density_functions.contains_key(id) {
                continue;
            }
            let Some(asset) = tables.density_functions.get(handle) else {
                continue;
            };
            self.density_functions
                .insert(id.clone(), asset.function.clone());
            self.collect_noises(&asset.noise_deps, tables);
            self.collect_functions(&asset.deps, tables);
        }
    }

    fn collect_conditions(
        &mut self,
        handles: &BTreeMap<ResourceLocation, Handle<MaterialConditionAsset>>,
        tables: &AssetTables<'_>,
    ) {
        for (id, handle) in handles {
            if self.conditions.contains_key(id) {
                continue;
            }
            let Some(asset) = tables.conditions.get(handle) else {
                continue;
            };
            self.conditions.insert(id.clone(), asset.condition.clone());
            self.collect_noises(&asset.noises, tables);
            self.collect_conditions(&asset.conditions, tables);
        }
    }

    fn collect_rules(
        &mut self,
        handles: &BTreeMap<ResourceLocation, Handle<MaterialRuleAsset>>,
        tables: &AssetTables<'_>,
    ) {
        for (id, handle) in handles {
            if self.rules.contains_key(id) {
                continue;
            }
            let Some(asset) = tables.rules.get(handle) else {
                continue;
            };
            self.rules.insert(id.clone(), asset.rule.clone());
            self.collect_noises(&asset.noises, tables);
            self.collect_functions(&asset.density_functions, tables);
            self.collect_conditions(&asset.conditions, tables);
            self.collect_rules(&asset.rules, tables);
        }
    }
}

#[derive(TypePath, Debug)]
pub struct NoiseGeneratorSettingsAsset {
    pub settings: NoiseGeneratorSettings,
    pub density_functions: BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    pub noises: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
    pub material_rules: BTreeMap<ResourceLocation, Handle<MaterialRuleAsset>>,
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
        for handle in self.material_rules.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(TypePath, Debug, Clone)]
pub struct MaterialRuleAsset {
    pub rule: MaterialRuleHolder,
    pub rules: BTreeMap<ResourceLocation, Handle<MaterialRuleAsset>>,
    pub conditions: BTreeMap<ResourceLocation, Handle<MaterialConditionAsset>>,
    pub density_functions: BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    pub noises: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
}

impl Asset for MaterialRuleAsset {}

impl VisitAssetDependencies for MaterialRuleAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.rules.values() {
            visit(handle.id().untyped());
        }
        for handle in self.conditions.values() {
            visit(handle.id().untyped());
        }
        for handle in self.density_functions.values() {
            visit(handle.id().untyped());
        }
        for handle in self.noises.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(TypePath, Debug, Clone)]
pub struct MaterialConditionAsset {
    pub condition: MaterialConditionHolder,
    pub conditions: BTreeMap<ResourceLocation, Handle<MaterialConditionAsset>>,
    pub noises: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
}

impl Asset for MaterialConditionAsset {}

impl VisitAssetDependencies for MaterialConditionAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.conditions.values() {
            visit(handle.id().untyped());
        }
        for handle in self.noises.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(Default, TypePath)]
pub struct MaterialRuleLoader;

impl AssetLoader for MaterialRuleLoader {
    type Asset = MaterialRuleAsset;
    type Settings = ();
    type Error = WorldgenLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let rule = serde_json::from_slice::<MaterialRuleHolder>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut refs = References::default();
        refs.visit_rule_holder(&rule);

        Ok(MaterialRuleAsset {
            rule,
            rules: handles(&refs.rules, "material_rule", load_context),
            conditions: handles(&refs.conditions, "material_condition", load_context),
            density_functions: handles(&refs.density_functions, "density_function", load_context),
            noises: handles(&refs.noises, "noise", load_context),
        })
    }
}

#[derive(Default, TypePath)]
pub struct MaterialConditionLoader;

impl AssetLoader for MaterialConditionLoader {
    type Asset = MaterialConditionAsset;
    type Settings = ();
    type Error = WorldgenLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let condition = serde_json::from_slice::<MaterialConditionHolder>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut refs = References::default();
        refs.visit_condition_holder(&condition);

        Ok(MaterialConditionAsset {
            condition,
            conditions: handles(&refs.conditions, "material_condition", load_context),
            noises: handles(&refs.noises, "noise", load_context),
        })
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

#[derive(Debug, Error)]
pub enum WorldgenLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

#[derive(Default, TypePath)]
pub struct NoiseGeneratorSettingsLoader;

impl AssetLoader for NoiseGeneratorSettingsLoader {
    type Asset = NoiseGeneratorSettingsAsset;
    type Settings = ();
    type Error = WorldgenLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let settings = serde_json::from_slice::<NoiseGeneratorSettings>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let refs = References::of_settings(&settings);

        Ok(NoiseGeneratorSettingsAsset {
            settings,
            density_functions: handles(&refs.density_functions, "density_function", load_context),
            noises: handles(&refs.noises, "noise", load_context),
            material_rules: handles(&refs.rules, "material_rule", load_context),
        })
    }
}

#[derive(Default, TypePath)]
pub struct NoiseParamLoader;

impl AssetLoader for NoiseParamLoader {
    type Asset = NoiseParamAsset;
    type Settings = ();
    type Error = WorldgenLoaderError;

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

#[derive(TypePath, Debug, Clone)]
pub struct CarverConfigAsset {
    pub config: crate::carver::CarverConfig,
}

impl Asset for CarverConfigAsset {}

impl VisitAssetDependencies for CarverConfigAsset {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct CarverConfigLoader;

impl AssetLoader for CarverConfigLoader {
    type Asset = CarverConfigAsset;
    type Settings = ();
    type Error = WorldgenLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        Ok(CarverConfigAsset {
            config: serde_json::from_slice(&bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

#[derive(Default, TypePath)]
pub struct DensityFunctionLoader;

impl AssetLoader for DensityFunctionLoader {
    type Asset = DensityFunctionAsset;
    type Settings = ();
    type Error = WorldgenLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let function = serde_json::from_slice::<DensityFunctionHolder>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut refs = References::default();
        refs.visit_holder(&function);

        Ok(DensityFunctionAsset {
            function,
            deps: handles(&refs.density_functions, "density_function", load_context),
            noise_deps: handles(&refs.noises, "noise", load_context),
        })
    }
}

/// The ids one asset names, one level deep: a referenced asset's own references
/// are collected when that asset loads.
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct References {
    pub(crate) density_functions: BTreeSet<ResourceLocation>,
    pub(crate) noises: BTreeSet<ResourceLocation>,
    pub(crate) rules: BTreeSet<ResourceLocation>,
    pub(crate) conditions: BTreeSet<ResourceLocation>,
}

impl References {
    /// What the noise settings themselves name: the density roots, the material
    /// rule, and the nine noises the surface stage samples that no datapack file
    /// names.
    pub(crate) fn of_settings(settings: &NoiseGeneratorSettings) -> Self {
        let mut refs = Self::default();
        for root in settings.noise_router.roots() {
            refs.visit_holder(root);
        }
        refs.rules.insert(settings.material_rule.clone());
        refs.noises
            .extend(SURFACE_NOISE_NAMES.map(ResourceLocation::minecraft));
        refs
    }

    pub(crate) fn visit_holder(&mut self, holder: &DensityFunctionHolder) {
        match holder {
            DensityFunctionHolder::Value(_) => {}
            DensityFunctionHolder::Reference(id) => {
                self.density_functions.insert(id.clone());
            }
            DensityFunctionHolder::Owned(function) => self.visit_function(function),
        }
    }

    fn visit_function(&mut self, function: &ProtoDensityFunction) {
        if let Some(NoiseHolder::Reference(id)) = noise_holder(function) {
            self.noises.insert(id.clone());
        }
        function.visit_children(&mut |child| self.visit_holder(child));
    }

    pub(crate) fn visit_rule_holder(&mut self, holder: &MaterialRuleHolder) {
        match holder {
            MaterialRuleHolder::Reference(id) => {
                self.rules.insert(id.clone());
            }
            MaterialRuleHolder::Owned(rule) => self.visit_rule(rule),
        }
    }

    fn visit_rule(&mut self, rule: &MaterialRule) {
        match rule {
            MaterialRule::Block { .. } | MaterialRule::Bandlands => {}
            MaterialRule::Sequence { sequence } => {
                for member in sequence {
                    self.visit_rule_holder(member);
                }
            }
            MaterialRule::Condition { if_true, then_run } => {
                self.visit_condition_holder(if_true);
                self.visit_rule_holder(then_run);
            }
            MaterialRule::OreVein {
                density,
                richness,
                filler_gap,
                ..
            } => {
                for function in [density, richness, filler_gap] {
                    self.visit_holder(function);
                }
            }
        }
    }

    pub(crate) fn visit_condition_holder(&mut self, holder: &MaterialConditionHolder) {
        match holder {
            MaterialConditionHolder::Reference(id) => {
                self.conditions.insert(id.clone());
            }
            MaterialConditionHolder::Owned(condition) => self.visit_condition(condition),
        }
    }

    fn visit_condition(&mut self, condition: &MaterialCondition) {
        match condition {
            MaterialCondition::NoiseThreshold { noise, .. } => {
                self.noises.insert(noise.clone());
            }
            MaterialCondition::Not { invert } => self.visit_condition_holder(invert),
            _ => {}
        }
    }
}

fn handles<A: Asset>(
    ids: &BTreeSet<ResourceLocation>,
    folder: &str,
    load_context: &mut LoadContext<'_>,
) -> BTreeMap<ResourceLocation, Handle<A>> {
    ids.iter()
        .map(|id| {
            let handle = load_context.load(format!(
                "{}/worldgen/{folder}/{}.json",
                id.namespace(),
                id.path()
            ));
            (id.clone(), handle)
        })
        .collect()
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
    use super::{ProtoChunkGenerator, ProtoWorldPreset, References, WorldGenConfig};
    use crate::material::compile::SURFACE_NOISE_NAMES;
    use crate::material::{MaterialConditionHolder, MaterialRuleHolder};
    use crate::router::NoiseGeneratorSettings;
    use mcrs_minecraft_core::ResourceLocation;
    use serde::de::DeserializeOwned;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn worldgen_dir() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
    }

    fn read<T: DeserializeOwned>(folder: &str, id: &ResourceLocation) -> T {
        let path = worldgen_dir()
            .join(folder)
            .join(format!("{}.json", id.path()));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn json_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                json_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    /// The transitive closure of the loader's one-level walk, driven off disk
    /// the way the asset server drives it through the dependency handles.
    fn closure(settings: &NoiseGeneratorSettings) -> References {
        let mut all = References::of_settings(settings);
        let mut expanded = References::default();
        loop {
            let rules: Vec<_> = all.rules.difference(&expanded.rules).cloned().collect();
            let conditions: Vec<_> = all
                .conditions
                .difference(&expanded.conditions)
                .cloned()
                .collect();
            let functions: Vec<_> = all
                .density_functions
                .difference(&expanded.density_functions)
                .cloned()
                .collect();
            if rules.is_empty() && conditions.is_empty() && functions.is_empty() {
                return all;
            }
            for id in rules {
                all.visit_rule_holder(&read::<MaterialRuleHolder>("material_rule", &id));
                expanded.rules.insert(id);
            }
            for id in conditions {
                all.visit_condition_holder(&read::<MaterialConditionHolder>(
                    "material_condition",
                    &id,
                ));
                expanded.conditions.insert(id);
            }
            for id in functions {
                all.visit_holder(&read("density_function", &id));
                expanded.density_functions.insert(id);
            }
        }
    }

    /// The ids the shipped material rules and conditions name, read out of the
    /// raw JSON so the expectation does not come from the same walk under test.
    fn corpus_references() -> (BTreeSet<String>, BTreeSet<String>) {
        fn scan(
            value: &serde_json::Value,
            noises: &mut BTreeSet<String>,
            functions: &mut BTreeSet<String>,
        ) {
            match value {
                serde_json::Value::Array(items) => {
                    for item in items {
                        scan(item, noises, functions);
                    }
                }
                serde_json::Value::Object(fields) => {
                    match fields.get("type").and_then(|t| t.as_str()) {
                        Some("minecraft:noise_threshold") => {
                            noises.insert(fields["noise"].as_str().unwrap().to_owned());
                        }
                        Some("minecraft:ore_vein") => {
                            for slot in ["density", "richness", "filler_gap"] {
                                if let Some(id) = fields[slot].as_str() {
                                    functions.insert(id.to_owned());
                                }
                            }
                        }
                        _ => {}
                    }
                    for field in fields.values() {
                        scan(field, noises, functions);
                    }
                }
                _ => {}
            }
        }

        let mut paths = Vec::new();
        json_files(&worldgen_dir().join("material_rule"), &mut paths);
        json_files(&worldgen_dir().join("material_condition"), &mut paths);
        let (mut noises, mut functions) = (BTreeSet::new(), BTreeSet::new());
        for path in paths {
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            scan(&value, &mut noises, &mut functions);
        }
        (noises, functions)
    }

    /// The dependency walk is what makes the surface stage reachable at runtime:
    /// every asset it misses is on disk and never loaded, and no test that reads
    /// the corpus off disk itself would notice.
    #[test]
    fn the_settings_walk_reaches_every_asset_the_material_rules_name() {
        let mut settings_files = Vec::new();
        json_files(&worldgen_dir().join("noise_settings"), &mut settings_files);
        let mut reached = References::default();
        for path in &settings_files {
            let settings: NoiseGeneratorSettings =
                serde_json::from_slice(&std::fs::read(path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let found = closure(&settings);
            reached.noises.extend(found.noises);
            reached.density_functions.extend(found.density_functions);
            reached.rules.extend(found.rules);
            reached.conditions.extend(found.conditions);
        }

        let ids: BTreeSet<String> = reached
            .noises
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let functions: BTreeSet<String> = reached
            .density_functions
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();

        let (corpus_noises, corpus_functions) = corpus_references();
        assert!(!corpus_noises.is_empty() && !corpus_functions.is_empty());
        assert!(
            corpus_noises.is_subset(&ids),
            "noises the material rules name but the walk misses: {:?}",
            corpus_noises.difference(&ids).collect::<Vec<_>>()
        );
        assert!(
            corpus_functions.is_subset(&functions),
            "ore vein density functions the walk misses: {:?}",
            corpus_functions.difference(&functions).collect::<Vec<_>>()
        );
        for hardcoded in SURFACE_NOISE_NAMES {
            assert!(
                ids.contains(&format!("minecraft:{hardcoded}")),
                "the walk misses the hardcoded surface noise {hardcoded}"
            );
        }

        let mut rules = Vec::new();
        json_files(&worldgen_dir().join("material_rule"), &mut rules);
        assert_eq!(reached.rules.len(), rules.len());
    }

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

    /// The walk above proves the ids are named; this proves they arrive. A
    /// handle map left out of `visit_dependencies` or a branch missing from the
    /// collect walk loses the assets with no error anywhere.
    #[test]
    fn the_asset_pipeline_delivers_the_material_registries() {
        use super::{
            AssetTables, DensityFunctionAsset, Loaded, MaterialConditionAsset, MaterialRuleAsset,
            NoiseGeneratorSettingsAsset, NoiseParamAsset, WorldgenAssetsPlugin,
        };
        use bevy_app::App;
        use bevy_asset::{AssetPlugin, AssetServer, Assets, Handle, RecursiveDependencyLoadState};

        let mut app = App::new();
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..AssetPlugin::default()
        });
        app.add_plugins(WorldgenAssetsPlugin);

        let handle: Handle<NoiseGeneratorSettingsAsset> = app
            .world()
            .resource::<AssetServer>()
            .load("minecraft/worldgen/noise_settings/overworld.json");

        let mut loaded = false;
        for _ in 0..10_000 {
            app.update();
            match app
                .world()
                .resource::<AssetServer>()
                .recursive_dependency_load_state(handle.id())
            {
                RecursiveDependencyLoadState::Loaded => {
                    loaded = true;
                    break;
                }
                RecursiveDependencyLoadState::Failed(error) => panic!("{error}"),
                _ => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
        assert!(
            loaded,
            "the overworld noise settings never finished loading"
        );

        let world = app.world();
        let settings = world.resource::<Assets<NoiseGeneratorSettingsAsset>>();
        let asset = settings.get(&handle).unwrap();
        let tables = AssetTables {
            density_functions: world.resource::<Assets<DensityFunctionAsset>>(),
            noises: world.resource::<Assets<NoiseParamAsset>>(),
            rules: world.resource::<Assets<MaterialRuleAsset>>(),
            conditions: world.resource::<Assets<MaterialConditionAsset>>(),
        };
        let mut collected = Loaded::default();
        collected.collect_noises(&asset.noises, &tables);
        collected.collect_functions(&asset.density_functions, &tables);
        collected.collect_rules(&asset.material_rules, &tables);

        for name in SURFACE_NOISE_NAMES {
            let id = format!("minecraft:{name}");
            assert!(
                collected.noises.contains_key(id.as_str()),
                "the hardcoded surface noise {name} did not arrive"
            );
        }
        for id in [
            "minecraft:overworld/ore_vein/iron_density",
            "minecraft:overworld/ore_vein/copper_density",
            "minecraft:overworld/ore_vein/richness",
            "minecraft:overworld/ore_vein/gap",
        ] {
            assert!(
                collected.density_functions.contains_key(id),
                "{id} did not arrive"
            );
        }

        fn ids<V>(map: &BTreeMap<ResourceLocation, V>) -> BTreeSet<ResourceLocation> {
            map.keys().cloned().collect()
        }
        let expected = closure(&asset.settings);
        assert_eq!(ids(&collected.rules), expected.rules);
        assert_eq!(ids(&collected.conditions), expected.conditions);
        assert_eq!(
            ids(&collected.density_functions),
            expected.density_functions
        );
        assert_eq!(ids(&collected.noises), expected.noises);
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
