pub mod clientbound {
    use crate::chunk::ChunkBlockUpdateEntry;
    use crate::entity::minecart::MinecartStep;
    use crate::entity::player::*;
    use crate::game_event::GameEventKind;
    use crate::packets::common::clientbound::KeepAlive;
    use crate::profile::{PlayerListActions, PlayerListEntry};
    use crate::{ChunkColumnPos, Look, PositionFlag, Slot, VarInt};
    use bevy_math::DVec3;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_protocol::{BlockStateId, ByteAngle};
    use mcrs_protocol_macros::{Decode, Encode, Packet};
    use std::borrow::Cow;
    use std::io::Write;
    use uuid::Uuid;
    use valence_ident::Ident;
    use valence_text::Text;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x01, state=Game)]
    pub struct ClientboundAddEntity {
        pub id: VarInt,
        pub uuid: Uuid,
        pub kind: VarInt,
        pub pos: DVec3,
        pub velocity: VarInt,
        pub yaw: ByteAngle,
        pub pitch: ByteAngle,
        pub head_yaw: ByteAngle,
        pub data: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x05, state=Game)]
    pub struct ClientboundBlockDestruction {
        pub id: VarInt,
        pub pos: BlockPos,
        pub progress: i8,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x08, state=Game)]
    pub struct ClientboundBlockUpdate {
        pub block_pos: BlockPos,
        pub block_state_id: BlockStateId,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x12, state=Game)]
    pub struct ClientboundContainerSetContent {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot_data: Vec<Slot>,
        pub carried_item: Slot,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x20, state=Game)]
    pub struct ClientboundDisconnect {
        pub reason: Text,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x23, state=Game)]
    pub struct ClientboundEntityPositionSync {
        pub entity_id: VarInt,
        pub position: DVec3,
        pub velocity: DVec3,
        pub look: Look,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x25, state=Game)]
    pub struct ClientboundForgetLevelChunk {
        pub z: i32,
        pub x: i32,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x26, state=Game)]
    pub struct ClientboundGameEvent {
        pub game_event: GameEventKind,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x2B, state=Game)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x2C, state=Game)]
    pub struct ClientboundLevelChunkWithLight<'a> {
        pub pos: ChunkColumnPos,
        pub chunk_data: crate::chunk::ChunkData<'a>,
        pub light_data: crate::chunk::LightData<'a>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x30, state=Game)]
    pub struct ClientboundLogin<'a> {
        pub player_id: i32,
        pub hardcore: bool,
        pub dimensions: Vec<Ident<Cow<'a, str>>>,
        pub max_players: VarInt,
        pub chunk_radius: VarInt,
        pub simulation_distance: VarInt,
        pub reduced_debug_info: bool,
        pub show_death_screen: bool,
        pub do_limited_crafting: bool,
        pub player_spawn_info: PlayerSpawnInfo<'a>,
        pub enforces_secure_chat: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x33, state=Game)]
    pub struct ClientboundMoveEntityPos {
        pub entity_id: VarInt,
        pub delta: [i16; 3],
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x34, state=Game)]
    pub struct ClientboundMoveEntityPosRot {
        pub entity_id: VarInt,
        pub delta: [i16; 3],
        pub y_rot: ByteAngle,
        pub x_rot: ByteAngle,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x35, state=Game)]
    pub struct ClientboundMoveMinecartAlongTrack {
        pub entity_id: VarInt,
        pub lerp_steps: Vec<MinecartStep>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x36, state=Game)]
    pub struct ClientboundMoveEntityRot {
        pub entity_id: VarInt,
        pub y_rot: ByteAngle,
        pub x_rot: ByteAngle,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, Packet)]
    #[packet(id=0x44, state=Game)]
    pub struct ClientboundPlayerInfoUpdate<'a> {
        pub actions: PlayerListActions,
        pub entries: Cow<'a, [PlayerListEntry<'a>]>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x46, state=Game)]
    pub struct ClientboundPlayerPosition {
        pub teleport_id: VarInt,
        pub position: DVec3,
        pub velocity: DVec3,
        pub look: Look,
        pub flags: Vec<PositionFlag>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x51, state=Game)]
    pub struct ClientboundRotateHead {
        pub entity_id: VarInt,
        pub y_head_rot: ByteAngle,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x52, state=Game)]
    pub struct ClientboundSectionBlocksUpdate<'a> {
        pub chunk_pos: ChunkPos,
        pub blocks: Cow<'a, [ChunkBlockUpdateEntry]>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x5C, state=Game)]
    pub struct ClientboundSetChunkCacheCenter {
        pub x: VarInt,
        pub z: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x5D, state=Game)]
    pub struct ClientboundChunkCacheRadius {
        pub radius: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x77, state=Game)]
    pub struct ClientboundSystemChatPacket {
        pub content: Text,
        pub overlay: bool,
    }

    impl<'a> crate::Encode for ClientboundPlayerInfoUpdate<'a> {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            self.actions.into_bits().encode(&mut w)?;

            // Write number of entries.
            VarInt(self.entries.len() as i32).encode(&mut w)?;

            for entry in self.entries.as_ref() {
                entry.player_uuid.encode(&mut w)?;

                if self.actions.add_player() {
                    entry.username.encode(&mut w)?;
                    entry.properties.encode(&mut w)?;
                }

                if self.actions.initialize_chat() {
                    entry.chat_data.encode(&mut w)?;
                }

                if self.actions.update_game_mode() {
                    entry.game_mode.encode(&mut w)?;
                }

                if self.actions.update_listed() {
                    entry.listed.encode(&mut w)?;
                }

                if self.actions.update_latency() {
                    VarInt(entry.ping).encode(&mut w)?;
                }

                if self.actions.update_display_name() {
                    entry.display_name.encode(&mut w)?;
                }
            }

            Ok(())
        }
    }

    impl<'a> crate::Decode<'a> for ClientboundPlayerInfoUpdate<'a> {
        fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
            let actions = PlayerListActions::from_bits(u8::decode(r)?);

            let mut entries = vec![];

            for _ in 0..VarInt::decode(r)?.0 {
                let mut entry = PlayerListEntry {
                    player_uuid: Uuid::decode(r)?,
                    ..Default::default()
                };

                if actions.add_player() {
                    entry.username = crate::Decode::decode(r)?;
                    entry.properties = crate::Decode::decode(r)?;
                }

                if actions.initialize_chat() {
                    entry.chat_data = crate::Decode::decode(r)?;
                }

                if actions.update_game_mode() {
                    entry.game_mode = crate::Decode::decode(r)?;
                }

                if actions.update_listed() {
                    entry.listed = crate::Decode::decode(r)?;
                }

                if actions.update_latency() {
                    entry.ping = VarInt::decode(r)?.0;
                }

                if actions.update_display_name() {
                    entry.display_name = crate::Decode::decode(r)?;
                }

                entries.push(entry);
            }

            Ok(Self {
                actions,
                entries: entries.into(),
            })
        }
    }
}

