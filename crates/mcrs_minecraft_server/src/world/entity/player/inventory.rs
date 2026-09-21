use bevy_app::{App, Plugin};
use bevy_ecs::prelude::On;
use bevy_ecs::system::Query;
use mcrs_minecraft_item::SelectedHotbarSlot;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundSetCarriedItem;
use tracing::warn;

pub struct PlayerInventoryPlugin;

impl Plugin for PlayerInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_carried_item);
    }
}

fn update_carried_item(
    event: On<ReceivedPacketEvent>,
    mut selected: Query<&mut SelectedHotbarSlot>,
) {
    let Ok(mut selected) = selected.get_mut(event.entity) else {
        return;
    };
    let Some(pkt) = event.decode::<ServerboundSetCarriedItem>() else {
        return;
    };
    if pkt.slot > 8 {
        warn!("Invalid carried item slot: {}", pkt.slot);
        return;
    }
    selected.0 = pkt.slot as u8;
}
