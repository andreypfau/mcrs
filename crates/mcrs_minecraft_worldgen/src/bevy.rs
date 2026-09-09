use crate::compile::{CompileError, build_router};
use crate::material::compile::SURFACE_NOISE_NAMES;
use crate::material::proto::{MaterialCondition, MaterialRule};
use crate::material::{MaterialConditionHolder, MaterialInputs, MaterialRuleHolder};
use crate::proto::{
    BlockState, DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction,
};
use crate::router::{NoiseGeneratorSettings, NoiseRouter};
use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetLoader, Assets, Handle, LoadContext, LoadDirectError, UntypedAssetId,
    VisitAssetDependencies,
};
use bevy_ecs::prelude::{Res, Resource};
use bevy_ecs::system::SystemParam;
use bevy_reflect::TypePath;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::asset::{JsonLoader, read_all};
use mcrs_voxel_storage::VoxelId;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;

/// Registers the worldgen asset types and their loaders, and nothing else.
///
/// The world preset that names these settings is loaded by whoever owns the
/// dimension list; this crate is handed the settings asset it produced.
pub struct WorldgenAssetsPlugin;

impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DensityFunctionAsset>()
            .init_asset::<NoiseGeneratorSettingsAsset>()
            .init_asset::<NoiseParamAsset>()
            .init_asset::<CarverConfigAsset>()
            .init_asset::<MaterialRuleAsset>()
            .init_asset::<MaterialConditionAsset>()
            .register_asset_loader(WorldgenAssetLoader::<DensityFunctionAsset>::default())
            .register_asset_loader(WorldgenAssetLoader::<NoiseGeneratorSettingsAsset>::default())
            .register_asset_loader(JsonLoader::<NoiseParamAsset>::default())
            .register_asset_loader(JsonLoader::<CarverConfigAsset>::default())
            .register_asset_loader(WorldgenAssetLoader::<MaterialRuleAsset>::default())
            .register_asset_loader(WorldgenAssetLoader::<MaterialConditionAsset>::default());
    }
}

/// One dimension's compiled router. Built once where the assets are loaded and
/// handed to that dimension's sub-app as a read-only snapshot.
#[derive(Resource)]
pub struct DimensionNoiseRouter(pub Arc<NoiseRouter>);

/// Compiles one dimension's router from its loaded noise settings.
///
/// `block` resolves a datapack block state against the block registry this
/// crate does not have, and `biome` an id against the numbering the column's
/// biome grid holds. Both are lookups the caller owns; the terrain block and
/// the sea fluid come from the settings themselves, so they are per dimension
/// rather than global.
pub fn build_dimension_router(
    settings: &NoiseGeneratorSettingsAsset,
    assets: &WorldgenAssets<'_>,
    seed: u64,
    block: &dyn Fn(&BlockState) -> Option<VoxelId>,
    biome: &dyn Fn(&ResourceLocation) -> Option<u32>,
) -> Result<NoiseRouter, CompileError> {
    let mut loaded = Loaded::default();
    loaded.collect(&settings.deps, assets);

    let resolve = |state: &BlockState| {
        block(state).ok_or_else(|| CompileError::UnknownBlockState(state.name.as_str().to_string()))
    };
    let default_block = resolve(&settings.settings.default_block)?;
    let default_fluid = resolve(&settings.settings.default_fluid)?;

    let material = MaterialInputs {
        rules: &loaded.rules,
        conditions: &loaded.conditions,
        block,
        biome,
    };
    build_router(
        &settings.settings,
        &loaded.density_functions,
        &loaded.noises,
        seed,
        default_block,
        default_fluid,
        Some(&material),
    )
}

