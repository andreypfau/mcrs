use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Decode;

/// The wire form of the stack `stack`, which the spawn and the stack data of an item entity carry.
pub(crate) fn wire_stack_of(
    stacks: &mut Stacks,
    table: &ItemTable,
    stack: Entity,
) -> Option<ProtoStack<Wire>> {
    stacks
        .to_value(stack)
        .and_then(|value| wire_stack(&value, table))
}

// chisle: the two stack models meet through the bytes of the wire; a direct conversion when
// the protocol holds `ItemStack`.
fn wire_stack(stack: &ItemStack, table: &ItemTable) -> Option<ProtoStack<Wire>> {
    let mut bytes = Vec::new();
    if let Err(error) = write_wire(Some(stack), table, &mut bytes) {
        tracing::warn!("the stack of an item entity has no wire form: {error:?}");
        return None;
    }
    ProtoStack::decode(&mut bytes.as_slice()).ok()
}
