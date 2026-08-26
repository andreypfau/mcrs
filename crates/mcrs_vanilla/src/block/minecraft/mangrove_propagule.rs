use crate::block::Block;
use crate::block::behaviour;
use crate::block::state_properties;

define_block! {
    name: "mangrove_propagule",
    protocol_id: 33,
    base_state_id: 45,
    properties: [&state_properties::AGE_4, &state_properties::HANGING, &state_properties::STAGE, &state_properties::WATERLOGGED],
    default: { age: 0, hanging: false, stage: 0, waterlogged: false },
    block_properties: behaviour::Properties::new()
}
