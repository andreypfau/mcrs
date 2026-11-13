use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_inventory_click::Click;
use mcrs_minecraft_inventory_view::SlotView;

use crate::ClaimedSlot;

/// One `container_click` packet of a player.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct InventoryClick {
    pub player: Entity,
    pub container_id: i32,
    /// The last state id that the client received for the menu.
    pub state_id: i32,
    pub click: Click,
    /// What the client holds after the click for each entry that it lists as changed: a hashed
    /// stack, or [`SlotView::Unknown`] when the packet gives no hashed stack that the server can
    /// read.
    pub changed: Vec<ClaimedSlot>,
    /// What the client holds in the cursor after the click.
    pub carried: SlotView,
}