/// The four registries a worldgen asset can name. Each one is spelled here
/// once and turned into the set of ids an asset names, the handles those ids
/// load to, the values that arrive, and the walk that flattens them.
///
/// `leaf` registries hold a value with no references of its own; `nested` ones
/// carry their own [`AssetRefs`], so collecting one follows them.
macro_rules! registries {
    (
        leaf { $(($lname:ident, $lfolder:literal, $lasset:ty, $lvalue:ty, $lfield:ident)),* $(,)? }
        nested {
            $(($nname:ident, $nfolder:literal, $nasset:ident, $nvalue:ty, $nfield:ident, $nvisit:ident)),* $(,)?
        }
    ) => {
        $(
            #[derive(Asset, TypePath, Debug, Clone)]
            pub struct $nasset {
                pub $nfield: $nvalue,
                #[dependency]
                pub deps: AssetRefs,
            }

            impl WorldgenAsset for $nasset {
                type Proto = $nvalue;

                fn references(proto: &Self::Proto) -> References {
                    let mut refs = References::default();
                    refs.$nvisit(proto);
                    refs
                }

                fn build(proto: Self::Proto, deps: AssetRefs) -> Self {
                    Self { $nfield: proto, deps }
                }
            }
        )*

        /// The ids one asset names, one level deep: a referenced asset's own
        /// references are collected when that asset loads.
        #[derive(Default, Debug, PartialEq, Eq)]
        pub(crate) struct References {
            $(pub(crate) $lname: BTreeSet<ResourceLocation>,)*
            $(pub(crate) $nname: BTreeSet<ResourceLocation>,)*
        }

        /// The handles one asset's references resolve to. Every worldgen asset
        /// names ids from the same four registries, so one dependency visitor
        /// and one loader serve all of them.
        #[derive(Default, Debug, Clone)]
        pub struct AssetRefs {
            $(pub $lname: BTreeMap<ResourceLocation, Handle<$lasset>>,)*
            $(pub $nname: BTreeMap<ResourceLocation, Handle<$nasset>>,)*
        }

        impl AssetRefs {
            fn load(refs: &References, load_context: &mut LoadContext<'_>) -> Self {
                Self {
                    $($lname: handles(&refs.$lname, $lfolder, load_context),)*
                    $($nname: handles(&refs.$nname, $nfolder, load_context),)*
                }
            }
        }

        impl VisitAssetDependencies for AssetRefs {
            fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
                $(for handle in self.$lname.values() {
                    visit(handle.id().untyped());
                })*
                $(for handle in self.$nname.values() {
                    visit(handle.id().untyped());
                })*
            }
        }

        /// Walks the loaded handle graph into the flat registries the compiler
        /// takes. A referenced asset carries its own dependency handles, so the
        /// walk follows them rather than assuming the settings asset named
        /// everything.
        #[derive(Default)]
        struct Loaded {
            $($lname: BTreeMap<ResourceLocation, $lvalue>,)*
            $($nname: BTreeMap<ResourceLocation, $nvalue>,)*
        }

        /// The loaded worldgen registries a router is compiled out of, as the
        /// asset collections themselves rather than a copy of their contents.
        #[derive(SystemParam)]
        pub struct WorldgenAssets<'w> {
            $(pub $lname: Res<'w, Assets<$lasset>>,)*
            $(pub $nname: Res<'w, Assets<$nasset>>,)*
        }

        impl Loaded {
            /// An id is recorded before its own references are followed, so a
            /// reference cycle in the corpus terminates here rather than
            /// recursing forever.
            fn collect(&mut self, refs: &AssetRefs, tables: &WorldgenAssets<'_>) {
                $(for (id, handle) in &refs.$lname {
                    if let Some(asset) = tables.$lname.get(handle) {
                        self.$lname.insert(id.clone(), asset.$lfield.clone());
                    }
                })*
                $(for (id, handle) in &refs.$nname {
                    if self.$nname.contains_key(id) {
                        continue;
                    }
                    let Some(asset) = tables.$nname.get(handle) else {
                        continue;
                    };
                    self.$nname.insert(id.clone(), asset.$nfield.clone());
                    self.collect(&asset.deps, tables);
                })*
            }
        }
    };
}

registries! {
    leaf {
        (noises, "noise", NoiseParamAsset, NoiseParam, noise),
    }
    nested {
        (
            density_functions,
            "density_function",
            DensityFunctionAsset,
            DensityFunctionHolder,
            function,
            visit_holder
        ),
        (
            conditions,
            "material_condition",
            MaterialConditionAsset,
            MaterialConditionHolder,
            condition,
            visit_condition_holder
        ),
        (
            rules,
            "material_rule",
            MaterialRuleAsset,
            MaterialRuleHolder,
            rule,
            visit_rule_holder
        ),
    }
}

#[derive(Asset, TypePath, Debug)]
pub struct NoiseGeneratorSettingsAsset {
    pub settings: NoiseGeneratorSettings,
    #[dependency]
    pub deps: AssetRefs,
}

#[derive(Asset, TypePath, Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct NoiseParamAsset {
    pub noise: NoiseParam,
}

#[derive(Asset, TypePath, Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
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

/// The JSON loader for an asset that names other worldgen assets: parse, turn
/// the ids into handles, keep both. A leaf takes
/// [`mcrs_minecraft_core::asset::JsonLoader`] instead.
#[derive(TypePath)]
pub struct WorldgenAssetLoader<A: TypePath>(PhantomData<fn() -> A>);

impl<A: TypePath> Default for WorldgenAssetLoader<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A: WorldgenAsset> AssetLoader for WorldgenAssetLoader<A> {
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
    use super::References;
    use crate::material::compile::SURFACE_NOISE_NAMES;
    use crate::material::{MaterialConditionHolder, MaterialRuleHolder};
    use crate::router::NoiseGeneratorSettings;
    use mcrs_minecraft_core::ResourceLocation;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::corpus::{json_files, read, worldgen_dir};

