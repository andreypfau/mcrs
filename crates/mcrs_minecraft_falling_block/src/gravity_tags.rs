use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_resource::TagKey;
use mcrs_minecraft_resource::rl;

/// The blocks that fall when the block below them is free (FBK-TRG-002).
pub const GRAVITY_BLOCKS: TagKey<'static, Block> = TagKey::new(rl!("mcrs:gravity_blocks"));
/// The blocks whose falling block breaks at its landing and makes no block (FBK-TRG-002).
pub const BREAKS_AFTER_FALL: TagKey<'static, Block> = TagKey::new(rl!("mcrs:breaks_after_fall"));
