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
    LoadState, UntypedAssetId, VisitAssetDependencies,
};
use bevy_ecs::prelude::{
    Commands, IntoScheduleConfigs, Res, Resource, SystemCondition, not, resource_exists,
};
use bevy_reflect::TypePath;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::asset::read_all;
use mcrs_voxel_storage::VoxelId;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

/// Which world preset to generate, which of its dimensions this world is, and
/// the seed.
///
/// `MCRS_WORLD_PRESET` and `MCRS_WORLD_SEED` override the preset and the seed.
/// A dimension sub-app inserts its own copy with `dimension` set to the one it
/// runs; the default names the overworld.
#[derive(Resource, Clone, Debug)]
pub struct WorldGenConfig {
    pub preset: ResourceLocation,
    pub dimension: ResourceLocation,
    pub seed: u64,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        let preset = env::var("MCRS_WORLD_PRESET")
            .ok()
            .map(|raw| raw.trim().to_lowercase())
            .filter(|raw| !raw.is_empty())
            .map(|raw| {
                ResourceLocation::parse(&raw).unwrap_or_else(|_| ResourceLocation::minecraft(&raw))
            })
            .unwrap_or_else(|| ResourceLocation::minecraft("normal"));
        let seed = env::var("MCRS_WORLD_SEED")
            .ok()
            .and_then(|raw| raw.trim().parse().ok())
            .unwrap_or(0);
        Self {
            preset,
            dimension: ResourceLocation::minecraft("overworld"),
            seed,
        }
    }
}

impl WorldGenConfig {
    pub fn preset_asset_path(&self) -> String {
        format!(
            "{}/worldgen/world_preset/{}.json",
            self.preset.namespace(),
            self.preset.path()
        )
    }
}

#[derive(serde::Deserialize)]
struct ProtoWorldPreset {
    dimensions: BTreeMap<ResourceLocation, ProtoLevelStem>,
}

#[derive(serde::Deserialize)]
struct ProtoLevelStem {
    generator: ProtoChunkGenerator,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
enum ProtoChunkGenerator {
    #[serde(rename = "minecraft:noise")]
    Noise { settings: ResourceLocation },
    /// A flat or debug generator. This worldgen only drives noise generators, so
    /// the dimension is left out of the map rather than refused: a preset naming
    /// one still serves whichever of its dimensions does use noise.
    #[serde(other)]
    Unsupported,
}

/// The noise settings each of a preset's dimensions names, as handles rather
/// than ids, so a dimension's whole density-function graph is pulled in with it.
#[derive(Default, Debug, Clone)]
pub struct DimensionSettings(pub BTreeMap<ResourceLocation, Handle<NoiseGeneratorSettingsAsset>>);

impl VisitAssetDependencies for DimensionSettings {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for handle in self.0.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(Asset, TypePath, Debug)]
pub struct WorldPresetAsset {
    #[dependency]
    pub noise_settings: DimensionSettings,
}

#[derive(Default, TypePath)]
pub struct WorldPresetLoader;

#[derive(Debug, Error)]
pub enum WorldPresetLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
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

        let noise_settings = preset
            .dimensions
            .into_iter()
            .filter_map(|(dimension, stem)| match stem.generator {
                ProtoChunkGenerator::Noise { settings } => {
                    let handle = load_context.load(format!(
                        "{}/worldgen/noise_settings/{}.json",
                        settings.namespace(),
                        settings.path()
                    ));
                    Some((dimension, handle))
                }
                ProtoChunkGenerator::Unsupported => None,
            })
            .collect();

        Ok(WorldPresetAsset {
            noise_settings: DimensionSettings(noise_settings),
        })
    }
}

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
            .register_asset_loader(JsonLoader::<DensityFunctionAsset>::default())
            .register_asset_loader(JsonLoader::<NoiseGeneratorSettingsAsset>::default())
            .register_asset_loader(JsonLoader::<NoiseParamAsset>::default())
            .register_asset_loader(JsonLoader::<CarverConfigAsset>::default())
            .register_asset_loader(JsonLoader::<MaterialRuleAsset>::default())
            .register_asset_loader(JsonLoader::<MaterialConditionAsset>::default())
            .register_asset_loader(WorldPresetLoader);
    }
}

/// [`WorldgenAssetsPlugin`] plus the systems that load the world preset and
/// compile its noise router. A dimension sub-app wants this one; the main app,
/// which only reads presets, wants the assets plugin alone.
pub struct NoiseGeneratorSettingsPlugin;

