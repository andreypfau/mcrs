use crate::world::inventory::PlayerHotbarSlots;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::On;
use bevy_ecs::system::Query;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::serverbound::ServerboundSetCarriedItem;

pub struct PlayerInventoryPlugin;

impl Plugin for PlayerInventoryPlugin {
    fn build(&self, app: &mut App) {}
}

fn update_carried_item(event: On<ReceivedPacketEvent>, mut hotbar: Query<&mut PlayerHotbarSlots>) {
    let Ok(mut hotbar) = hotbar.get_mut(event.entity) else {
        return;
    };
    let Some(pkt) = event.decode::<ServerboundSetCarriedItem>() else {
        return;
    };
    if pkt.slot > 8 {
        eprintln!("Invalid carried item slot: {}", pkt.slot);
        return;
    }
    hotbar.selected = pkt.slot as u8
}
