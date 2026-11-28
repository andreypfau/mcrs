use bevy_ecs::prelude::*;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine_world::session::PlayerSession;

use crate::console_plugin::Console;

pub fn log_console_answers(world: &mut World) {
    if !world.contains_resource::<Console>() {
        return;
    }
    if let Err(error) = world.run_system_cached(log_answers) {
        tracing::error!(%error, "the console answers were not written");
    }
}

fn log_answers(mut packets: MessageReader<OutboundPlayerPacket>, console: Res<Console>) {
    for packet in packets.read() {
        let (PacketTarget::SinglePlayer(PlayerSession::SERVER), PacketPayload::SystemChat(chat)) =
            (&packet.target, &packet.data)
        else {
            continue;
        };
        console.log.push(chat.content.clone());
    }
}
