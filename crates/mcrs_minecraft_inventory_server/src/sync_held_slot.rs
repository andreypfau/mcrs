use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::Changed;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHeldSlot;
use mcrs_minecraft_wire::VarInt;

use crate::sent_selected::SentSelected;

pub(crate) fn sync_held_slot(
    mut players: Query<(&Owner, &Selected, &mut SentSelected), Changed<Selected>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    for (&Owner(session), selected, mut sent) in &mut players {
        if !sent.set_if_neq(SentSelected(selected.0)) {
            continue;
        }
        packets.write(to(
            session,
            PacketPayload::SetHeldSlot(ClientboundSetHeldSlot {
                slot: VarInt(i32::from(selected.0)),
            }),
        ));
    }
}
