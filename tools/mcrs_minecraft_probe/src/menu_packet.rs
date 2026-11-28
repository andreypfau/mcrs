use anyhow::Context;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_protocol::{Decode, Packet};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundContainerClose, ClientboundContainerSetContent, ClientboundContainerSetData,
    ClientboundContainerSetSlot, ClientboundOpenScreen, ClientboundSetCursorItem,
    ClientboundSetHeldSlot, ClientboundSetPlayerInventory,
};
use mcrs_minecraft_protocol_game::item::ProtoStack;
use mcrs_minecraft_protocol_game::item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_text::Text;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuEvent {
    pub client: String,
    #[serde(flatten)]
    pub packet: MenuPacket,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MenuPacket {
    OpenScreen {
        container: i32,
        menu: MenuType,
        title: Text,
    },
    ContainerClose {
        container: i32,
    },
    ContainerSetContent {
        container: i32,
        state_id: i32,
        slots: Vec<Option<SlotStack>>,
        carried: Option<SlotStack>,
    },
    ContainerSetSlot {
        container: i32,
        state_id: i32,
        slot: i16,
        stack: Option<SlotStack>,
    },
    ContainerSetData {
        container: i32,
        id: i16,
        value: i16,
    },
    SetCursorItem {
        stack: Option<SlotStack>,
    },
    SetPlayerInventory {
        slot: i32,
        stack: Option<SlotStack>,
    },
    SetHeldSlot {
        slot: i32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotStack {
    pub item: Item,
    pub count: i32,
    pub components: PatchNames,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchNames {
    pub added: Vec<DataComponentType>,
    pub removed: Vec<DataComponentType>,
}

impl MenuPacket {
    pub(crate) fn read(id: i32, body: &[u8]) -> anyhow::Result<Option<Self>> {
        let packet = if id == ClientboundOpenScreen::ID {
            let packet: ClientboundOpenScreen = decode(body)?;
            MenuPacket::OpenScreen {
                container: packet.container_id.0,
                menu: packet.menu_type,
                title: packet.title.to_text()?,
            }
        } else if id == ClientboundContainerClose::ID {
            let packet: ClientboundContainerClose = decode(body)?;
            MenuPacket::ContainerClose {
                container: packet.container_id.0,
            }
        } else if id == ClientboundContainerSetContent::ID {
            let packet: ClientboundContainerSetContent = decode(body)?;
            MenuPacket::ContainerSetContent {
                container: packet.container_id.0,
                state_id: packet.state_seqno.0,
                slots: packet
                    .slot_data
                    .iter()
                    .map(SlotStack::from_wire)
                    .collect::<anyhow::Result<_>>()?,
                carried: SlotStack::from_wire(&packet.carried_item)?,
            }
        } else if id == ClientboundContainerSetSlot::ID {
            let packet: ClientboundContainerSetSlot = decode(body)?;
            MenuPacket::ContainerSetSlot {
                container: packet.container_id.0,
                state_id: packet.state_seqno.0,
                slot: packet.slot,
                stack: SlotStack::from_wire(&packet.item)?,
            }
        } else if id == ClientboundContainerSetData::ID {
            let packet: ClientboundContainerSetData = decode(body)?;
            MenuPacket::ContainerSetData {
                container: packet.container_id.0,
                id: packet.id,
                value: packet.value,
            }
        } else if id == ClientboundSetCursorItem::ID {
            let packet: ClientboundSetCursorItem = decode(body)?;
            MenuPacket::SetCursorItem {
                stack: SlotStack::from_wire(&packet.contents)?,
            }
        } else if id == ClientboundSetPlayerInventory::ID {
            let packet: ClientboundSetPlayerInventory = decode(body)?;
            MenuPacket::SetPlayerInventory {
                slot: packet.slot.0,
                stack: SlotStack::from_wire(&packet.contents)?,
            }
        } else if id == ClientboundSetHeldSlot::ID {
            let packet: ClientboundSetHeldSlot = decode(body)?;
            MenuPacket::SetHeldSlot {
                slot: packet.slot.0,
            }
        } else {
            return Ok(None);
        };
        Ok(Some(packet))
    }
}

impl SlotStack {
    fn from_wire(stack: &ProtoStack<Wire>) -> anyhow::Result<Option<Self>> {
        if stack.is_empty() {
            return Ok(None);
        }
        let number = stack.id.number();
        let item = Item::from_protocol_id(number)
            .with_context(|| format!("the item {number} is not in the item registry"))?;
        Ok(Some(SlotStack {
            item,
            count: stack.count,
            components: PatchNames {
                added: stack
                    .components
                    .added
                    .iter()
                    .map(|value| value.kind())
                    .collect(),
                removed: stack.components.removed.clone(),
            },
        }))
    }
}

fn decode<'a, P: Decode<'a>>(mut body: &'a [u8]) -> anyhow::Result<P> {
    Ok(P::decode(&mut body)?)
}
