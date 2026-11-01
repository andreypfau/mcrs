use crate::item::{ContainerInput, DelimitedStack, HashedStack};
use crate::player::{CommandArgumentSignature, MessageSignature, PlayerAction};
use crate::pos::MoveFlags;
use crate::recipe::RecipeBookType;
use crate::{Difficulty, Direction, Hand, LpVec3, Position};
use bitfield_struct::bitfield;
use derive_more::From;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::packets::common::serverbound::{
    ClientInformation, CustomClickAction, Payload, Pong, ResourcePack,
};
use mcrs_minecraft_protocol::packets::cookie::serverbound::CookieResponse;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, VarInt};

use mcrs_voxel::BlockPos;
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
    fn encode(&self, w: impl std::io::Write) -> mcrs_minecraft_protocol::Result<()> {
        (if self.flying { ABILITIES_FLYING } else { 0 }).encode(w)
    }
}

impl Decode<'_> for ServerboundPlayerAbilities {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_protocol::Result<Self> {
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
    fn encode(&self, w: impl std::io::Write) -> mcrs_minecraft_protocol::Result<()> {
        self.into_bits().encode(w)
    }
}

impl Decode<'_> for PlayerInputFlags {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_protocol::Result<Self> {
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
pub struct ServerboundCommandSuggestion<'a> {
    pub id: VarInt,
    pub text: Bounded<&'a str, 32500>,
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
    OpenedTab(mcrs_minecraft_resource::ResourceLocationBuf),
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
    pub item: DelimitedStack,
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
