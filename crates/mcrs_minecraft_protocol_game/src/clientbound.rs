use crate::ByteAngle;
use crate::CommandSuggestion;
use crate::Hand;
use crate::advancement::{AdvancementProgress, PositionedAdvancement};
use crate::block::BlockStateNumber;
use crate::chat::{ChatTypeId, CompletionAction, FilterMask, PackedSignature};
use crate::chunk::ChunkBlockUpdateEntry;
use crate::entity::{EquipmentSlot, Metadata};
use crate::game_event::GameEventKind;
use crate::item::{MerchantOffer, ProtoStack};
use crate::minecart::MinecartStep;
use crate::player::*;
use crate::player_list::{PlayerListActions, PlayerListEntry};
use crate::recipe::{RecipeBookEntry, RecipeBookSettings, RecipePropertySet, SelectableRecipe};
use crate::text::RawText;
use crate::{ColumnPos, LpVec3, PositionFlags};
use glam::DVec3;
use mcrs_minecraft_block::block_entity_type_keys::BlockEntityType;
use mcrs_minecraft_command_tree::CommandNode;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
pub use mcrs_minecraft_entity_attribute::AttributeSnapshot;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_particle::ParticleOptions;
use mcrs_minecraft_protocol::{Bounded, Result, VarInt, VarLong};
use mcrs_minecraft_registries::Wire;

