//! The header of the player list, which shows the number of players online.

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to_target};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Local, Query};
use mcrs_minecraft_engine_world::session::{PlayerSession, Session, SessionPlacement};
use mcrs_minecraft_protocol_game::clientbound::ClientboundTabList;
use mcrs_minecraft_text::{RawText, Text};
use smallvec::SmallVec;

/// Host system: sends every player the number of players online when it changes, and each
/// player that a dimension attaches the number that it does not have yet.
pub fn send_online_count(
    placements: Query<&SessionPlacement>,
    placed: Query<(&Session, &SessionPlacement), Changed<SessionPlacement>>,
    mut sent: Local<Option<usize>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let online = placements
        .iter()
        .filter(|placement| placement.place().attached().is_some())
        .count();
    let target = if *sent == Some(online) {
        let attached: SmallVec<[PlayerSession; 8]> = placed
            .iter()
            .filter(|(_, placement)| placement.place().attached().is_some())
            .map(|(&Session(session), _)| session)
            .collect();
        if attached.is_empty() {
            return;
        }
        PacketTarget::PlayerSet(attached)
    } else {
        PacketTarget::AllPlayers
    };
    let Some(packet) = tab_list(online) else {
        return;
    };
    *sent = Some(online);
    writer.write(to_target(target, PacketPayload::TabList(packet)));
}

fn tab_list(online: usize) -> Option<ClientboundTabList<'static>> {
    let header: Text = Text::from(format!("{online} players online"));
    let footer: Text = Text::from("");
    Some(ClientboundTabList {
        header: RawText::from_text(&header).ok()?,
        footer: RawText::from_text(&footer).ok()?,
    })
}