    /// Every shipped biome, numbered by its position in the registry directory,
    /// which is all the material rules need of a biome id.
    fn shipped_biome_ids() -> BTreeMap<ResourceLocation, u32> {
        json_files(&worldgen_dir().join("biome"))
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let name = path.file_stem().unwrap().to_str().unwrap();
                (ResourceLocation::minecraft(name), index as u32)
            })
            .collect()
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

        let mut paths = json_files(&worldgen_dir().join("material_rule"));
        paths.extend(json_files(&worldgen_dir().join("material_condition")));
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
        let settings_files = json_files(&worldgen_dir().join("noise_settings"));
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

        let rules = json_files(&worldgen_dir().join("material_rule"));
        assert_eq!(reached.rules.len(), rules.len());
    }

    /// Drives a real asset server over the shipped corpus until the named noise
    /// settings and everything they reference have landed.
    fn load_settings(name: &str) -> bevy_app::App {
        use super::{NoiseGeneratorSettingsAsset, WorldgenAssetsPlugin};
        use bevy_app::App;
        use bevy_asset::{AssetPlugin, AssetServer, Handle, RecursiveDependencyLoadState};

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
            .load(format!("minecraft/worldgen/noise_settings/{name}.json"));
        app.insert_resource(SettingsHandle(handle.clone()));

        for _ in 0..10_000 {
            app.update();
            match app
                .world()
                .resource::<AssetServer>()
                .recursive_dependency_load_state(handle.id())
            {
                RecursiveDependencyLoadState::Loaded => return app,
                RecursiveDependencyLoadState::Failed(error) => panic!("{error}"),
                _ => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
        panic!("the {name} noise settings never finished loading");
    }

    #[derive(bevy_ecs::prelude::Resource)]
    struct SettingsHandle(bevy_asset::Handle<super::NoiseGeneratorSettingsAsset>);

    /// The registries as the system that builds a router receives them.
    fn assets_of(
        app: &mut bevy_app::App,
    ) -> bevy_ecs::system::SystemState<super::WorldgenAssets<'static>> {
        bevy_ecs::system::SystemState::new(app.world_mut())
    }

    /// The walk above proves the ids are named; this proves they arrive. A
    /// handle map left out of `visit_dependencies` or a branch missing from the
    /// collect walk loses the assets with no error anywhere.
    #[test]
    fn the_asset_pipeline_delivers_the_material_registries() {
        use super::{Loaded, NoiseGeneratorSettingsAsset};
        use bevy_asset::Assets;

        let mut app = load_settings("overworld");
        let mut state = assets_of(&mut app);
        let world = app.world();
        let handle = world.resource::<SettingsHandle>().0.clone();
        let asset = world
            .resource::<Assets<NoiseGeneratorSettingsAsset>>()
            .get(&handle)
            .unwrap();
        let mut collected = Loaded::default();
        collected.collect(&asset.deps, &state.get(world).unwrap());

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

    /// End to end over the shipped corpus: what the asset pipeline delivers is
    /// what the compiler needs, the terrain block and the sea fluid included —
    /// those come from the settings asset itself, so each dimension gets its
    /// own rather than whichever settings loaded last.
    #[test]
    fn the_loaded_settings_compile_into_a_router() {
        use super::{NoiseGeneratorSettingsAsset, build_dimension_router};
        use crate::proto::BlockState;
        use bevy_asset::Assets;
        use mcrs_voxel_storage::VoxelId;

        let biomes = shipped_biome_ids();
        for name in ["overworld", "nether", "end", "beta"] {
            let mut app = load_settings(name);
            let mut state = assets_of(&mut app);
            let world = app.world();
            let handle = world.resource::<SettingsHandle>().0.clone();
            let asset = world
                .resource::<Assets<NoiseGeneratorSettingsAsset>>()
                .get(&handle)
                .unwrap();

            // Every distinct state gets a distinct id, so a router that mixed
            // the terrain block up with the sea fluid would not compare equal.
            let states = std::cell::RefCell::new(BTreeMap::<String, VoxelId>::new());
            let block = |state: &BlockState| {
                let mut states = states.borrow_mut();
                let next = VoxelId(states.len() as u16 + 1);
                Some(*states.entry(state.name.as_str().to_owned()).or_insert(next))
            };
            let router = build_dimension_router(
                asset,
                &state.get(world).unwrap(),
                0,
                &block,
                &|id: &ResourceLocation| biomes.get(id).copied(),
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));

            assert!(router.material().is_some(), "{name} has no material rules");
            assert_ne!(
                router.default_block_state, router.default_fluid_state,
                "{name} resolved the terrain block and the sea fluid to one id"
            );
        }
    }
}
