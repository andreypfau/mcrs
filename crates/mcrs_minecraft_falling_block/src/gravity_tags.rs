use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_resource::TagKey;
use mcrs_minecraft_resource::rl;

pub const GRAVITY_BLOCKS: TagKey<'static, Block> = TagKey::new(rl!("mcrs:gravity_blocks"));
pub const BREAKS_AFTER_FALL: TagKey<'static, Block> = TagKey::new(rl!("mcrs:breaks_after_fall"));