impl Plugin for NoiseGeneratorSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldgenAssetsPlugin)
            .init_resource::<WorldGenConfig>()
            .add_systems(Startup, request_world_preset)
            .add_systems(
                Update,
                build_dimension_noise_router.run_if(
                    not(resource_exists::<DimensionNoiseRouter>)
                        .and_then(not(resource_exists::<NoiseRouterUnavailable>)),
                ),
            );
    }
}

/// Retains the handle so the preset and everything it names stay loaded.
#[derive(Resource)]
pub struct WorldPresetHandle(pub Handle<WorldPresetAsset>);

/// The compiled router for the dimension [`WorldGenConfig::dimension`] names.
/// Each dimension sub-app builds its own in its own world.
#[derive(Resource)]
pub struct DimensionNoiseRouter(pub Arc<NoiseRouter>);

/// Inserted where this dimension can never get a router: the preset failed to
/// load, names no noise generator for it, or its material rules did not
/// compile. Stops the build retrying every tick for the life of the process.
#[derive(Resource)]
pub struct NoiseRouterUnavailable;

/// The terrain block and the sea fluid the active noise settings name, resolved
/// against the block registry this crate does not have. Inserted whole from the
/// side that owns that registry, which is what the router build waits on.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldgenDefaultStates {
    pub block: VoxelId,
    pub fluid: VoxelId,
}

/// The two lookups the material rules need and this crate cannot perform: a
/// rule's `result_state` becomes a stored block id, and a `biome_is` id becomes
/// the integer the column's biome grid holds. Inserted from the side that owns
/// those registries, as [`WorldgenDefaultStates`] is.
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

/// Compiles this dimension's router once three things have arrived: the noise
/// settings the preset names for it, loaded together with every asset they
/// reference, and the two registries this crate cannot resolve itself.
///
/// They arrive independently and in no fixed order, so readiness is asked of
/// the asset server rather than latched from a load message that a tick before
/// the registries exist would drop.
#[allow(clippy::too_many_arguments)]
fn build_dimension_noise_router(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    preset_handle: Option<Res<WorldPresetHandle>>,
    presets: Res<Assets<WorldPresetAsset>>,
    noise_settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
    density_functions: Res<Assets<DensityFunctionAsset>>,
    noises: Res<Assets<NoiseParamAsset>>,
    rules: Res<Assets<MaterialRuleAsset>>,
    conditions: Res<Assets<MaterialConditionAsset>>,
    resolvers: Option<Res<MaterialResolvers>>,
    defaults: Option<Res<WorldgenDefaultStates>>,
    config: Res<WorldGenConfig>,
) {
    let Some(preset_handle) = preset_handle.as_deref() else {
        return;
    };
    if let LoadState::Failed(error) = asset_server.load_state(preset_handle.0.id()) {
        error!(%error, "the world preset did not load");
        commands.insert_resource(NoiseRouterUnavailable);
        return;
    }
    let (Some(resolvers), Some(defaults)) = (resolvers, defaults) else {
        return;
    };
    let Some(preset) = presets.get(&preset_handle.0) else {
        return;
    };
    let Some(settings_handle) = preset.noise_settings.0.get(&config.dimension) else {
        error!(
            dimension = %config.dimension,
            preset = %config.preset,
            "the world preset names no noise generator for this dimension"
        );
        commands.insert_resource(NoiseRouterUnavailable);
        return;
    };
    if !asset_server.is_loaded_with_dependencies(settings_handle.id()) {
        return;
    }
    let Some(asset) = noise_settings.get(settings_handle) else {
        return;
    };

    let tables = AssetTables {
        density_functions: &density_functions,
        noises: &noises,
        rules: &rules,
        conditions: &conditions,
    };
    let mut loaded = Loaded::default();
    loaded.collect(&asset.deps, &tables);

    let seed = config.seed;
    info!(
        dimension = %config.dimension,
        noise_settings = ?settings_handle.path(),
        seed = seed,
        "Building DimensionNoiseRouter"
    );

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
        defaults.block,
        defaults.fluid,
        Some(&material),
    ) {
        Ok(router) => {
            for (name, error) in router.failed_roots() {
                error!(root = name, %error, "density root did not compile");
            }
            commands.insert_resource(DimensionNoiseRouter(Arc::new(router)));
        }
        Err(error) => {
            error!(
                material_rule = %asset.settings.material_rule,
                %error,
                "the material rules did not compile; no columns will generate"
            );
            commands.insert_resource(NoiseRouterUnavailable);
        }
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
    /// An id is recorded before its own references are followed, so a reference
    /// cycle in the corpus terminates here rather than recursing forever.
    fn collect(&mut self, refs: &AssetRefs, tables: &AssetTables<'_>) {
        for (id, handle) in &refs.noises {
            if let Some(asset) = tables.noises.get(handle) {
                self.noises.insert(id.clone(), asset.noise.clone());
            }
        }
        for (id, handle) in &refs.density_functions {
            if self.density_functions.contains_key(id) {
                continue;
            }
            let Some(asset) = tables.density_functions.get(handle) else {
                continue;
            };
            self.density_functions
                .insert(id.clone(), asset.function.clone());
            self.collect(&asset.deps, tables);
        }
        for (id, handle) in &refs.conditions {
            if self.conditions.contains_key(id) {
                continue;
            }
            let Some(asset) = tables.conditions.get(handle) else {
                continue;
            };
            self.conditions.insert(id.clone(), asset.condition.clone());
            self.collect(&asset.deps, tables);
        }
        for (id, handle) in &refs.rules {
            if self.rules.contains_key(id) {
                continue;
            }
            let Some(asset) = tables.rules.get(handle) else {
                continue;
            };
            self.rules.insert(id.clone(), asset.rule.clone());
            self.collect(&asset.deps, tables);
        }
    }
}

