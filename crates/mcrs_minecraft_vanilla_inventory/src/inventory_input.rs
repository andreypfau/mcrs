use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_inventory_click::Click;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_stack::ItemStack;

use crate::ClaimedSlot;
use crate::HandAction;

/// One inventory packet of a player, decoded. The inputs apply in the order that they arrive
/// (ITX-JAV-003).
#[derive(Message, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InventoryInput {
    /// A `container_click` packet.
    Click {
        player: Entity,
        container_id: i32,
        /// The last state id that the client received for the menu.
        state_id: i32,
        click: Click,
        /// What the client holds after the click for each entry that it lists as changed: a
        /// hashed stack, or [`SlotView::Unknown`] when the packet gives no hashed stack that the
        /// server can read.
        changed: Vec<ClaimedSlot>,
        /// What the client holds in the cursor after the click.
        carried: SlotView,
    },
    /// A `container_close` packet, whatever its container id.
    Close { player: Entity },
    /// A drop key or the hand swap of a `player_action` packet.
    Hand { player: Entity, action: HandAction },
    /// A `set_creative_mode_slot` packet whose stack decodes.
    CreativeSet {
        player: Entity,
        /// The entry of the inventory menu, or a negative value for a drop.
        slot: i16,
        stack: Option<ItemStack>,
    },
    /// A `set_carried_item` packet.
    SelectSlot { player: Entity, slot: u16 },
}
