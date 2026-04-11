use crate::world::block::Block;
use mcrs_core::tag::key::TagKey;
use mcrs_core::tag::registry::TagRegistry;
use mcrs_core::{ResourceLocation, StaticId, StaticRegistry};
use mcrs_vanilla::block::Block as VanillaBlock;
use std::sync::Arc;

pub type BlockTagSet = &'static [&'static BlockTag];

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum BlockTag {
    Tag(&'static Block),
    TagSet(BlockTagSet),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DynamicBlockTagSet {
    pub tag_key: TagKey<VanillaBlock, Arc<str>>,
}

impl DynamicBlockTagSet {
    pub fn new(s: &str) -> Self {
        let rl: ResourceLocation<Arc<str>> = ResourceLocation::parse(s)
            .unwrap_or_else(|_| panic!("invalid tag identifier: {s}"));
        Self {
            tag_key: TagKey::from_location(rl),
        }
    }

    pub fn from_static(s: &'static str) -> Self {
        let rl = ResourceLocation::new_static(s).to_arc();
        Self {
            tag_key: TagKey::from_location(rl),
        }
    }

    pub fn contains_block(
        &self,
        tag_registry: &TagRegistry<VanillaBlock>,
        id: StaticId<VanillaBlock>,
    ) -> bool {
        tag_registry.contains(&self.tag_key, id)
    }
}

pub trait BlockTagSetExt {
    fn contains_block(&self, block: &Block) -> bool;
}

impl BlockTagSetExt for BlockTag {
    fn contains_block(&self, block: &Block) -> bool {
        match self {
            BlockTag::Tag(b) => b == &block,
            BlockTag::TagSet(tag_set) => tag_set.contains_block(block),
        }
    }
}

impl BlockTagSetExt for BlockTagSet {
    fn contains_block(&self, block: &Block) -> bool {
        for tag in *self {
            if tag.contains_block(block) {
                return true;
            }
        }
        false
    }
}
