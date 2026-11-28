use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Decode;

pub(crate) fn wire_stack_of(
    stacks: &mut Stacks,
    table: &ItemTable,
    stack: Entity,
    bytes: &mut Vec<u8>,
) -> Option<ProtoStack<Wire>> {
    stacks
        .to_value(stack)
        .and_then(|value| wire_stack(&value, table, bytes))
}

fn wire_stack(
    stack: &ItemStack,
    table: &ItemTable,
    bytes: &mut Vec<u8>,
) -> Option<ProtoStack<Wire>> {
    bytes.clear();
    if let Err(error) = write_wire(Some(stack), table, bytes) {
        tracing::warn!("the stack of an item entity has no wire form: {error:?}");
        return None;
    }
    ProtoStack::decode(&mut bytes.as_slice()).ok()
}
