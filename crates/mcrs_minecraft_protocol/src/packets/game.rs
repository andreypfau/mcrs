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
    use mcrs_minecraft_protocol_macros::{Decode, Encode};
    use mcrs_minecraft_registry::BlockStateId;
    use std::borrow::Cow;
    use std::io::Write;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
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

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundBlockDestruction {
        pub id: VarInt,
        pub pos: BlockPos,
        pub progress: i8,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundBlockUpdate {
        pub block_pos: BlockPos,
        pub block_state_id: BlockStateId,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundChunkBatchFinished {
        pub batch_size: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundChunkBatchStart;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundContainerClose {
        pub container_id: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundContainerSetContent {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot_data: Vec<RawStack>,
        pub carried_item: RawStack,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundContainerSetData {
        pub container_id: VarInt,
        pub id: i16,
        pub value: i16,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundContainerSetSlot {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot: i16,
        pub item: RawStack,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundDisconnect {
        pub reason: Text,
    }

    #[derive(Clone, Debug, Encode, Decode)]
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

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundEntityPositionSync {
        pub entity_id: VarInt,
        pub position: PositionPath,
        pub look: Look,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundForgetLevelChunk {
        pub z: i32,
        pub x: i32,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundGameEvent {
        pub game_event: GameEventKind,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundLevelChunkWithLight<'a> {
        pub pos: ColumnPos,
        pub chunk_data: crate::chunk::ChunkData<'a>,
        pub light_data: crate::chunk::LightData<'a>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundLevelParticles {
        pub particle: RawParticle,
        pub override_limiter: bool,
        pub always_show: bool,
        pub pos: DVec3,
        pub dist: [f32; 3],
        pub max_speed: [f32; 3],
        pub count: VarInt,
        pub randomization: ParticleRandomization,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode)]
    pub enum ParticleRandomization {
        #[default]
        Default,
        Alternative,
        AlternativeWithSpeed,
    }

    impl crate::Decode<'_> for ParticleRandomization {
        fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
            // Vanilla maps an unknown id to the first variant instead of rejecting it.
            Ok(match VarInt::decode(r)?.0 {
                1 => Self::Alternative,
                2 => Self::AlternativeWithSpeed,
                _ => Self::Default,
            })
        }
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundLightUpdate<'a> {
        pub x: VarInt,
        pub z: VarInt,
        pub light_data: crate::chunk::LightData<'a>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
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

    #[derive(Clone, Debug)]
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

    #[derive(Clone, Debug)]
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

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundMerchantOffers {
        pub container_id: VarInt,
        pub offers: Vec<RawMerchantOffer>,
        pub villager_level: VarInt,
        pub villager_xp: VarInt,
        pub show_progress: bool,
        pub can_restock: bool,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundMoveMinecartAlongTrack {
        pub entity_id: VarInt,
        pub lerp_steps: Vec<MinecartStep>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundMoveEntityRot {
        pub entity_id: VarInt,
        pub y_rot: ByteAngle,
        pub x_rot: ByteAngle,
        pub on_ground: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundOpenScreen {
        pub container_id: VarInt,
        pub menu_type: VarInt,
        pub title: Text,
    }

    #[derive(Clone, Debug)]
    pub struct ClientboundPlayerInfoUpdate<'a> {
        pub actions: PlayerListActions,
        pub entries: Cow<'a, [PlayerListEntry<'a>]>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundPlayerPosition {
        pub teleport_id: VarInt,
        pub position: DVec3,
        pub velocity: DVec3,
        pub look: Look,
        pub flags: Vec<PositionFlag>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundRecipeBookAdd {
        pub entries: Vec<Raw<RecipeBookEntry>>,
        pub replace: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundRecipeBookRemove {
        pub recipes: Vec<VarInt>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundRecipeBookSettings {
        pub book_settings: RecipeBookSettings,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundRemoveEntities {
        pub entity_ids: Vec<VarInt>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundRespawn<'a> {
        pub player_spawn_info: PlayerSpawnInfo<'a>,
        pub data_to_keep: u8,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundRotateHead {
        pub entity_id: VarInt,
        pub y_head_rot: ByteAngle,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundSectionBlocksUpdate<'a> {
        pub chunk_pos: SectionPos,
        pub blocks: Cow<'a, [ChunkBlockUpdateEntry]>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundSetCursorItem {
        pub contents: RawStack,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundSetEntityData<'a> {
        pub entity_id: VarInt,
        pub metadata: Metadata<'a>,
    }

    /// One equipped stack per slot; the wire chains the entries by a
    /// continuation bit on the slot byte, so the list must not be empty.
    #[derive(Clone, Debug, PartialEq)]
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

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundSetHeldSlot {
        pub slot: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundSetPassengers {
        pub vehicle: VarInt,
        pub passengers: Vec<VarInt>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundSetPlayerInventory {
        pub slot: VarInt,
        pub contents: RawStack,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
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

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundUpdateAdvancements {
        pub reset: bool,
        pub added: Vec<RawAdvancement>,
        pub removed: Vec<ResourceLocation>,
        pub progress: Vec<(ResourceLocation, AdvancementProgress)>,
        pub show_advancements: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundUpdateAttributes<'a> {
        pub entity_id: VarInt,
        pub attributes: Vec<AttributeSnapshot<'a>>,
    }

    /// `item_sets` is keyed by `recipe_property_set` id; vanilla writes it
    /// from a hash map, so the order is whatever was received.
    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ClientboundUpdateRecipes {
        pub item_sets: Vec<(ResourceLocation, Raw<RecipePropertySet>)>,
        pub stonecutter_recipes: Vec<Raw<SelectableRecipe>>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundSetChunkCacheCenter {
        pub x: VarInt,
        pub z: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundChunkCacheRadius {
        pub radius: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundStartConfiguration;

    #[derive(Clone, Debug, Encode, Decode)]
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
    use crate::packets::common::serverbound::{
        ClientInformation, CustomClickAction, KeepAlive, Payload, Pong, ResourcePack,
    };
    use crate::packets::cookie::serverbound::CookieResponse;
    use crate::pos::MoveFlags;
    use crate::recipe::RecipeBookType;
    use crate::{
        Bounded, Decode, Difficulty, Direction, Encode, GameMode, Hand, Look, LpVec3, Position,
        VarInt,
    };
    use bitfield_struct::bitfield;
    use derive_more::From;
    use mcrs_minecraft_core::{BlockPos, ResourceLocation};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundAttack {
        pub entity_id: VarInt,
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub enum ClientCommandAction {
        PerformRespawn,
        RequestStats,
        RequestGameRuleValues,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundClientCommand {
        pub action: ClientCommandAction,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundClientTickEnd;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundInteract {
        pub entity_id: VarInt,
        pub hand: Hand,
        pub location: LpVec3,
        pub using_secondary_action: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundMoveVehicle {
        pub position: Position,
        pub look: Look,
        pub on_ground: bool,
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct ServerboundPlayerAbilities {
        pub flying: bool,
    }

    const ABILITIES_FLYING: u8 = 2;

    impl Encode for ServerboundPlayerAbilities {
        fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
            (if self.flying { ABILITIES_FLYING } else { 0 }).encode(w)
        }
    }

    impl Decode<'_> for ServerboundPlayerAbilities {
        fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
            Ok(Self {
                flying: u8::decode(r)? & ABILITIES_FLYING != 0,
            })
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub enum PlayerCommandAction {
        StopSleeping,
        StartSprinting,
        StopSprinting,
        StartRidingJump,
        StopRidingJump,
        OpenInventory,
        StartFallFlying,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPlayerCommand {
        pub entity_id: VarInt,
        pub action: PlayerCommandAction,
        pub data: VarInt,
    }

    #[bitfield(u8)]
    #[derive(PartialEq, Eq)]
    pub struct PlayerInputFlags {
        pub forward: bool,
        pub backward: bool,
        pub left: bool,
        pub right: bool,
        pub jump: bool,
        pub shift: bool,
        pub sprint: bool,
        _pad: bool,
    }

    impl Encode for PlayerInputFlags {
        fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
            self.into_bits().encode(w)
        }
    }

    impl Decode<'_> for PlayerInputFlags {
        fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
            Ok(Self::from_bits(u8::decode(r)?))
        }
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPlayerInput {
        pub input: PlayerInputFlags,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPlayerLoaded;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPunch;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundUseItem {
        pub hand: Hand,
        pub sequence: VarInt,
        pub look: Look,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode, From)]
    pub struct ServerboundCookieResponse<'a>(pub CookieResponse<'a>);

    #[derive(Clone, Debug, PartialEq, Encode, Decode, From)]
    pub struct ServerboundCustomPayload<'a>(pub Payload<'a>);

    #[derive(Clone, Debug, PartialEq, Encode, Decode, From)]
    pub struct ServerboundPong(pub Pong);

    #[derive(Clone, Debug, PartialEq, Encode, Decode, From)]
    pub struct ServerboundResourcePack(pub ResourcePack);

    #[derive(Clone, Debug, PartialEq, Encode, Decode, From)]
    pub struct ServerboundCustomClickAction<'a>(pub CustomClickAction<'a>);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundAcceptTeleportation {
        pub teleport_id: VarInt,
        pub position: Position,
        pub look: Look,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundBlockEntityTagQuery {
        pub transaction_id: VarInt,
        pub block_pos: BlockPos,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundSelectBundleItem {
        pub slot_id: VarInt,
        pub selected_item_index: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChangeDifficulty {
        pub difficulty: Difficulty,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChangeGameMode {
        pub mode: GameMode,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChatAck {
        pub offset: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChatCommand<'a> {
        pub command: Bounded<&'a str, 32767>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChatCommandSigned<'a> {
        pub command: Bounded<&'a str, 32767>,
        pub timestamp: u64,
        pub salt: u64,
        pub argument_signatures: Vec<CommandArgumentSignature<'a>>,
        pub last_seen_messages: MessageSignature,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChat<'a> {
        pub message: Bounded<&'a str, 256>,
        pub timestamp: u64,
        pub salt: u64,
        pub signature: Option<&'a [u8; 256]>,
        pub last_seen_messages: MessageSignature,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChatSessionUpdate {
        pub session_id: Uuid,
        pub public_key: [u8; 32],
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundChunkBatchReceived {
        pub desired_chunks_per_tick: f32,
    }

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundClientInformation<'a>(pub ClientInformation<'a>);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundConfigurationAcknowledged;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundContainerButtonClick {
        pub container_id: VarInt,
        pub button_id: VarInt,
    }

    pub const MAX_CHANGED_SLOTS: usize = 128;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundContainerClick {
        pub container_id: VarInt,
        pub state_seqno: VarInt,
        pub slot_index: i16,
        pub button: u8,
        pub container_input: ContainerInput,
        pub changed_slots: Bounded<Vec<(u16, Option<HashedStack>)>, MAX_CHANGED_SLOTS>,
        pub carried_item: Option<HashedStack>,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundContainerClose {
        pub container_id: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundContainerSlotStateChanged {
        pub slot_id: VarInt,
        pub container_id: VarInt,
        pub new_state: bool,
    }

    pub const MAX_BOOK_PAGES: usize = 100;
    pub const MAX_BOOK_PAGE_CHARS: usize = 1024;
    pub const MAX_BOOK_TITLE_CHARS: usize = 32;

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundEditBook<'a> {
        pub slot: VarInt,
        pub pages: Bounded<Vec<Bounded<&'a str, MAX_BOOK_PAGE_CHARS>>, MAX_BOOK_PAGES>,
        pub title: Option<Bounded<&'a str, MAX_BOOK_TITLE_CHARS>>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundMovePlayerPos {
        pub position: Position,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundMovePlayerPosRot {
        pub position: Position,
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundMovePlayerRot {
        pub look: Look,
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundMovePlayerStatusOnly {
        pub flags: MoveFlags,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPickItemFromBlock {
        pub pos: BlockPos,
        pub include_data: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPickItemFromEntity {
        pub id: VarInt,
        pub include_data: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundPlaceRecipe {
        pub container_id: VarInt,
        pub recipe: VarInt,
        pub use_max_items: bool,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundPlayerAction {
        pub action: PlayerAction,
        pub pos: BlockPos,
        pub direction: Direction,
        pub sequence: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundRenameItem<'a> {
        pub name: &'a str,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundSelectTrade {
        pub item: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundRecipeBookChangeSettings {
        pub book_type: RecipeBookType,
        pub is_open: bool,
        pub is_filtering: bool,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundRecipeBookSeenRecipe {
        pub recipe: VarInt,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub enum SeenAdvancementsAction {
        OpenedTab(ResourceLocation),
        ClosedScreen,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundSeenAdvancements {
        pub action: SeenAdvancementsAction,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundSetCarriedItem {
        pub slot: u16,
    }

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundSetCreativeModeSlot {
        pub slot: i16,
        pub item: RawDelimitedStack,
    }

    #[derive(Clone, Debug, Encode, Decode)]
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
