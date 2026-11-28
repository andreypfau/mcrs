use alloc::vec::Vec;
use core::num::NonZeroI32;

use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, NetworkBlockPos, VarI32, VarU32,
};

use crate::inventory_action::InventoryAction;
use crate::item_stack::{NetworkItemStack, decode_network_item, encode_network_item};

#[derive(Clone, Debug, PartialEq)]
pub struct ItemUseTransaction<'a> {
    pub legacy_request: Option<LegacyRequest<'a>>,
    pub actions: Vec<InventoryAction<'a>>,
    pub action_type: ItemUseAction,
    pub trigger_type: ItemUseTrigger,
    pub block_position: NetworkBlockPos,
    pub block_face: VarI32,
    pub hotbar_slot: VarI32,
    pub held_item: Option<NetworkItemStack<'a>>,
    pub player_position: Vec3,
    pub click_position: Vec3,
    pub block_runtime_id: VarU32,
    pub client_interact_prediction: InteractPrediction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyRequest<'a> {
    pub id: NonZeroI32,
    pub slots: Vec<LegacySlots<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct LegacySlots<'a> {
    pub container_id: u8,
    pub slots: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum ItemUseAction {
    ClickBlock = 0,
    ClickAir = 1,
    BreakBlock = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum ItemUseTrigger {
    Unknown = 0,
    PlayerInput = 1,
    SimulationTick = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum InteractPrediction {
    Failure = 0,
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