/// The handles one asset's references resolve to. Every worldgen asset names ids
/// from the same four registries, so one dependency visitor and one loader serve
/// all of them.
#[derive(Default, Debug, Clone)]
pub struct AssetRefs {
    pub density_functions: BTreeMap<ResourceLocation, Handle<DensityFunctionAsset>>,
    pub noises: BTreeMap<ResourceLocation, Handle<NoiseParamAsset>>,
    pub rules: BTreeMap<ResourceLocation, Handle<MaterialRuleAsset>>,
    pub conditions: BTreeMap<ResourceLocation, Handle<MaterialConditionAsset>>,
}

impl AssetRefs {
    fn load(refs: &References, load_context: &mut LoadContext<'_>) -> Self {
        Self {
            density_functions: handles(&refs.density_functions, "density_function", load_context),
            noises: handles(&refs.noises, "noise", load_context),
            rules: handles(&refs.rules, "material_rule", load_context),
            conditions: handles(&refs.conditions, "material_condition", load_context),
        }
    }
}

impl VisitAssetDependencies for AssetRefs {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        let ids = (self.density_functions.values().map(|h| h.id().untyped()))
            .chain(self.noises.values().map(|h| h.id().untyped()))
            .chain(self.rules.values().map(|h| h.id().untyped()))
            .chain(self.conditions.values().map(|h| h.id().untyped()));
        for id in ids {
            visit(id);
        }
    }
}

