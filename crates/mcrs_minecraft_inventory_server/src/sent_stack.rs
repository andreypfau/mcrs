use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Decode;

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

    fn unsendable(stack: ItemStack) -> Self {
        Self {
            wire: ProtoStack::default(),
            stack: Some(stack),
        }
    }

    pub(crate) fn of(
        stack: Option<ItemStack>,
        names: &RegistryNames<'_>,
        bytes: &mut Vec<u8>,
    ) -> Self {
        let Some(stack) = stack else {
            return Self::empty();
        };
        bytes.clear();
        if let Err(error) = write_wire(Some(&stack), names, bytes) {
            tracing::warn!("a stack of a player has no wire form: {error:?}");
            return Self::unsendable(stack);
        }
        match ProtoStack::decode(&mut bytes.as_slice()) {
            Ok(wire) => Self {
                wire,
                stack: Some(stack),
            },
            Err(error) => {
                tracing::warn!("a stack of a player does not decode from its wire form: {error:?}");
                Self::unsendable(stack)
            }
        }
    }
}
