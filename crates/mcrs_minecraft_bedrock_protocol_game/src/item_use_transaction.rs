use alloc::vec::Vec;
use core::num::NonZeroI32;

use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, NetworkBlockPos, VarI32, VarU32,
};

use crate::inventory_action::InventoryAction;
use crate::item_stack::{NetworkItemStack, decode_network_item, encode_network_item};

/// The use of the held item on a block or on the air, the part of input flag 34 of
/// [`PlayerAuthInput`](crate::PlayerAuthInput).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemUseTransaction<'a> {
    /// The legacy request; `None` writes a legacy request id of 0 and no legacy slots.
    pub legacy_request: Option<LegacyRequest<'a>>,
    /// The changes of slots.
    pub actions: Vec<InventoryAction<'a>>,
    /// What the player does with the item.
    pub action_type: ItemUseAction,
    /// Why the client sends the transaction.
    pub trigger_type: ItemUseTrigger,
    /// The block that the player clicks.
    pub block_position: NetworkBlockPos,
    /// The face of the block: 0 down, 1 up, 2 north, 3 south, 4 west, 5 east.
    pub block_face: VarI32,
    /// The selected hotbar slot.
    pub hotbar_slot: VarI32,
    /// The held item; `None` is the empty item.
    pub held_item: Option<NetworkItemStack<'a>>,
    /// The position of the player.
    pub player_position: Vec3,
    /// The click position, relative to the block.
    pub click_position: Vec3,
    /// The block runtime id of the clicked block.
    pub block_runtime_id: VarU32,
    /// Whether the client predicts that the interaction succeeds.
    pub client_interact_prediction: InteractPrediction,
}

/// A legacy request with a non-zero id and its slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyRequest<'a> {
    /// The legacy request id.
    pub id: NonZeroI32,
    /// The slots of the request.
    pub slots: Vec<LegacySlots<'a>>,
}

/// The changed slots of one container in a [`LegacyRequest`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct LegacySlots<'a> {
    /// The container id.
    pub container_id: u8,
    /// The slot indices.
    pub slots: &'a [u8],
}

/// The action type of an [`ItemUseTransaction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum ItemUseAction {
    /// Clicks a block.
    ClickBlock = 0,
    /// Clicks the air.
    ClickAir = 1,
    /// Breaks a block.
    BreakBlock = 2,
}

/// The trigger type of an [`ItemUseTransaction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum ItemUseTrigger {
    /// The trigger is not known.
    Unknown = 0,
    /// The input starts in this tick.
    PlayerInput = 1,
    /// The player holds the input from an earlier tick.
    SimulationTick = 2,
}

/// The prediction of the client for an [`ItemUseTransaction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum InteractPrediction {
    /// The interaction fails.
    Failure = 0,
    /// The interaction succeeds.
    Success = 1,
}

impl BedrockEncode for ItemUseTransaction<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match &self.legacy_request {
            Some(request) => {
                VarI32(request.id.get()).encode(w)?;
                request.slots.as_slice().encode(w)?;
            }
            None => VarI32(0).encode(w)?,
        }
        self.actions.as_slice().encode(w)?;
        self.action_type.encode(w)?;
        self.trigger_type.encode(w)?;
        self.block_position.encode(w)?;
        self.block_face.encode(w)?;
        self.hotbar_slot.encode(w)?;
        encode_network_item(self.held_item.as_ref(), w)?;
        self.player_position.encode(w)?;
        self.click_position.encode(w)?;
        self.block_runtime_id.encode(w)?;
        self.client_interact_prediction.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for ItemUseTransaction<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarI32(legacy_request_id) = VarI32::decode(r)?;
        let legacy_request = match NonZeroI32::new(legacy_request_id) {
            Some(id) => Some(LegacyRequest {
                id,
                slots: BedrockDecode::decode(r)?,
            }),
            None => None,
        };
        Ok(Self {
            legacy_request,
            actions: BedrockDecode::decode(r)?,
            action_type: ItemUseAction::decode(r)?,
            trigger_type: ItemUseTrigger::decode(r)?,
            block_position: NetworkBlockPos::decode(r)?,
            block_face: VarI32::decode(r)?,
            hotbar_slot: VarI32::decode(r)?,
            held_item: decode_network_item(r)?,
            player_position: Vec3::decode(r)?,
            click_position: Vec3::decode(r)?,
            block_runtime_id: VarU32::decode(r)?,
            client_interact_prediction: InteractPrediction::decode(r)?,
        })
    }
}
