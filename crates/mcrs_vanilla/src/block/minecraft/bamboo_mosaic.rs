use crate::block::Block;
use crate::block::behaviour;

// .sound(SoundType.BAMBOO_WOOD) - not implemented yet
define_block! {
    name: "bamboo_mosaic",
    protocol_id: 24,
    base_state_id: 28,
    block_properties: behaviour::Properties::new()
}
