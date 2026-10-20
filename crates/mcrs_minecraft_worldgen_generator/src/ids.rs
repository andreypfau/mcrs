use crate::structures::index::EndBiomes;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_core::TagId;
use mcrs_minecraft_registries::{Diagnostics, Registries, RegistryOf, Tags};
use mcrs_minecraft_registries::{require_id, require_registry, require_tag, require_tags};

pub struct FillIds {
    pub end: EndBiomes,
    pub uncarvable: TagId<Block>,
    pub block_tags: Tags<Block>,
}

impl FillIds {
    pub fn resolve(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Self> {
        let biomes = require_registry::<_, RegistryOf<BiomeFile>>(
            registries,
            mcrs_minecraft_biome::keys::BIOME,
            diagnostics,
        );
        let end = biomes
            .and_then(|biomes| EndBiomes::from_lookup(|key| require_id(biomes, key, diagnostics)));
        let tags = require_tags(registries, mcrs_minecraft_block::keys::BLOCK, diagnostics);
        let uncarvable = tags.as_ref().and_then(|tags| {
            require_tag(
                tags,
                mcrs_minecraft_block::keys::BLOCK,
                &block_tags::UNCARVABLE,
                diagnostics,
            )
        });
        Some(Self {
            end: end?,
            uncarvable: uncarvable?,
            block_tags: tags?,
        })
    }
}
