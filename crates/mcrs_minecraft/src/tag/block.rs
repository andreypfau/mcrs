use mcrs_core::ResourceLocation;
use mcrs_core::tag::key::TagKey;
use mcrs_core::tag::registry::DynTagRegistry;
use mcrs_vanilla::block::Block as VanillaBlock;
use std::sync::Arc;

pub type BlockTagSet = &'static [&'static BlockTag];

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum BlockTag {
    TagSet(BlockTagSet),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DynamicBlockTagSet {
    pub tag_key: TagKey<VanillaBlock, Arc<str>>,
}

impl DynamicBlockTagSet {
    pub fn new(s: &str) -> Self {
        let rl: ResourceLocation<Arc<str>> =
            ResourceLocation::parse(s).unwrap_or_else(|_| panic!("invalid tag identifier: {s}"));
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

    pub fn contains_block(&self, tag_registry: &DynTagRegistry<VanillaBlock>, id: u32) -> bool {
        tag_registry.contains(&self.tag_key, id)
    }
}

pub trait BlockTagSetExt {
    fn contains_name(&self, block: &str) -> bool;
}

impl BlockTagSetExt for BlockTag {
    fn contains_name(&self, block: &str) -> bool {
        match self {
            BlockTag::TagSet(tag_set) => tag_set.contains_name(block),
        }
    }
}

impl BlockTagSetExt for BlockTagSet {
    fn contains_name(&self, block: &str) -> bool {
        for tag in *self {
            if tag.contains_name(block) {
                return true;
            }
        }
        false
    }
}
