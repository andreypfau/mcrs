use crate::block::behaviour;
use crate::block::behaviour::LightSpec;
use crate::block::Block;

define_block! {
    name: "torch",
    protocol_id: 194,
    base_state_id: 3370,
    block_properties: behaviour::Properties::new()
        .no_collision()
        .instant_break()
        .with_light_emission(LightSpec::Const(14))
}
