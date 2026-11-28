use crate::ability::PlayerOpLevel;
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::op_level::op_level_packet;
use crate::server_edit::{ticking_state, ticking_step};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_protocol_game::GameEventKind;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundGameEvent, ClientboundPlayerAbilities, ClientboundPlayerPosition,
};

pub(crate) fn enter_level_packets(
    session: PlayerSession,
    op_level: PlayerOpLevel,
    position: ClientboundPlayerPosition,
    abilities: ClientboundPlayerAbilities,
    pace: Option<&TickPace>,
    mut push: impl FnMut(OutboundPlayerPacket),
) {
    push(op_level_packet(session, op_level));
    push(to(session, PacketPayload::PlayerPosition(position)));
    push(to(session, PacketPayload::PlayerAbilities(abilities)));
    if let Some(pace) = pace {
        push(to(session, ticking_state(pace)));
        push(to(session, ticking_step(pace.steps())));
    }
    push(to(
        session,
        PacketPayload::GameEvent(ClientboundGameEvent {
            game_event: GameEventKind::LevelChunksLoadStart,
        }),
    ));
}
