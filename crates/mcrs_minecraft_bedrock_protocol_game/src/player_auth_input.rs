use alloc::vec::Vec;

use glam::{Vec2, Vec3};
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, Packet, VarI64, VarU32, VarU64,
};

use crate::block_action::{BlockAction, decode_block_actions, encode_block_actions};
use crate::input_flags::InputFlags;
use crate::item_stack_request::ItemStackRequest;
use crate::item_use_transaction::ItemUseTransaction;

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerAuthInput<'a> {
    pub pitch: f32,
    pub yaw: f32,
    pub position: Vec3,
    pub move_vector: Vec2,
    pub head_yaw: f32,
    pub input_flags: InputFlags,
    pub input_mode: InputMode,
    pub play_mode: PlayMode,
    pub interaction_model: InteractionModel,
    pub client_tick: VarU64,
    pub position_delta: Vec3,
    pub item_use_transaction: Option<ItemUseTransaction<'a>>,
    pub item_stack_request: Option<ItemStackRequest<'a>>,
    pub block_actions: Option<Vec<BlockAction>>,
    pub client_predicted_vehicle: Option<ClientPredictedVehicle>,
    pub analog_move_vector: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
#[non_exhaustive]
pub enum InputMode {
    Mouse = 1,
    Touch = 2,
    Gamepad = 3,
    MotionController = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
#[non_exhaustive]
pub enum InteractionModel {
    Touch = 0,
    Crosshair = 1,
    Classic = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum PlayMode {
    Normal,
    Teaser,
    Screen,
    Viewer,
    Reality { gaze_direction: Vec3 },
    Placement,
    LivingRoom,
    ExitLevel,
    ExitLevelLivingRoom,
}

#[derive(Clone, Copy, Debug, PartialEq, BedrockEncode, BedrockDecode)]
pub struct ClientPredictedVehicle {
    pub rotation: Vec2,
    pub unique_id: VarI64,
}

impl PlayMode {
    const fn wire_id(self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::Teaser => 1,
            Self::Screen => 2,
            Self::Viewer => 3,
            Self::Reality { .. } => 4,
            Self::Placement => 5,
            Self::LivingRoom => 6,
            Self::ExitLevel => 7,
            Self::ExitLevelLivingRoom => 8,
        }
    }
}

impl PlayerAuthInput<'_> {
    fn wire_input_flags(&self) -> u64 {
        let mut bits = self.input_flags.bits();
        if self.item_use_transaction.is_some() {
            bits |= InputFlags::ITEM_INTERACTION;
        }
        if self.block_actions.is_some() {
            bits |= InputFlags::BLOCK_ACTIONS;
        }
        if self.item_stack_request.is_some() {
            bits |= InputFlags::ITEM_STACK_REQUEST;
        }
        if self.client_predicted_vehicle.is_some() {
            bits |= InputFlags::CLIENT_PREDICTED_VEHICLE;
        }
        bits
    }
}

impl Packet for PlayerAuthInput<'_> {
    const ID: u16 = 0x90;
}

impl BedrockEncode for PlayerAuthInput<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.pitch.encode(w)?;
        self.yaw.encode(w)?;
        self.position.encode(w)?;
        self.move_vector.encode(w)?;
        self.head_yaw.encode(w)?;
        VarU64(self.wire_input_flags()).encode(w)?;
        self.input_mode.encode(w)?;
        VarU32(self.play_mode.wire_id()).encode(w)?;
        self.interaction_model.encode(w)?;
        if let PlayMode::Reality { gaze_direction } = self.play_mode {
            gaze_direction.encode(w)?;
        }
        self.client_tick.encode(w)?;
        self.position_delta.encode(w)?;
        if let Some(transaction) = &self.item_use_transaction {
            transaction.encode(w)?;
        }
        if let Some(request) = &self.item_stack_request {
            request.encode(w)?;
        }
        if let Some(actions) = &self.block_actions {
            encode_block_actions(actions, w)?;
        }
        if let Some(vehicle) = &self.client_predicted_vehicle {
            vehicle.encode(w)?;
        }
        self.analog_move_vector.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for PlayerAuthInput<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let pitch = f32::decode(r)?;
        let yaw = f32::decode(r)?;
        let position = Vec3::decode(r)?;
        let move_vector = Vec2::decode(r)?;
        let head_yaw = f32::decode(r)?;
        let VarU64(flags) = VarU64::decode(r)?;
        let input_mode = InputMode::decode(r)?;
        let VarU32(play_mode_id) = VarU32::decode(r)?;
        let interaction_model = InteractionModel::decode(r)?;
        let play_mode = match play_mode_id {
            0 => PlayMode::Normal,
            1 => PlayMode::Teaser,
            2 => PlayMode::Screen,
            3 => PlayMode::Viewer,
            4 => PlayMode::Reality {
                gaze_direction: Vec3::decode(r)?,
            },
            5 => PlayMode::Placement,
            6 => PlayMode::LivingRoom,
            7 => PlayMode::ExitLevel,
            8 => PlayMode::ExitLevelLivingRoom,
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        };
        let client_tick = VarU64::decode(r)?;
        let position_delta = Vec3::decode(r)?;
        let has = |bit: u64| flags & bit != 0;
        let item_use_transaction = if has(InputFlags::ITEM_INTERACTION) {
            Some(ItemUseTransaction::decode(r)?)
        } else {
            None
        };
        let item_stack_request = if has(InputFlags::ITEM_STACK_REQUEST) {
            Some(ItemStackRequest::decode(r)?)
        } else {
            None
        };
        let block_actions = if has(InputFlags::BLOCK_ACTIONS) {
            Some(decode_block_actions(r)?)
        } else {
            None
        };
        let client_predicted_vehicle = if has(InputFlags::CLIENT_PREDICTED_VEHICLE) {
            Some(ClientPredictedVehicle::decode(r)?)
        } else {
            None
        };
        Ok(Self {
            pitch,
            yaw,
            position,
            move_vector,
            head_yaw,
            input_flags: InputFlags::from_bits(flags),
            input_mode,
            play_mode,
            interaction_model,
            client_tick,
            position_delta,
            item_use_transaction,
            item_stack_request,
            block_actions,
            client_predicted_vehicle,
            analog_move_vector: Vec2::decode(r)?,
        })
    }
}
