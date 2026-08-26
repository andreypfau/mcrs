use crate::block::Block;
use crate::block::behaviour;

// .sound(SoundType.BAMBOO_WOOD) - not implemented yet
define_block! {
    name: "bamboo_planks",
    protocol_id: 23,
    base_state_id: 27,
    block_properties: behaviour::Properties::new()
}
