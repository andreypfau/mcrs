use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Decode;

/// The wire form of a stack and the stack that it gives the client.
pub(crate) struct SentStack {
    pub(crate) wire: ProtoStack<Wire>,
    pub(crate) stack: Option<ItemStack>,
}

impl SentStack {
    fn empty() -> Self {
        Self {
            wire: ProtoStack::default(),
            stack: None,
        }
    }

    /// A stack without a wire form goes as the empty stack.
    // chisle: the two stack models meet through the bytes of the wire; a direct conversion
    // lifts it when the protocol holds `ItemStack`.
    pub(crate) fn of(stack: Option<ItemStack>, table: &ItemTable, bytes: &mut Vec<u8>) -> Self {
        let Some(stack) = stack else {
            return Self::empty();
        };
        bytes.clear();
        if let Err(error) = write_wire(Some(&stack), table, bytes) {
            tracing::warn!("a stack of a player has no wire form: {error:?}");
            return Self::empty();
        }
        match ProtoStack::decode(&mut bytes.as_slice()) {
            Ok(wire) => Self {
                wire,
                stack: Some(stack),
            },
            Err(error) => {
                tracing::warn!("a stack of a player does not decode from its wire form: {error:?}");
                Self::empty()
            }
        }
    }
}
