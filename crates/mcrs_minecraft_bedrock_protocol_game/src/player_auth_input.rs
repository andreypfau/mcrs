use alloc::vec::Vec;

use glam::{Vec2, Vec3};
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, Packet, VarI64, VarU32, VarU64,
};

use crate::block_action::{BlockAction, decode_block_actions, encode_block_actions};
use crate::input_flags::InputFlags;
use crate::item_stack_request::ItemStackRequest;
use crate::item_use_transaction::ItemUseTransaction;

/// The input of the client for one client tick.
///
/// Each optional part sets its input flag on the wire: the flags that the packet writes always
/// agree with the parts that follow them.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerAuthInput<'a> {
    /// The pitch in degrees.
    pub pitch: f32,
    /// The yaw in degrees.
    pub yaw: f32,
    /// The eye position.
    pub position: Vec3,
    /// The side input as x and the forward input as y, each from -1 to 1.
    pub move_vector: Vec2,
    /// The head yaw in degrees.
    pub head_yaw: f32,
    /// The input flags that add no field.
    pub input_flags: InputFlags,
    /// The input device.
    pub input_mode: InputMode,
    /// The play mode, with the gaze direction in virtual reality.
    pub play_mode: PlayMode,
    /// The interaction model.
    pub interaction_model: InteractionModel,
    /// The client tick.
    pub client_tick: VarU64,
    /// The velocity of the client.
    pub position_delta: Vec3,
    /// Input flag 34: the use of the held item.
    pub item_use_transaction: Option<ItemUseTransaction<'a>>,
    /// Input flag 36: an item stack request.
    pub item_stack_request: Option<ItemStackRequest<'a>>,
    /// Input flag 35: the block actions, behind a `varint32` count.
    pub block_actions: Option<Vec<BlockAction>>,
    /// Input flag 45: the vehicle that the client predicts.
    pub client_predicted_vehicle: Option<ClientPredictedVehicle>,
    /// The analog move vector.
    pub analog_move_vector: Vec2,
}

/// The input device of [`PlayerAuthInput`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
#[non_exhaustive]
pub enum InputMode {
    /// A mouse and a keyboard.
    Mouse = 1,
    /// A touch screen.
    Touch = 2,
    /// A gamepad.
    Gamepad = 3,
    /// A motion controller.
    MotionController = 4,
}

/// The interaction model of [`PlayerAuthInput`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
#[non_exhaustive]
pub enum InteractionModel {
    /// Touch.
    Touch = 0,
    /// A crosshair.
    Crosshair = 1,
    /// Classic.
    Classic = 2,
}

/// The play mode of [`PlayerAuthInput`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum PlayMode {
    /// Mode 0.
    Normal,
    /// Mode 1.
    Teaser,
    /// Mode 2.
    Screen,
    /// Mode 3.
    Viewer,
    /// Mode 4: virtual reality.
    Reality {
        /// The gaze direction.
        gaze_direction: Vec3,
    },
    /// Mode 5.
    Placement,
    /// Mode 6.
    LivingRoom,
    /// Mode 7.
    ExitLevel,
    /// Mode 8.
    ExitLevelLivingRoom,
}

/// The vehicle that the client predicts, the part of input flag 45.
#[derive(Clone, Copy, Debug, PartialEq, BedrockEncode, BedrockDecode)]
pub struct ClientPredictedVehicle {
    /// The rotation of the vehicle.
    pub rotation: Vec2,
    /// The unique id of the vehicle.
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