use mcrs_minecraft_wire::{Decode, Encode};
use mcrs_voxel::BlockPos;
use mcrs_voxel::SectionPos;
use std::borrow::Cow;
use std::io::Write;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundAddEntity {
    pub id: VarInt,
    pub uuid: Uuid,
    pub kind: EntityType,
    pub pos: DVec3,
    pub movement: LpVec3,
    pub pitch: ByteAngle,
    pub yaw: ByteAngle,
    pub head_yaw: ByteAngle,
    pub data: VarInt,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundBlockChangedAck {
    pub sequence: VarInt,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundBlockDestruction {
    pub id: VarInt,
    pub pos: BlockPos,
    pub progress: i8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientboundBlockEntityData {
    pub pos: BlockPos,
    pub data: Option<(BlockEntityType, NbtCompound)>,
}

impl Decode<'_> for ClientboundBlockEntityData {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let pos = BlockPos::decode(r)?;
        let mut body = std::mem::take(r);
        let data = BlockEntityType::decode(&mut body).ok().and_then(|kind| {
            let data = NbtCompound::decode(&mut body).ok()?;
            body.is_empty().then_some((kind, data))
        });
        Ok(Self { pos, data })
    }
}

impl Encode for ClientboundBlockEntityData {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let (kind, data) = self.data.as_ref().ok_or_else(|| {
            mcrs_minecraft_protocol::Error::invalid("a block entity update without its data")
        })?;
        self.pos.encode(&mut w)?;
        kind.encode(&mut w)?;
        data.encode(&mut w)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ClientboundBlockEvent {
    pub pos: BlockPos,
    pub action: u8,
    pub param: u8,
    pub block: VarInt,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundBlockUpdate {
    pub block_pos: BlockPos,
    pub block_state_id: BlockStateNumber,
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
    pub slot_data: Vec<ProtoStack<Wire>>,
    pub carried_item: ProtoStack<Wire>,
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
    pub item: ProtoStack<Wire>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundDamageEvent {
    pub entity_id: VarInt,
    pub damage_type: VarInt,
    pub cause_entity_id: VarInt,
    pub direct_entity_id: VarInt,
    pub source_position: Option<DVec3>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundEntityEvent {
    pub entity_id: i32,
    pub entity_status: i8,
}

impl ClientboundEntityEvent {
    pub const DIED: i8 = 3;
    pub const REMOVED: i8 = 60;
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct PositionStep {
    pub position: DVec3,
    pub tick_offset: VarInt,
}

#[derive(Clone, Debug, PartialEq, Encode)]
pub enum PositionPath {
    Linear(DVec3),
    Stepped {
        steps: Vec<PositionStep>,
        start_immediately: bool,
    },
}

impl mcrs_minecraft_protocol::Decode<'_> for PositionPath {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        match VarInt::decode(r)?.0 {
            0 => Ok(Self::Linear(mcrs_minecraft_protocol::Decode::decode(r)?)),
            1 => {
                let steps: Vec<PositionStep> = mcrs_minecraft_protocol::Decode::decode(r)?;
                mcrs_minecraft_protocol::wire::ensure!(
                    !steps.is_empty(),
                    "a stepped PositionPath has no step and so no end position"
                );
                Ok(Self::Stepped {
                    steps,
                    start_immediately: bool::decode(r)?,
                })
            }
            tag => Err(mcrs_minecraft_protocol::wire::ErrorKind::UnknownTag {
                ty: "PositionPath",
                tag,
            }
            .into()),
        }
    }
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
pub struct ClientboundLevelChunkWithLight<'a> {
    pub pos: ColumnPos,
    pub chunk_data: crate::chunk::ChunkData<'a>,
    pub light_data: crate::chunk::LightData<'a>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundLevelEvent {
    pub effect: i32,
    pub pos: BlockPos,
    pub effect_data: i32,
    pub global: bool,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundLevelParticles {
    pub particle: ParticleOptions<Wire>,
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

impl mcrs_minecraft_protocol::Decode<'_> for ParticleRandomization {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            1 => Self::Alternative,
            2 => Self::AlternativeWithSpeed,
            _ => Self::Default,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundHurtAnimation {
    pub entity_id: VarInt,
    pub yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ClientboundAnimate {
    pub entity_id: VarInt,
    pub action: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ClientboundSwingAnimation {
    pub entity_id: VarInt,
    pub hand: Hand,
    pub animation: mcrs_minecraft_item::component::SwingAnimationKind,
    pub duration: VarInt,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundLightUpdate<'a> {
    pub x: VarInt,
    pub z: VarInt,
    pub light_data: crate::chunk::LightData<'a>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundLogin {
    pub player_id: i32,
    pub hardcore: bool,
    pub dimensions:
        Vec<mcrs_minecraft_resource::ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>>,
    pub max_players: VarInt,
    pub chunk_radius: VarInt,
    pub simulation_distance: VarInt,
    pub reduced_debug_info: bool,
    pub show_death_screen: bool,
    pub do_limited_crafting: bool,
    pub player_spawn_info: PlayerSpawnInfo,
    pub online_mode: bool,
    pub enforces_secure_chat: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct DeltaStep {
    pub ticks: VarInt,
    pub delta: [i16; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VecDelta {
    Linear([i16; 3]),
    Stepped {
        steps: Vec<DeltaStep>,
        start_immediately: bool,
    },
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
            Self::Stepped { steps, .. } => steps.len() as i32,
        }
    }

    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            Self::Linear(delta) => delta.encode(w),
            Self::Stepped {
                steps,
                start_immediately,
            } => {
                start_immediately.encode(&mut w)?;
                for step in steps {
                    step.encode(&mut w)?;
                }
                Ok(())
            }
        }
    }

    fn decode(r: &mut &[u8], step_count: i32) -> Result<Self> {
        if step_count <= 0 {
            return Ok(Self::Linear(mcrs_minecraft_protocol::Decode::decode(r)?));
        }
        let start_immediately = bool::decode(r)?;
        let step_count = step_count as usize;
        mcrs_minecraft_protocol::wire::ensure!(
            step_count <= r.len() / Self::MIN_ENCODED_STEP_LEN,
            "VecDelta with {step_count} steps exceeds the remaining input"
        );
        let mut steps = Vec::with_capacity(step_count);
        for _ in 0..step_count {
            steps.push(DeltaStep::decode(r)?);
        }
        Ok(Self::Stepped {
            steps,
            start_immediately,
        })
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

impl mcrs_minecraft_protocol::Encode for ClientboundMoveEntityPos {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.entity_id.encode(&mut w)?;
        pack_move_properties(self.on_ground, self.delta.step_count()).encode(&mut w)?;
        self.delta.encode(w)
    }
}

impl mcrs_minecraft_protocol::Decode<'_> for ClientboundMoveEntityPos {
    fn decode(r: &mut &[u8]) -> Result<Self> {
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

impl mcrs_minecraft_protocol::Encode for ClientboundMoveEntityPosRot {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.entity_id.encode(&mut w)?;
        pack_move_properties(self.on_ground, self.delta.step_count()).encode(&mut w)?;
        self.delta.encode(&mut w)?;
        self.y_rot.encode(&mut w)?;
        self.x_rot.encode(w)
    }
}

impl mcrs_minecraft_protocol::Decode<'_> for ClientboundMoveEntityPosRot {
    fn decode(r: &mut &[u8]) -> Result<Self> {
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
    pub offers: Vec<MerchantOffer<Wire>>,
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
    pub on_ground: bool,
    pub y_rot: ByteAngle,
    pub x_rot: ByteAngle,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundOpenScreen<'a> {
    pub container_id: VarInt,
    pub menu_type: MenuType,
    pub title: RawText<'a>,
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
    pub flags: PositionFlags,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientboundPlayerAbilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub may_fly: bool,
    pub instant_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl Encode for ClientboundPlayerAbilities {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let flags = u8::from(self.invulnerable)
            | u8::from(self.flying) << 1
            | u8::from(self.may_fly) << 2
            | u8::from(self.instant_build) << 3;
        flags.encode(&mut w)?;
        self.flying_speed.encode(&mut w)?;
        self.walking_speed.encode(w)
    }
}

impl Decode<'_> for ClientboundPlayerAbilities {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let flags = u8::decode(r)?;
        let [invulnerable, flying, may_fly, instant_build] =
            [0, 1, 2, 3].map(|bit| flags & 1 << bit != 0);
        Ok(Self {
            invulnerable,
            flying,
            may_fly,
            instant_build,
            flying_speed: f32::decode(r)?,
            walking_speed: f32::decode(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundPlayerLookAt {
    pub from: LookAnchor,
    pub target: DVec3,
    pub entity: Option<LookAtEntity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LookAtEntity {
    pub id: VarInt,
    pub anchor: LookAnchor,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundRecipeBookAdd {
    pub entries: Vec<RecipeBookEntry<Wire>>,
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
pub struct ClientboundRespawn {
    pub player_spawn_info: PlayerSpawnInfo,
    pub data_to_keep: u8,
}

impl ClientboundRespawn {
    pub const KEEP_ATTRIBUTE_MODIFIERS: u8 = 0x01;
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
    pub contents: ProtoStack<Wire>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundSetEntityMotion {
    pub entity_id: VarInt,
    pub velocity: LpVec3,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundSetEntityData<'a> {
    pub entity_id: VarInt,
    pub metadata: Metadata<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientboundSetEquipment {
    pub entity_id: VarInt,
    pub slots: Vec<(EquipmentSlot, ProtoStack<Wire>)>,
}

impl mcrs_minecraft_protocol::Encode for ClientboundSetEquipment {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        mcrs_minecraft_protocol::wire::ensure!(
            !self.slots.is_empty(),
            "SetEquipment with no slots"
        );
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

impl mcrs_minecraft_protocol::Decode<'_> for ClientboundSetEquipment {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let entity_id = VarInt::decode(r)?;
        let mut slots = Vec::new();
        loop {
            let byte = u8::decode(r)?;
            slots.push((
                EquipmentSlot::from_id(byte & 0x7F).ok_or_else(|| {
                    mcrs_minecraft_protocol::Error::invalid(format!(
                        "invalid equipment slot {}",
                        byte & 0x7F
                    ))
                })?,
                ProtoStack::<Wire>::decode(r)?,
            ));
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
    pub contents: ProtoStack<Wire>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundTeleportEntity {
    pub entity_id: VarInt,
    pub position: DVec3,
    pub velocity: DVec3,
    pub look: Look,
    pub relatives: PositionFlags,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundTickingState {
    pub tick_rate: f32,
    pub frozen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ClientboundTickingStep {
    pub steps: VarInt,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundTakeItemEntity {
    pub item_id: VarInt,
    pub player_id: VarInt,
    pub amount: VarInt,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundUpdateAdvancements {
    pub reset: bool,
    pub added: Vec<PositionedAdvancement<Wire>>,
    pub removed: Vec<mcrs_minecraft_resource::ResourceLocationBuf>,
    pub progress: Vec<(
        mcrs_minecraft_resource::ResourceLocationBuf,
        AdvancementProgress,
    )>,
    pub show_advancements: bool,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundUpdateAttributes<'a> {
    pub entity_id: VarInt,
    pub attributes: Vec<AttributeSnapshot<'a>>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundUpdateRecipes {
    pub item_sets: Vec<(
        mcrs_minecraft_resource::ResourceLocationBuf,
        RecipePropertySet<Wire>,
    )>,
    pub stonecutter_recipes: Vec<SelectableRecipe<Wire>>,
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
pub struct ClientboundUpdateTags<'a> {
    pub registries:
        Vec<mcrs_minecraft_protocol::packets::configuration::clientbound::RegistryTags<'a>>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundCommands<'a> {
    pub nodes: Vec<CommandNode<'a>>,
    pub root: VarInt,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundCommandSuggestions<'a> {
    pub id: VarInt,
    pub start: VarInt,
    pub length: VarInt,
    pub suggestions: Vec<CommandSuggestion<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ClientboundPing {
    pub id: i32,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct ClientboundSetTime {
    pub game_time: i64,
    pub clocks: Vec<ClockUpdate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Encode, Decode)]
pub struct ClockUpdate {
    pub clock: VarInt,
    pub total_ticks: VarLong,
    pub partial_tick: f32,
    pub rate: f32,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundTabList<'a> {
    pub header: RawText<'a>,
    pub footer: RawText<'a>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundSystemChatPacket<'a> {
    pub content: RawText<'a>,
    pub overlay: bool,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundSetActionBarText<'a> {
    pub text: RawText<'a>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundDisguisedChat<'a> {
    pub message: RawText<'a>,
    pub chat_type: ChatTypeId,
    pub sender_name: RawText<'a>,
    pub target_name: Option<RawText<'a>>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundPlayerChat<'a> {
    pub global_index: VarInt,
    pub sender: Uuid,
    pub index: VarInt,
    pub signature: Option<&'a [u8; 256]>,
    pub content: Bounded<&'a str, 256>,
    pub timestamp: u64,
    pub salt: u64,
    pub last_seen: Bounded<Vec<PackedSignature<'a>>, 20>,
    pub unsigned_content: Option<RawText<'a>>,
    pub filter: FilterMask,
    pub chat_type: ChatTypeId,
    pub sender_name: RawText<'a>,
    pub target_name: Option<RawText<'a>>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundCustomChatCompletions<'a> {
    pub action: CompletionAction,
    pub entries: Vec<Cow<'a, str>>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct ClientboundPlayerInfoRemove {
    pub players: Vec<Uuid>,
}

impl<'a> mcrs_minecraft_protocol::Encode for ClientboundPlayerInfoUpdate<'a> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.actions.into_bits().encode(&mut w)?;

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

            if self.actions.update_list_order() {
                VarInt(entry.list_order).encode(&mut w)?;
            }

            if self.actions.update_hat() {
                entry.show_hat.encode(&mut w)?;
            }
        }

        Ok(())
    }
}

impl<'a> mcrs_minecraft_protocol::Decode<'a> for ClientboundPlayerInfoUpdate<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let actions = PlayerListActions::from_bits(u8::decode(r)?);

        let mut entries = vec![];

        for _ in 0..VarInt::decode(r)?.0 {
            let mut entry = PlayerListEntry {
                player_uuid: Uuid::decode(r)?,
                ..Default::default()
            };

            if actions.add_player() {
                entry.username = mcrs_minecraft_protocol::Decode::decode(r)?;
                entry.properties = mcrs_minecraft_protocol::Decode::decode(r)?;
            }

            if actions.initialize_chat() {
                entry.chat_data = mcrs_minecraft_protocol::Decode::decode(r)?;
            }

            if actions.update_game_mode() {
                entry.game_mode = mcrs_minecraft_protocol::Decode::decode(r)?;
            }

            if actions.update_listed() {
                entry.listed = mcrs_minecraft_protocol::Decode::decode(r)?;
            }

            if actions.update_latency() {
                entry.ping = VarInt::decode(r)?.0;
            }

            if actions.update_display_name() {
                entry.display_name = mcrs_minecraft_protocol::Decode::decode(r)?;
            }

            if actions.update_list_order() {
                entry.list_order = VarInt::decode(r)?.0;
            }

            if actions.update_hat() {
                entry.show_hat = mcrs_minecraft_protocol::Decode::decode(r)?;
            }

            entries.push(entry);
        }

        Ok(Self {
            actions,
            entries: entries.into(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_event_test() {
        let mut bytes = Vec::new();
        ClientboundLevelEvent {
            effect: 2001,
            pos: BlockPos::new(2, -60, 0),
            effect_data: 1,
            global: false,
        }
        .encode(&mut bytes)
        .unwrap();

        assert_eq!(
            bytes,
            [
                0x00, 0x00, 0x07, 0xD1, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x0F, 0xC4, 0x00, 0x00,
                0x00, 0x01, 0x00
            ]
        );
    }

    #[test]
    fn section_blocks_update_packs_each_block_into_a_var_long() {
        let entry = |y, block_state| {
            ChunkBlockUpdateEntry::new()
                .with_off_x(10)
                .with_off_y(y)
                .with_off_z(8)
                .with_block_state(block_state)
        };
        let mut bytes = Vec::new();
        ClientboundSectionBlocksUpdate {
            chunk_pos: SectionPos::new(12, -4, 12),
            blocks: vec![entry(6, 89), entry(7, 0)].into(),
        }
        .encode(&mut bytes)
        .unwrap();

        assert_eq!(
            bytes,
            [
                0x00, 0x00, 0x30, 0x00, 0x00, 0xCF, 0xFF, 0xFC, 0x02, 0x86, 0xB5, 0x16, 0x87, 0x15
            ]
        );
    }

    #[test]
    fn block_changed_ack_is_only_the_sequence() {
        let mut bytes = Vec::new();
        ClientboundBlockChangedAck {
            sequence: VarInt(5),
        }
        .encode(&mut bytes)
        .unwrap();

        assert_eq!(bytes, [5]);
    }
}