#[derive(Asset, TypePath, Debug)]
pub struct NoiseGeneratorSettingsAsset {
    pub settings: NoiseGeneratorSettings,
    #[dependency]
    pub deps: AssetRefs,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct DensityFunctionAsset {
    pub function: DensityFunctionHolder,
    #[dependency]
    pub deps: AssetRefs,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct MaterialRuleAsset {
    pub rule: MaterialRuleHolder,
    #[dependency]
    pub deps: AssetRefs,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct MaterialConditionAsset {
    pub condition: MaterialConditionHolder,
    #[dependency]
    pub deps: AssetRefs,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct NoiseParamAsset {
    pub noise: NoiseParam,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct CarverConfigAsset {
    pub config: crate::carver::CarverConfig,
}

#[derive(Debug, Error)]
pub enum WorldgenLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

/// The JSON a worldgen asset parses from and the ids that JSON names. Loading is
/// the same for every one of them: parse, turn the ids into handles, keep both.
trait WorldgenAsset: Asset {
    type Proto: DeserializeOwned;

    fn references(proto: &Self::Proto) -> References;

    fn build(proto: Self::Proto, deps: AssetRefs) -> Self;
}

impl WorldgenAsset for NoiseGeneratorSettingsAsset {
    type Proto = NoiseGeneratorSettings;

    fn references(settings: &Self::Proto) -> References {
        References::of_settings(settings)
    }

    fn build(settings: Self::Proto, deps: AssetRefs) -> Self {
        Self { settings, deps }
    }
}

impl WorldgenAsset for DensityFunctionAsset {
    type Proto = DensityFunctionHolder;

    fn references(function: &Self::Proto) -> References {
        let mut refs = References::default();
        refs.visit_holder(function);
        refs
    }

    fn build(function: Self::Proto, deps: AssetRefs) -> Self {
        Self { function, deps }
    }
}

impl WorldgenAsset for MaterialRuleAsset {
    type Proto = MaterialRuleHolder;

    fn references(rule: &Self::Proto) -> References {
        let mut refs = References::default();
        refs.visit_rule_holder(rule);
        refs
    }

    fn build(rule: Self::Proto, deps: AssetRefs) -> Self {
        Self { rule, deps }
    }
}

impl WorldgenAsset for MaterialConditionAsset {
    type Proto = MaterialConditionHolder;

    fn references(condition: &Self::Proto) -> References {
        let mut refs = References::default();
        refs.visit_condition_holder(condition);
        refs
    }

    fn build(condition: Self::Proto, deps: AssetRefs) -> Self {
        Self { condition, deps }
    }
}

impl WorldgenAsset for NoiseParamAsset {
    type Proto = NoiseParam;

    fn references(_: &Self::Proto) -> References {
        References::default()
    }

    fn build(noise: Self::Proto, _: AssetRefs) -> Self {
        Self { noise }
    }
}

impl WorldgenAsset for CarverConfigAsset {
    type Proto = crate::carver::CarverConfig;

    fn references(_: &Self::Proto) -> References {
        References::default()
    }

    fn build(config: Self::Proto, _: AssetRefs) -> Self {
        Self { config }
    }
}

#[derive(TypePath)]
pub struct JsonLoader<A: TypePath>(PhantomData<fn() -> A>);

impl<A: TypePath> Default for JsonLoader<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A: WorldgenAsset> AssetLoader for JsonLoader<A> {
    type Asset = A;
    type Settings = ();
    type Error = WorldgenLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let proto = serde_json::from_slice::<A::Proto>(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let deps = AssetRefs::load(&A::references(&proto), load_context);
        Ok(A::build(proto, deps))
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

    fn overworld_settings(preset: &str) -> ResourceLocation {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(format!(
                    "assets/minecraft/worldgen/world_preset/{preset}.json"
                )),
        )
        .unwrap();
        let preset: ProtoWorldPreset = serde_json::from_slice(&bytes).unwrap();
        match &preset.dimensions[&ResourceLocation::minecraft("overworld")].generator {
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
        collected.collect(&asset.deps, &tables);

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
        assert_eq!(overworld_settings("normal").as_str(), "minecraft:overworld");
    }

    #[test]
    fn noise_settings_for_beta_preset_is_beta() {
        assert_eq!(overworld_settings("beta").as_str(), "minecraft:beta");
    }

    #[test]
    fn default_config_names_the_normal_preset_asset() {
        assert_eq!(
            WorldGenConfig::default().preset_asset_path(),
            "minecraft/worldgen/world_preset/normal.json"
        );
    }

    /// The assets and the two registries the compiler needs arrive from three
    /// different sides in no fixed order. This drives the worst order: the
    /// settings finish loading, several ticks pass, and only then do the
    /// registries land.
    #[test]
    fn the_router_is_built_when_the_registries_arrive_after_the_assets() {
        use super::{
            DimensionNoiseRouter, MaterialResolvers, NoiseGeneratorSettingsPlugin,
            WorldgenDefaultStates,
        };
        use crate::proto::BlockState;
        use bevy_app::App;
        use bevy_asset::{AssetPlugin, AssetServer, RecursiveDependencyLoadState};
        use mcrs_voxel_storage::VoxelId;
        use std::sync::Arc;

        let mut app = App::new();
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..AssetPlugin::default()
        });
        app.insert_resource(WorldGenConfig::default());
        app.add_plugins(NoiseGeneratorSettingsPlugin);

        let mut loaded = false;
        for _ in 0..10_000 {
            app.update();
            let handle = app.world().get_resource::<super::WorldPresetHandle>();
            let Some(handle) = handle.map(|h| h.0.clone()) else {
                continue;
            };
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
        assert!(loaded, "the world preset never finished loading");

        for _ in 0..8 {
            app.update();
        }
        assert!(
            app.world().get_resource::<DimensionNoiseRouter>().is_none(),
            "the router cannot be built before the registries arrive"
        );

        app.insert_resource(WorldgenDefaultStates {
            block: VoxelId(1),
            fluid: VoxelId(2),
        });
        app.insert_resource(MaterialResolvers {
            block: Arc::new(|_: &BlockState| Some(VoxelId(1))),
            biome: Arc::new(|_: &ResourceLocation| Some(0)),
        });

        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world().get_resource::<DimensionNoiseRouter>().is_some(),
            "the registries arriving late did not reach the router build"
        );
    }
}
