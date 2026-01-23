use crate::density_function::proto::{
    DensityFunctionHolder, NoiseHolder, ProtoDensityFunction, Visitor,
};
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
use thiserror::Error;

pub struct NoiseGeneratorSettingsPlugin;

impl Plugin for NoiseGeneratorSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DensityFunctionAsset>()
            .init_asset::<NoiseGeneratorSettingsAsset>()
            .register_asset_loader(DensityFunctionLoader)
            .register_asset_loader(NoiseGeneratorSettingsLoader)
            .add_systems(Update, print_loaded_noise_settings);

        app.add_systems(Startup, setup_overworld_noise_settings);
    }
}

#[derive(Resource)]
pub struct OverworldNoiseSettings(pub Handle<NoiseGeneratorSettingsAsset>);

fn setup_overworld_noise_settings(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(OverworldNoiseSettings(
        asset_server.load("minecraft/worldgen/noise_settings/overworld.json"),
    ))
}

fn print_loaded_noise_settings(
    mut messages: MessageReader<AssetEvent<NoiseGeneratorSettingsAsset>>,
    noise_settings: Res<Assets<NoiseGeneratorSettingsAsset>>,
) {
    messages.read().for_each(|event| match event {
        AssetEvent::LoadedWithDependencies { id } => {
            if let Some(settings) = noise_settings.get(*id) {
                println!("Loaded NoiseGeneratorSettingsAsset: {:?}", settings);
            }
        }
        _ => {}
    });
}

#[derive(TypePath, Debug)]
pub struct NoiseGeneratorSettingsAsset {
    pub settings: NoiseGeneratorSettings,
    pub density_functions: BTreeMap<Ident<String>, Handle<DensityFunctionAsset>>,
}

impl bevy_asset::Asset for NoiseGeneratorSettingsAsset {}
impl bevy_asset::VisitAssetDependencies for NoiseGeneratorSettingsAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.density_functions.values() {
            visit(handle.id().untyped());
        }
    }
}

#[derive(TypePath, Debug)]
pub struct DensityFunctionAsset {
    pub function: DensityFunctionHolder,
    pub deps: BTreeMap<Ident<String>, Handle<DensityFunctionAsset>>,
}

impl bevy_asset::Asset for DensityFunctionAsset {}
impl bevy_asset::VisitAssetDependencies for DensityFunctionAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {
        for handle in self.deps.values() {
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
        })
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
        };
        collector.visit_density_function_holder(&holder);

        Ok(DensityFunctionAsset {
            function: holder,
            deps: collector.density_functions,
        })
    }
}

struct DensityFunctionVisitor<'a, 'b> {
    pub load_context: &'a mut LoadContext<'b>,
    pub density_functions: BTreeMap<Ident<String>, Handle<DensityFunctionAsset>>,
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
        println!("Loading DensityFunctionAsset: {}", value);
        self.density_functions.insert(value.clone(), handle);
    }
}
