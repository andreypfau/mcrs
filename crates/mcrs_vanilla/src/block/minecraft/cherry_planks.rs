use crate::block::Block;
use crate::block::behaviour;

// .sound(SoundType.CHERRY_WOOD) - not implemented yet
define_block! {
    name: "cherry_planks",
    protocol_id: 18,
    base_state_id: 20,
    block_properties: behaviour::Properties::new()
}
