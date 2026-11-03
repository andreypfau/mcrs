use crate::ability::PlayerOpLevel;
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::op_level::op_level_packet;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::GameEventKind;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundGameEvent, ClientboundPlayerAbilities, ClientboundPlayerPosition,
    ClientboundSetChunkCacheCenter,
};
use mcrs_voxel::ColumnPos;

/// The packets that put a player into the level of its world, after the `login` of a join or
/// the `respawn` of a dimension change: with `position` as the destination of the move.
// chisle: steps 2, 8, 9, 10, 13 to 16 and 18 are not sent, because the server holds no
// difficulty, world border, clock packet, spawn position, tick rate packet, inventory sync or post
// effects yet; each step joins this sequence with its state.
pub(crate) fn enter_level_packets(
    session: PlayerSession,
    op_level: PlayerOpLevel,
    position: ClientboundPlayerPosition,
    abilities: ClientboundPlayerAbilities,
    mut push: impl FnMut(OutboundPlayerPacket),
) {
    let column = ColumnPos::from(position.position);
    push(op_level_packet(session, op_level));
    push(to(session, PacketPayload::PlayerPosition(position)).critical());
    push(
        to(
            session,
            PacketPayload::SetChunkCacheCenter(ClientboundSetChunkCacheCenter {
                x: VarInt(column.x),
                z: VarInt(column.z),
            }),
        )
        .critical(),
    );
    push(to(session, PacketPayload::PlayerAbilities(abilities)).critical());
    push(
        to(
            session,
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::LevelChunksLoadStart,
            }),
        )
        .critical(),
    );
}
