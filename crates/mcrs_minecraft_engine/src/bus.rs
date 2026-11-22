use crate::persisted_registry::PersistedSnapshot;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageWriter};
use bytes::{BufMut, Bytes, BytesMut};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::event::Trigger;
use mcrs_minecraft_network::metrics::PacketKind;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Encode, Packet};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockChangedAck, ClientboundBlockDestruction, ClientboundBlockEvent,
    ClientboundBlockUpdate, ClientboundChunkBatchFinished, ClientboundChunkBatchStart,
    ClientboundChunkCacheRadius, ClientboundCommandSuggestions, ClientboundContainerClose,
    ClientboundContainerSetContent, ClientboundContainerSetSlot, ClientboundEntityEvent,
    ClientboundEntityPositionSync, ClientboundForgetLevelChunk, ClientboundGameEvent,
    ClientboundHurtAnimation, ClientboundLevelEvent, ClientboundLightUpdate, ClientboundLogin,
    ClientboundOpenScreen, ClientboundPlayerAbilities, ClientboundPlayerCombatKill,
    ClientboundPlayerLookAt, ClientboundPlayerPosition, ClientboundRespawn,
    ClientboundSectionBlocksUpdate, ClientboundSetChunkCacheCenter, ClientboundSetCursorItem,
    ClientboundSetHealth, ClientboundSetHeldSlot, ClientboundSetTime, ClientboundSystemChatPacket,
    ClientboundTabList,
};
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_text::Text;
use mcrs_voxel::ColumnPos;
use smallvec::SmallVec;
use std::sync::Arc;

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerPacket {
    pub target: PacketTarget,
    pub data: PacketPayload,
    pub session: PlayerSession,
    pub epoch: u32,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerPacket {
    pub session: PlayerSession,
    pub id: i32,
    pub data: Bytes,
    pub trigger: Option<Trigger>,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerSpawn {
    pub session: PlayerSession,
    pub snapshot: PersistedSnapshot,
    pub dimensions: Arc<[ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>]>,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerAttached {
    pub session: PlayerSession,
}

#[derive(Message, Clone, Debug)]
pub struct InboundPlayerDespawn {
    pub session: PlayerSession,
}

#[derive(Message, Clone, Debug)]
pub struct OutboundPlayerReleased {
    pub session: PlayerSession,
}

#[derive(Clone, Debug)]
pub enum PacketTarget {
    SinglePlayer(PlayerSession),
    AllInDim(Entity),
    AllPlayers,
    PlayerSet(SmallVec<[PlayerSession; 8]>),
}

pub fn to(session: PlayerSession, data: PacketPayload) -> OutboundPlayerPacket {
    to_target(PacketTarget::SinglePlayer(session), data)
}

pub(crate) fn send_own_entity_state(
    writer: &mut MessageWriter<OutboundPlayerPacket>,
    session: PlayerSession,
    packet: &(impl Packet + Encode),
    subject: &str,
) {
    let mut bytes = BytesMut::new().writer();
    match packet.encode_with_id(&mut bytes) {
        Ok(()) => {
            let packet = bytes.into_inner().freeze();
            writer.write(to(session, PacketPayload::OwnEntityState(packet)));
        }
        Err(error) => {
            tracing::warn!(
                ?session,
                "the {subject} of a player did not encode: {error}"
            );
        }
    }
}

pub fn to_target(target: PacketTarget, data: PacketPayload) -> OutboundPlayerPacket {
    OutboundPlayerPacket {
        target,
        data,
        session: PlayerSession(0),
        epoch: 0,
    }
}

#[derive(Clone, Debug)]
pub enum PacketPayload {
    LightUpdate(ClientboundLightUpdate<'static>),
    BlockUpdate(ClientboundBlockUpdate),
    SectionBlocksUpdate(ClientboundSectionBlocksUpdate<'static>),
    BlockEvent(ClientboundBlockEvent),
    LevelEvent(ClientboundLevelEvent),
    BlockChangedAck(ClientboundBlockChangedAck),
    BlockDestruction(ClientboundBlockDestruction),
    Disconnect(Text),
    ChunkLoad { column: ColumnPos, packet: Bytes },
    ChunkUnload(ClientboundForgetLevelChunk),
    ChunkBatchStart(ClientboundChunkBatchStart),
    ChunkBatchFinished(ClientboundChunkBatchFinished),
    Replication(Vec<Bytes>),
    EntityPosSync(ClientboundEntityPositionSync),
    PlayerLogin(ClientboundLogin),
    Respawn(ClientboundRespawn),
    OpLevelEntityEvent(ClientboundEntityEvent),
    SetChunkCacheCenter(ClientboundSetChunkCacheCenter),
    SetChunkCacheRadius(ClientboundChunkCacheRadius),
    PlayerInfoUpdate { entries: Vec<PlayerInfoEntry> },
    TabList(ClientboundTabList<'static>),
    PlayerPosition(ClientboundPlayerPosition),
    PlayerLookAt(ClientboundPlayerLookAt),
    PlayerAbilities(ClientboundPlayerAbilities),
    OwnEntityState(Bytes),
    SystemChat(ClientboundSystemChatPacket<'static>),
    GameEvent(ClientboundGameEvent),
    Commands(Bytes),
    CommandSuggestions(ClientboundCommandSuggestions<'static>),
    SetTime(ClientboundSetTime),
    SetHealth(ClientboundSetHealth),
    HurtAnimation(ClientboundHurtAnimation),
    PlayerCombatKill(ClientboundPlayerCombatKill<'static>),
    SetHeldSlot(ClientboundSetHeldSlot),
    ContainerSetContent(ClientboundContainerSetContent),
    ContainerSetSlot(ClientboundContainerSetSlot),
    SetCursorItem(ClientboundSetCursorItem),
    OpenScreen(ClientboundOpenScreen<'static>),
    ContainerClose(ClientboundContainerClose),
}

impl PacketPayload {
    pub fn kind(&self) -> PacketKind {
        match self {
            Self::EntityPosSync(_) => PacketKind::EntityMove,
            Self::Replication(_) => PacketKind::Replication,
            Self::PlayerInfoUpdate { .. } | Self::TabList(_) => PacketKind::PlayerInfo,
            Self::ChunkLoad { .. }
            | Self::ChunkUnload(_)
            | Self::ChunkBatchStart(_)
            | Self::ChunkBatchFinished(_)
            | Self::LightUpdate(_) => PacketKind::Chunk,
            _ => PacketKind::Other,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerInfoEntry {
    pub player_uuid: Uuid,
    pub username: String,
    pub game_mode: GameMode,
    pub listed: bool,
}
