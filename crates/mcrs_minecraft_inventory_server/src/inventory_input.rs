use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_inventory_click::Click;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_item_stack::ItemStack;

use crate::ClaimedSlot;
use mcrs_minecraft_vanilla_inventory::HandAction;

#[derive(Message, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InventoryInput {
    Click {
        player: Entity,
        container_id: i32,
        state_id: i32,
        click: Click,
        changed: Vec<ClaimedSlot>,
        carried: SlotView,
    },
    Close {
        player: Entity,
    },
    Hand {
        player: Entity,
        action: HandAction,
    },
    CreativeSet {
        player: Entity,
        slot: i16,
        stack: Option<ItemStack>,
    },
    SelectSlot {
        player: Entity,
        slot: u16,
    },
}
