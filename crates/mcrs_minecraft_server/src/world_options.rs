use crate::WorldSave;
use crate::loaded::Loaded;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use crate::refuse;
use mcrs_minecraft_world::save::read_world_gen_settings;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;
use std::sync::Arc;
use tracing::info;

/// The dimension list the server plugin was given, which replaces the save's and the preset's.
#[derive(Resource, Clone)]
pub(crate) struct PluginDimensions(pub Dimensions);

#[derive(Resource)]
pub(crate) struct WorldPresetName(pub String);

pub(crate) fn bake_dimensions(
    mut commands: Commands,
    registries: Res<CurrentRegistries>,
    save: Option<Res<WorldSave>>,
    given: Option<Res<PluginDimensions>>,
    preset: Res<WorldPresetName>,
    mut seed: ResMut<WorldSeed>,
) {
    let saved = save.map(|save| {
        let settings =
            read_world_gen_settings(&save.0, &registries).unwrap_or_else(|err| refuse(&err));
        seed.0 = settings.seed as u64;
        settings.dimensions
    });
    let (source, list) = if let Some(given) = given {
        ("plugin".to_owned(), bake_list(&given.0, &registries))
    } else if let Some(saved) = saved {
        ("save".to_owned(), bake(&saved, &registries))
    } else {
        let name = &preset.0;
        let presets = registries.loaded::<WorldPreset>();
        let Some(preset) = ResourceLocation::read(name)
            .ok()
            .and_then(|location| presets.by_name(&location))
            .and_then(|id| presets.get(id))
        else {
            refuse(&format!(
                "{}: no world preset is named {name}",
                mcrs_minecraft_world::keys::WORLD_PRESET
            ))
        };
        (name.clone(), bake(&preset.dimensions, &registries))
    };
    let list = list.unwrap_or_else(|error| refuse(&error));

    info!(%source, dimensions = list.len(), "dimension list");
    commands.insert_resource(DimensionList::new(list));
}

/// Every dimension the world spawns, in spawn order. Written once at `Startup`.
#[derive(Resource, Clone)]
pub struct DimensionList {
    keys: Arc<[ResourceKey<Dimension>]>,
    entries: Arc<[DimensionEntry]>,
}

impl DimensionList {
    pub fn new(list: Vec<(ResourceKey<Dimension>, DimensionEntry)>) -> Self {
        let (keys, entries): (Vec<_>, Vec<_>) = list.into_iter().unzip();
        DimensionList {
            keys: keys.into(),
            entries: entries.into(),
        }
    }

    pub fn keys(&self) -> &Arc<[ResourceKey<Dimension>]> {
        &self.keys
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceKey<Dimension>, &DimensionEntry)> {
        self.keys.iter().zip(self.entries.iter())
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// The seed every dimension's noise router is compiled against. One writer in
/// the host; the routers carry it into the sub-apps.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WorldSeed(pub u64);
