use crate::structures::index::EndBiomes;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_core::TagId;
use mcrs_minecraft_registries::{Diagnostics, Registries, Tags};
use mcrs_minecraft_registries::{require_id, require_tags};

pub struct FillIds {
    pub end: EndBiomes,
    pub uncarvable: Option<TagId<Block>>,
    pub block_tags: Tags<Block>,
}

impl FillIds {
    pub fn resolve(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Self> {
        let end = EndBiomes::from_lookup(|key| require_id(registries, key, diagnostics));
        let block_tags = require_tags::<Block>(registries, diagnostics)?;
        Some(Self {
            end: end?,
            uncarvable: block_tags.id(&block_tags::UNCARVABLE),
            block_tags,
        })
    }
}
