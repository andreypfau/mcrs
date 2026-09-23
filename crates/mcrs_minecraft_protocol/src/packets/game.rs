pub mod clientbound {
    use crate::advancement::{AdvancementProgress, RawAdvancement};
    use crate::chunk::ChunkBlockUpdateEntry;
    use crate::entity::minecart::MinecartStep;
    use crate::entity::player::*;
    use crate::entity::{EquipmentSlot, Metadata};
    use crate::game_event::GameEventKind;
    use crate::item::{Raw, RawMerchantOffer, RawStack};
    use crate::packets::common::clientbound::KeepAlive;
    use crate::particle::RawParticle;
    use crate::profile::{PlayerListActions, PlayerListEntry};
    use crate::recipe::{RecipeBookEntry, RecipeBookSettings, RecipePropertySet, SelectableRecipe};
    use crate::text::Text;
    use crate::{ColumnPos, Look, LpVec3, PositionFlag, VarInt};
    use crate::{Decode as _, Encode as _};
    use bevy_math::DVec3;
    use mcrs_minecraft_core::BlockPos;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_core::SectionPos;
    use mcrs_minecraft_protocol::ByteAngle;
    use mcrs_minecraft_protocol_macros::{Decode, Encode, Packet};
    use mcrs_minecraft_registry::BlockStateId;
    use std::borrow::Cow;
    use std::io::Write;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x01, state=Game)]
    pub struct ClientboundAddEntity {
        pub id: VarInt,
        pub uuid: Uuid,
        pub kind: VarInt,
        pub pos: DVec3,
        pub movement: LpVec3,
        pub pitch: ByteAngle,
        pub yaw: ByteAngle,
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
    #[packet(id=0x0B, state=Game)]
    pub struct ClientboundChunkBatchFinished {
        pub batch_size: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x0C, state=Game)]
    pub struct ClientboundChunkBatchStart;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x11, state=Game)]
    pub struct ClientboundContainerClose {
        pub container_id: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x12, state=Game)]
    pub struct ClientboundContainerSetContent {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot_data: Vec<RawStack>,
        pub carried_item: RawStack,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x13, state=Game)]
    pub struct ClientboundContainerSetData {
        pub container_id: VarInt,
        pub id: i16,
        pub value: i16,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x14, state=Game)]
    pub struct ClientboundContainerSetSlot {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot: i16,
        pub item: RawStack,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x20, state=Game)]
    pub struct ClientboundDisconnect {
        pub reason: Text,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x22, state=Game)]
    pub struct ClientboundEntityEvent {
        pub entity_id: i32,
        pub entity_status: i8,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct PositionStep {
        pub position: DVec3,
        pub tick_offset: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub enum PositionPath {
        Linear(DVec3),
        Stepped(Vec<PositionStep>),
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x23, state=Game)]
    pub struct ClientboundEntityPositionSync {
        pub entity_id: VarInt,
        pub position: PositionPath,
        pub look: Look,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x26, state=Game)]
    pub struct ClientboundForgetLevelChunk {
        pub z: i32,
        pub x: i32,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x27, state=Game)]
    pub struct ClientboundGameEvent {
        pub game_event: GameEventKind,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x2D, state=Game)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x2E, state=Game)]
    pub struct ClientboundLevelChunkWithLight<'a> {
        pub pos: ColumnPos,
        pub chunk_data: crate::chunk::ChunkData<'a>,
        pub light_data: crate::chunk::LightData<'a>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x30, state=Game)]
    pub struct ClientboundLevelParticles {
        pub override_limiter: bool,
        pub always_show: bool,
        pub pos: DVec3,
        pub dist: [f32; 3],
        pub max_speed: f32,
        pub count: i32,
        pub particle: RawParticle,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x31, state=Game)]
    pub struct ClientboundLightUpdate<'a> {
        pub x: VarInt,
        pub z: VarInt,
        pub light_data: crate::chunk::LightData<'a>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x32, state=Game)]
    pub struct ClientboundLogin<'a> {
        pub player_id: i32,
        pub hardcore: bool,
        pub dimensions: Vec<ResourceLocation<Cow<'a, str>>>,
        pub max_players: VarInt,
        pub chunk_radius: VarInt,
        pub simulation_distance: VarInt,
        pub reduced_debug_info: bool,
        pub show_death_screen: bool,
        pub do_limited_crafting: bool,
        pub player_spawn_info: PlayerSpawnInfo<'a>,
        pub online_mode: bool,
        pub enforces_secure_chat: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct DeltaStep {
        pub ticks: VarInt,
        pub delta: [i16; 3],
    }

    /// A position delta whose step count is not self-describing: it is packed
    /// into the enclosing packet's properties field alongside the on-ground bit.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum VecDelta {
        Linear([i16; 3]),
        Stepped(Vec<DeltaStep>),
    }

    impl Default for VecDelta {
        fn default() -> Self {
            Self::Linear([0; 3])
        }
    }

    impl VecDelta {
        const MIN_ENCODED_STEP_LEN: usize = 7;

        pub fn step_count(&self) -> i32 {
            match self {
                Self::Linear(_) => 0,
                Self::Stepped(steps) => steps.len() as i32,
            }
        }

        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            match self {
                Self::Linear(delta) => delta.encode(w),
                Self::Stepped(steps) => {
                    for step in steps {
                        step.encode(&mut w)?;
                    }
                    Ok(())
                }
            }
        }

        fn decode(r: &mut &[u8], step_count: i32) -> anyhow::Result<Self> {
            if step_count <= 0 {
                return Ok(Self::Linear(crate::Decode::decode(r)?));
            }
            let step_count = step_count as usize;
            anyhow::ensure!(
                step_count <= r.len() / Self::MIN_ENCODED_STEP_LEN,
                "VecDelta with {step_count} steps exceeds the remaining input"
            );
            let mut steps = Vec::with_capacity(step_count);
            for _ in 0..step_count {
                steps.push(DeltaStep::decode(r)?);
            }
            Ok(Self::Stepped(steps))
        }
    }

    fn pack_move_properties(on_ground: bool, step_count: i32) -> VarInt {
        VarInt(i32::from(on_ground) | step_count << 1)
    }

    fn unpack_move_properties(properties: VarInt) -> (bool, i32) {
        (properties.0 & 1 != 0, (properties.0 as u32 >> 1) as i32)
    }

    #[derive(Clone, Debug, Packet)]
    #[packet(id=0x36, state=Game)]
    pub struct ClientboundMoveEntityPos {
        pub entity_id: VarInt,
        pub delta: VecDelta,
        pub on_ground: bool,
    }

    impl crate::Encode for ClientboundMoveEntityPos {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            self.entity_id.encode(&mut w)?;
            pack_move_properties(self.on_ground, self.delta.step_count()).encode(&mut w)?;
            self.delta.encode(w)
        }
    }

    impl crate::Decode<'_> for ClientboundMoveEntityPos {
        fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
            let entity_id = VarInt::decode(r)?;
            let (on_ground, step_count) = unpack_move_properties(VarInt::decode(r)?);
            Ok(Self {
                entity_id,
                delta: VecDelta::decode(r, step_count)?,
                on_ground,
            })
        }
    }

    #[derive(Clone, Debug, Packet)]
    #[packet(id=0x37, state=Game)]
    pub struct ClientboundMoveEntityPosRot {
        pub entity_id: VarInt,
        pub delta: VecDelta,
        pub y_rot: ByteAngle,
        pub x_rot: ByteAngle,
        pub on_ground: bool,
    }

    impl crate::Encode for ClientboundMoveEntityPosRot {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            self.entity_id.encode(&mut w)?;
            pack_move_properties(self.on_ground, self.delta.step_count()).encode(&mut w)?;
            self.delta.encode(&mut w)?;
            self.y_rot.encode(&mut w)?;
            self.x_rot.encode(w)
        }
    }

    impl crate::Decode<'_> for ClientboundMoveEntityPosRot {
        fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
            let entity_id = VarInt::decode(r)?;
            let (on_ground, step_count) = unpack_move_properties(VarInt::decode(r)?);
            let delta = VecDelta::decode(r, step_count)?;
            Ok(Self {
                entity_id,
                delta,
                y_rot: ByteAngle::decode(r)?,
                x_rot: ByteAngle::decode(r)?,
                on_ground,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x35, state=Game)]
    pub struct ClientboundMerchantOffers {
        pub container_id: VarInt,
        pub offers: Vec<RawMerchantOffer>,
        pub villager_level: VarInt,
        pub villager_xp: VarInt,
        pub show_progress: bool,
        pub can_restock: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x38, state=Game)]
    pub struct ClientboundMoveMinecartAlongTrack {
        pub entity_id: VarInt,
        pub lerp_steps: Vec<MinecartStep>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x39, state=Game)]
    pub struct ClientboundMoveEntityRot {
        pub entity_id: VarInt,
        pub y_rot: ByteAngle,
        pub x_rot: ByteAngle,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x3C, state=Game)]
    pub struct ClientboundOpenScreen {
        pub container_id: VarInt,
        pub menu_type: VarInt,
        pub title: Text,
    }

    #[derive(Clone, Debug, Packet)]
    #[packet(id=0x47, state=Game)]
    pub struct ClientboundPlayerInfoUpdate<'a> {
        pub actions: PlayerListActions,
        pub entries: Cow<'a, [PlayerListEntry<'a>]>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x49, state=Game)]
    pub struct ClientboundPlayerPosition {
        pub teleport_id: VarInt,
        pub position: DVec3,
        pub velocity: DVec3,
        pub look: Look,
        pub flags: Vec<PositionFlag>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x4B, state=Game)]
    pub struct ClientboundRecipeBookAdd {
        pub entries: Vec<Raw<RecipeBookEntry>>,
        pub replace: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x4C, state=Game)]
    pub struct ClientboundRecipeBookRemove {
        pub recipes: Vec<VarInt>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x4D, state=Game)]
    pub struct ClientboundRecipeBookSettings {
        pub book_settings: RecipeBookSettings,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x4E, state=Game)]
    pub struct ClientboundRemoveEntities {
        pub entity_ids: Vec<VarInt>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x54, state=Game)]
    pub struct ClientboundRespawn<'a> {
        pub player_spawn_info: PlayerSpawnInfo<'a>,
        pub data_to_keep: u8,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x55, state=Game)]
    pub struct ClientboundRotateHead {
        pub entity_id: VarInt,
        pub y_head_rot: ByteAngle,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x56, state=Game)]
    pub struct ClientboundSectionBlocksUpdate<'a> {
        pub chunk_pos: SectionPos,
        pub blocks: Cow<'a, [ChunkBlockUpdateEntry]>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x62, state=Game)]
    pub struct ClientboundSetCursorItem {
        pub contents: RawStack,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x65, state=Game)]
    pub struct ClientboundSetEntityData<'a> {
        pub entity_id: VarInt,
        pub metadata: Metadata<'a>,
    }

    /// One equipped stack per slot; the wire chains the entries by a
    /// continuation bit on the slot byte, so the list must not be empty.
    #[derive(Clone, Debug, PartialEq, Packet)]
    #[packet(id=0x68, state=Game)]
    pub struct ClientboundSetEquipment {
        pub entity_id: VarInt,
        pub slots: Vec<(EquipmentSlot, RawStack)>,
    }

    impl crate::Encode for ClientboundSetEquipment {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            anyhow::ensure!(!self.slots.is_empty(), "SetEquipment with no slots");
            self.entity_id.encode(&mut w)?;
            let last = self.slots.len() - 1;
            for (i, (slot, stack)) in self.slots.iter().enumerate() {
                let continues = if i != last { 0x80 } else { 0 };
                (*slot as u8 | continues).encode(&mut w)?;
                stack.encode(&mut w)?;
            }
            Ok(())
        }
    }

    impl crate::Decode<'_> for ClientboundSetEquipment {
        fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
            let entity_id = VarInt::decode(r)?;
            let mut slots = Vec::new();
            loop {
                let byte = u8::decode(r)?;
                slots.push((EquipmentSlot::from_id(byte & 0x7F)?, RawStack::decode(r)?));
                if byte & 0x80 == 0 {
                    return Ok(Self { entity_id, slots });
                }
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x6B, state=Game)]
    pub struct ClientboundSetHeldSlot {
        pub slot: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x6D, state=Game)]
    pub struct ClientboundSetPassengers {
        pub vehicle: VarInt,
        pub passengers: Vec<VarInt>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x6E, state=Game)]
    pub struct ClientboundSetPlayerInventory {
        pub slot: VarInt,
        pub contents: RawStack,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x7F, state=Game)]
    pub struct ClientboundTakeItemEntity {
        pub item_id: VarInt,
        pub player_id: VarInt,
        pub amount: VarInt,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
    pub enum AttributeOperation {
        AddValue,
        AddMultipliedBase,
        AddMultipliedTotal,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct AttributeModifier<'a> {
        pub id: ResourceLocation<Cow<'a, str>>,
        pub amount: f64,
        pub operation: AttributeOperation,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct AttributeSnapshot<'a> {
        pub attribute: VarInt,
        pub base: f64,
        pub modifiers: Vec<AttributeModifier<'a>>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x85, state=Game)]
    pub struct ClientboundUpdateAdvancements {
        pub reset: bool,
        pub added: Vec<RawAdvancement>,
        pub removed: Vec<ResourceLocation>,
        pub progress: Vec<(ResourceLocation, AdvancementProgress)>,
        pub show_advancements: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x86, state=Game)]
    pub struct ClientboundUpdateAttributes<'a> {
        pub entity_id: VarInt,
        pub attributes: Vec<AttributeSnapshot<'a>>,
    }

    /// `item_sets` is keyed by `recipe_property_set` id; vanilla writes it
    /// from a hash map, so the order is whatever was received.
    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x88, state=Game)]
    pub struct ClientboundUpdateRecipes {
        pub item_sets: Vec<(ResourceLocation, Raw<RecipePropertySet>)>,
        pub stonecutter_recipes: Vec<Raw<SelectableRecipe>>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x60, state=Game)]
    pub struct ClientboundSetChunkCacheCenter {
        pub x: VarInt,
        pub z: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x61, state=Game)]
    pub struct ClientboundChunkCacheRadius {
        pub radius: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x78, state=Game)]
    pub struct ClientboundStartConfiguration;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x7C, state=Game)]
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
    use crate::item::{ContainerInput, HashedStack, RawDelimitedStack};
    use crate::packets::common::serverbound::{ClientInformation, KeepAlive};
    use crate::pos::MoveFlags;
    use crate::recipe::RecipeBookType;
    use crate::{Bounded, Difficulty, Direction, GameMode, Look, Position, VarInt};
    use derive_more::From;
    use mcrs_minecraft_core::{BlockPos, ResourceLocation};
    use mcrs_minecraft_protocol_macros::{Decode, Encode, Packet};
    use uuid::Uuid;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x00, state=Game)]
    pub struct ServerboundAcceptTeleportation {
        pub teleport_id: VarInt,
        pub position: Position,
        pub look: Look,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x02, state=Game)]
    pub struct ServerboundBlockEntityTagQuery {
        pub transaction_id: VarInt,
        pub block_pos: BlockPos,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x03, state=Game)]
    pub struct ServerboundSelectBundleItem {
        pub slot_id: VarInt,
        pub selected_item_index: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x04, state=Game)]
    pub struct ServerboundChangeDifficulty {
        pub difficulty: Difficulty,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x05, state=Game)]
    pub struct ServerboundChangeGameMode {
        pub mode: GameMode,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x06, state=Game)]
    pub struct ServerboundChatAck {
        pub offset: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x07, state=Game)]
    pub struct ServerboundChatCommand<'a> {
        pub command: Bounded<&'a str, 32767>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x08, state=Game)]
    pub struct ServerboundChatCommandSigned<'a> {
        pub command: Bounded<&'a str, 32767>,
        pub timestamp: u64,
        pub salt: u64,
        pub argument_signatures: Vec<CommandArgumentSignature<'a>>,
        pub last_seen_messages: MessageSignature,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x09, state=Game)]
    pub struct ServerboundChat<'a> {
        pub message: Bounded<&'a str, 256>,
        pub timestamp: u64,
        pub salt: u64,
        pub signature: Option<&'a [u8; 256]>,
        pub last_seen_messages: MessageSignature,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x0A, state=Game)]
    pub struct ServerboundChatSessionUpdate {
        pub session_id: Uuid,
        pub public_key: [u8; 32],
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x0B, state=Game)]
    pub struct ServerboundChunkBatchReceived {
        pub desired_chunks_per_tick: f32,
    }

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x0E, state=Game)]
    pub struct ServerboundClientInformation<'a>(pub ClientInformation<'a>);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x10, state=Game)]
    pub struct ServerboundConfigurationAcknowledged;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x11, state=Game)]
    pub struct ServerboundContainerButtonClick {
        pub container_id: VarInt,
        pub button_id: VarInt,
    }

    pub const MAX_CHANGED_SLOTS: usize = 128;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x12, state=Game)]
    pub struct ServerboundContainerClick {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot_index: i16,
        pub button: u8,
        pub container_input: ContainerInput,
        pub changed_slots: Bounded<Vec<(u16, Option<HashedStack>)>, MAX_CHANGED_SLOTS>,
        pub carried_item: Option<HashedStack>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x13, state=Game)]
    pub struct ServerboundContainerClose {
        pub container_id: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x14, state=Game)]
    pub struct ServerboundContainerSlotStateChanged {
        pub slot_id: VarInt,
        pub container_id: VarInt,
        pub new_state: bool,
    }

    pub const MAX_BOOK_PAGES: usize = 100;
    pub const MAX_BOOK_PAGE_CHARS: usize = 1024;
    pub const MAX_BOOK_TITLE_CHARS: usize = 32;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x18, state=Game)]
    pub struct ServerboundEditBook<'a> {
        pub slot: VarInt,
        pub pages: Bounded<Vec<Bounded<&'a str, MAX_BOOK_PAGE_CHARS>>, MAX_BOOK_PAGES>,
        pub title: Option<Bounded<&'a str, MAX_BOOK_TITLE_CHARS>>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1C, state=Game)]
    pub struct ServerboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1E, state=Game)]
    pub struct ServerboundMovePlayerPos {
        pub position: Position,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x1F, state=Game)]
    pub struct ServerboundMovePlayerPosRot {
        pub position: Position,
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x20, state=Game)]
    pub struct ServerboundMovePlayerRot {
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x21, state=Game)]
    pub struct ServerboundMovePlayerStatusOnly {
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x24, state=Game)]
    pub struct ServerboundPickItemFromBlock {
        pub pos: BlockPos,
        pub include_data: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x25, state=Game)]
    pub struct ServerboundPickItemFromEntity {
        pub id: VarInt,
        pub include_data: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x27, state=Game)]
    pub struct ServerboundPlaceRecipe {
        pub container_id: VarInt,
        pub recipe: VarInt,
        pub use_max_items: bool,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x29, state=Game)]
    pub struct ServerboundPlayerAction {
        pub action: PlayerAction,
        pub pos: BlockPos,
        pub direction: Direction,
        pub sequence: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x31, state=Game)]
    pub struct ServerboundRenameItem<'a> {
        pub name: &'a str,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x34, state=Game)]
    pub struct ServerboundSelectTrade {
        pub item: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x2F, state=Game)]
    pub struct ServerboundRecipeBookChangeSettings {
        pub book_type: RecipeBookType,
        pub is_open: bool,
        pub is_filtering: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x30, state=Game)]
    pub struct ServerboundRecipeBookSeenRecipe {
        pub recipe: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub enum SeenAdvancementsAction {
        OpenedTab(ResourceLocation),
        ClosedScreen,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x33, state=Game)]
    pub struct ServerboundSeenAdvancements {
        pub action: SeenAdvancementsAction,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x36, state=Game)]
    pub struct ServerboundSetCarriedItem {
        pub slot: u16,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Packet)]
    #[packet(id=0x39, state=Game)]
    pub struct ServerboundSetCreativeModeSlot {
        pub slot: i16,
        pub item: RawDelimitedStack,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x42, state=Game)]
    pub struct ServerboundUseItemOn {
        pub hand: crate::Hand,
        pub block_pos: BlockPos,
        pub face: Direction,
        pub cursor_x: f32,
        pub cursor_y: f32,
        pub cursor_z: f32,
        pub inside_block: bool,
        pub world_border_hit: bool,
        pub sequence: VarInt,
    }
}
