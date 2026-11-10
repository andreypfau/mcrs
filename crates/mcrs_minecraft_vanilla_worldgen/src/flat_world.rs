use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_game_registries::worldgen::chunk_generator::FlatChunkGenerator;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocation, rl};
use mcrs_minecraft_worldgen_generator::flat::{FlatLayer, FlatLayers};
use tracing::warn;

const DEFAULT_LAYERS: [(&str, u32); 3] = [
    ("minecraft:bedrock", 1),
    ("minecraft:dirt", 2),
    ("minecraft:grass_block", 1),
];
const DEFAULT_BIOME: ResourceLocation<'static> = rl!("minecraft:plains");

/// The layers of a flat generator. Settings without a readable layer list give the default
/// superflat layers: bedrock, two dirt and a grass block.
pub(crate) fn flat_layers(generator: &FlatChunkGenerator, blocks: &BlockDefinitions) -> FlatLayers {
    let read = settings_layers(&generator.settings, blocks);
    let layers = read.unwrap_or_else(|| {
        DEFAULT_LAYERS
            .iter()
            .filter_map(|&(block, height)| layer(blocks, block, height))
            .collect()
    });
    FlatLayers::new(layers)
}

/// The biome of every column of a flat generator: the one its settings name, or plains.
pub(crate) fn flat_biome(
    generator: &FlatChunkGenerator,
    registries: &Registries,
) -> Option<Id<Biome>> {
    let biomes = registries.registry::<BiomeFile>()?;
    let named = compound_string(&generator.settings, "biome")
        .and_then(|name| ResourceLocation::read(name).ok())
        .and_then(|name| biomes.by_name(name));
    named.or_else(|| biomes.by_name(DEFAULT_BIOME))
}

fn settings_layers(settings: &NbtTag, blocks: &BlockDefinitions) -> Option<Vec<FlatLayer>> {
    let NbtTag::Compound(settings) = settings else {
        return None;
    };
    let list = settings.get_list("layers")?;
    let mut layers = Vec::with_capacity(list.len());
    for entry in list {
        let NbtTag::Compound(entry) = entry else {
            return None;
        };
        let block = entry.get_string("block")?;
        let height = u32::try_from(entry.get_int("height")?).ok()?;
        let Some(read) = layer(blocks, block, height) else {
            warn!(
                block,
                "a flat layer names no block; the default layers apply"
            );
            return None;
        };
        layers.push(read);
    }
    Some(layers)
}

fn layer(blocks: &BlockDefinitions, block: &str, height: u32) -> Option<FlatLayer> {
    let state = blocks.default_state(block)?;
    Some(FlatLayer { state, height })
}

fn compound_string<'a>(tag: &'a NbtTag, name: &str) -> Option<&'a str> {
    let NbtTag::Compound(compound) = tag else {
        return None;
    };
    compound.get_string(name)
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, TaskPoolPlugin};
    use bevy_asset::{AssetPlugin, AssetServer};
    use mcrs_minecraft_block::definition::load_block_definitions;
    use mcrs_minecraft_nbt::compound::NbtCompound;

    use super::*;

    fn layer_tag(block: &str, height: i32) -> NbtTag {
        let mut layer = NbtCompound::new();
        layer.put("block", NbtTag::String(block.to_owned()));
        layer.put("height", NbtTag::Int(height));
        NbtTag::Compound(layer)
    }

    fn settings(layers: Vec<NbtTag>) -> FlatChunkGenerator {
        let mut settings = NbtCompound::new();
        settings.put("layers", NbtTag::List(layers));
        FlatChunkGenerator {
            settings: NbtTag::Compound(settings),
        }
    }

    #[test]
    fn a_layer_list_naming_an_unknown_block_gives_the_default_layers() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        let blocks = load_block_definitions(&asset_server)
            .expect("the corpus loads")
            .0;

        let read = flat_layers(&settings(vec![layer_tag("minecraft:stone", 3)]), &blocks);
        let refused = flat_layers(
            &settings(vec![
                layer_tag("minecraft:stone", 3),
                layer_tag("test:no_such_block", 1),
            ]),
            &blocks,
        );
        let default = FlatLayers::new(
            DEFAULT_LAYERS
                .iter()
                .filter_map(|&(block, height)| layer(&blocks, block, height))
                .collect(),
        );
        assert_ne!(read, default);
        assert_eq!(refused, default);
    }
}
