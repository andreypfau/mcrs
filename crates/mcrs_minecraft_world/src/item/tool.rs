use mcrs_minecraft_item::Tool;
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_registry::{Id, Tags};

pub fn mining_speed(tool: &Tool, block: Id<Block>, tags: &Tags<Block>) -> f32 {
    tool.rules
        .iter()
        .find(|rule| rule.speed.is_some() && rule.blocks.contains(block, tags))
        .and_then(|rule| rule.speed)
        .unwrap_or(tool.default_mining_speed)
}

pub fn is_correct_for_drops(tool: &Tool, block: Id<Block>, tags: &Tags<Block>) -> bool {
    tool.rules
        .iter()
        .find(|rule| rule.correct_for_drops.is_some() && rule.blocks.contains(block, tags))
        .and_then(|rule| rule.correct_for_drops)
        .unwrap_or(false)
}