pub mod serverbound {
    use crate::entity::player::{CommandArgumentSignature, MessageSignature, PlayerAction};
    use crate::item::{ContainerInput, HashedSlot};
    use crate::packets::common::serverbound::{ClientInformation, KeepAlive};
    use crate::pos::MoveFlags;
    use crate::{Bounded, Difficulty, Direction, GameMode, Look, Position, VarInt};
    use derive_more::From;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_protocol_macros::{Decode, Encode, Packet};
    use uuid::Uuid;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x00, state=Game)]
    pub struct ServerboundAcceptTeleportation {
        pub teleport_id: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x01, state=Game)]
    pub struct ServerboundBlockEntityTagQuery {
        pub transaction_id: VarInt,
        pub block_pos: BlockPos,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x02, state=Game)]
    pub struct ServerboundSelectBundleItem {
        pub slot_id: VarInt,
        pub selected_item_index: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x03, state=Game)]
    pub struct ServerboundChangeDifficulty {
        pub difficulty: Difficulty,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x04, state=Game)]
    pub struct ServerboundChangeGameMode {
        pub mode: GameMode,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x05, state=Game)]
    pub struct ServerboundChatAck {
        pub offset: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x06, state=Game)]
    pub struct ServerboundChatCommand<'a> {
        pub command: Bounded<&'a str, 32767>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x07, state=Game)]
    pub struct ServerboundChatCommandSigned<'a> {
        pub command: Bounded<&'a str, 32767>,
        pub timestamp: u64,
        pub salt: u64,
        pub argument_signatures: Vec<CommandArgumentSignature<'a>>,
        pub last_seen_messages: MessageSignature,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x08, state=Game)]
    pub struct ServerboundChat<'a> {
        pub message: Bounded<&'a str, 256>,
        pub timestamp: u64,
        pub salt: u64,
        pub signature: Option<&'a [u8; 256]>,
        pub last_seen_messages: MessageSignature,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x09, state=Game)]
    pub struct ServerboundChatSessionUpdate {
        pub session_id: Uuid,
        pub public_key: [u8; 32],
    }

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x0D, state=Game)]
    pub struct ServerboundClientInformation<'a>(pub ClientInformation<'a>);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x11, state=Game)]
    pub struct ServerboundContainerClick {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot_index: i16,
        pub button: u8,
        pub container_input: ContainerInput,
        pub changed_slots: Vec<(u16, Option<HashedSlot>)>,
        pub carried_item: Option<HashedSlot>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1B, state=Game)]
    pub struct ServerboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1D, state=Game)]
    pub struct ServerboundMovePlayerPos {
        pub position: Position,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1E, state=Game)]
    pub struct ServerboundMovePlayerPosRot {
        pub position: Position,
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1F, state=Game)]
    pub struct ServerboundMovePlayerRot {
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x20, state=Game)]
    pub struct ServerboundMovePlayerStatusOnly {
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x28, state=Game)]
    pub struct ServerboundPlayerAction {
        pub action: PlayerAction,
        pub pos: BlockPos,
        pub direction: Direction,
        pub sequence: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x34, state=Game)]
    pub struct ServerboundSetCarriedItem {
        pub slot: u16,
    }
}
