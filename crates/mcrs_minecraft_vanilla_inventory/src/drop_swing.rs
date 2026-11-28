use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::Res;
use mcrs_minecraft_engine::swing::SwingPlayer;
use mcrs_minecraft_engine_item::RequestDrop;

use crate::RequestPurpose;
use crate::VanillaInventoryRequests;

pub(crate) fn swing_on_hand_drop(
    mut drops: MessageReader<RequestDrop>,
    requests: Option<Res<VanillaInventoryRequests>>,
    mut swings: MessageWriter<SwingPlayer>,
) {
    let Some(requests) = requests else {
        return;
    };
    for drop in drops.read() {
        if drop.id.source != requests.source || drop.id.serial != RequestPurpose::HandDrop.serial()
        {
            continue;
        }
        swings.write(SwingPlayer::default_main(drop.holder));
    }
}
