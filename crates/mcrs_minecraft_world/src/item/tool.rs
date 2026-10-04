use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::{HolderSet, ResourceKey};
use mcrs_minecraft_item::Tool;
use mcrs_minecraft_registry::BlockReg;
use mcrs_minecraft_registry::key::Block;

pub fn mining_speed(
    tool: &Tool,
    block: &str,
    blocks: &BlockDefinitions,
    tags: &DynTagRegistry<Block>,
) -> f32 {
    tool.rules
        .iter()
        .find(|rule| rule.speed.is_some() && contains(&rule.blocks, block, blocks, tags))
        .and_then(|rule| rule.speed)
        .unwrap_or(tool.default_mining_speed)
}

pub fn is_correct_for_drops(
    tool: &Tool,
    block: &str,
    blocks: &BlockDefinitions,
    tags: &DynTagRegistry<Block>,
) -> bool {
    tool.rules
        .iter()
        .find(|rule| {
            rule.correct_for_drops.is_some() && contains(&rule.blocks, block, blocks, tags)
        })
        .and_then(|rule| rule.correct_for_drops)
        .unwrap_or(false)
}

fn contains(
    set: &HolderSet<ResourceKey<BlockReg>>,
    block: &str,
    blocks: &BlockDefinitions,
    tags: &DynTagRegistry<Block>,
) -> bool {
    match set {
        HolderSet::Tag(tag) => blocks
            .index_of(block)
            .is_some_and(|id| tags.contains(&TagKey::<Block, _>::from_location(tag.clone()), id)),
        _ => set.entries().iter().any(|key| key.as_str() == block),
    }
}
