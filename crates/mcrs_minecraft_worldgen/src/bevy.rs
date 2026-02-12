use crate::density_function::proto::{
    DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction, Visitor,
};
use crate::density_function::{NoiseRouter, build_functions};
use crate::proto::{Either, NoiseGeneratorSettings};
use bevy_app::{App, FixedUpdate, Plugin, Startup, Update};
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
use std::sync::Arc;
use thiserror::Error;

pub struct NoiseGeneratorSettingsPlugin;

impl Plugin for NoiseGeneratorSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DensityFunctionAsset>()
            .init_asset::<NoiseGeneratorSettingsAsset>()
            .init_asset::<NoiseParamAsset>()
            .register_asset_loader(DensityFunctionLoader)
            .register_asset_loader(NoiseGeneratorSettingsLoader)
            .register_asset_loader(NoiseParamLoader)
            .add_systems(Update, print_loaded_noise_settings);

        app.add_systems(Startup, setup_overworld_noise_settings);
    }
}

#[derive(Resource)]
pub struct OverworldNoiseSettings(pub Handle<NoiseGeneratorSettingsAsset>);

#[derive(Resource)]
pub struct OverworldNoiseRouter(pub Arc<NoiseRouter>);

fn setup_overworld_noise_settings(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(OverworldNoiseSettings(
        asset_server.load("minecraft/worldgen/noise_settings/overworld.json"),
    ))
}

fn print_loaded_noise_settings(
    mut commands: Commands,
    mut messages: MessageReader<AssetEvent<NoiseGeneratorSettingsAsset>>,
    noise_settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
    density_functions: Res<Assets<DensityFunctionAsset>>,
    noises: Res<Assets<NoiseParamAsset>>,
) {
    messages.read().for_each(|event| match event {
        AssetEvent::LoadedWithDependencies { id } => {
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

                let overworld = OverworldNoiseRouter(Arc::new(build_functions(
                    &functions_proto,
                    &noises_proto,
                    &settings.settings,
                    2,
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

        // println!("Loaded NoiseParamAsset: {:?}", noise);

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
