use alloc::vec::Vec;

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query};
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_damage::Died;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerCombatKill;
use mcrs_minecraft_text::{RawText, Text};

use crate::DeathMessage;

pub(crate) fn send_kill(
    mut died: MessageReader<Died>,
    mut described: MessageReader<DeathMessage>,
    players: Query<&Owner, With<Player>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut recent: Local<Vec<DeathMessage>>,
) {
    recent.clear();
    recent.extend(described.read().cloned());
    for death in died.read() {
        let Ok(&Owner(session)) = players.get(death.target) else {
            continue;
        };
        let text = recent
            .iter()
            .rev()
            .find(|message| message.player == death.target)
            .map(|message| RawText::from_text(&message.text));
        let Some(message) = text.and_then(Result::ok).or_else(empty_text) else {
            continue;
        };
        writer.write(bus::to(
            session,
            PacketPayload::PlayerCombatKill(ClientboundPlayerCombatKill {
                player_id: VarInt(player_network_id(session)),
                message,
            }),
        ));
    }
}

fn empty_text() -> Option<RawText<'static>> {
    let empty: Text = Text::default();
    RawText::from_text(&empty).ok()
}
