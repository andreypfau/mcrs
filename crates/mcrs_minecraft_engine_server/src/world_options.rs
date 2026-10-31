use bevy_app::AppExit;
use bevy_ecs::prelude::{Commands, MessageWriter, ResMut};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine::world_seed::WorldSeed;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_save::read_world_gen_settings;
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;
use tracing::{error, info};

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
    mut exit: MessageWriter<AppExit>,
) {
    let baked = dimension_list(
        &registries,
        save.as_deref(),
        given.as_deref(),
        &preset.0,
        &mut seed,
    );
    match baked {
        Ok((source, list)) => {
            info!(%source, dimensions = list.len(), "dimension list");
            commands.insert_resource(DimensionList::new(
                list.iter()
                    .map(|(key, entry)| (key.clone(), entry.dimension_type)),
            ));
            commands.insert_resource(DimensionGenerators(
                list.into_iter().map(|(_, entry)| entry.generator).collect(),
            ));
        }
        Err(error) => {
            error!("the registries are unusable:\n{error}");
            exit.write(AppExit::error());
        }
    }
}

fn dimension_list(
    registries: &Registries,
    save: Option<&WorldSave>,
    given: Option<&PluginDimensions>,
    preset: &str,
    seed: &mut WorldSeed,
) -> Result<(String, Vec<(ResourceKeyBuf<Dimension>, DimensionEntry)>), String> {
    let saved = match save {
        Some(save) => {
            let settings =
                read_world_gen_settings(&save.0, registries).map_err(|error| error.to_string())?;
            seed.0 = settings.seed as u64;
            Some(settings.dimensions)
        }
        None => None,
    };
    if let Some(given) = given {
        Ok(("plugin".to_owned(), bake_list(&given.0, registries)?))
    } else if let Some(saved) = saved {
        Ok(("save".to_owned(), bake(&saved, registries)?))
    } else {
        let named = registries
            .registry::<WorldPreset>()
            .and_then(|presets| {
                presets.get(presets.by_name(&ResourceLocationBuf::read(preset).ok()?)?)
            })
            .ok_or_else(|| {
                format!(
                    "{}: no world preset is named {preset}",
                    mcrs_minecraft_world::keys::WORLD_PRESET
                )
            })?;
        Ok((preset.to_owned(), bake(&named.dimensions, registries)?))
    }
}
