use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::WireStackWithLength;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::Encode;
use mcrs_minecraft_protocol::VarInt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Click {
    pub client: String,
    pub slot: i16,
    pub button: u8,
    pub mode: ClickMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<i32>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub state_id_offset: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_slots: Vec<PredictedSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carried: Option<PredictedStack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickMode {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

impl From<ClickMode> for ClickKind {
    fn from(mode: ClickMode) -> Self {
        match mode {
            ClickMode::Pickup => ClickKind::PickUp,
            ClickMode::QuickMove => ClickKind::QuickMove,
            ClickMode::Swap => ClickKind::Swap,
            ClickMode::Clone => ClickKind::Clone,
            ClickMode::Throw => ClickKind::Throw,
            ClickMode::QuickCraft => ClickKind::Drag,
            ClickMode::PickupAll => ClickKind::PickUpAll,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictedSlot {
    pub slot: u16,
    pub stack: Option<PredictedStack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictedStack {
    pub item: Item,
    pub count: i32,
}

impl PredictedStack {
    pub(crate) fn hashed(self) -> HashedStack {
        HashedStack {
            kind: Kind(self.item.id().number()),
            count: self.count,
            values: Vec::new(),
            removals: Vec::new(),
        }
    }

    pub(crate) fn with_length(
        stack: Option<Self>,
    ) -> mcrs_minecraft_protocol::Result<WireStackWithLength> {
        let Some(stack) = stack.filter(|stack| stack.count > 0 && stack.item != Item::Air) else {
            return Ok(WireStackWithLength::EMPTY);
        };
        let mut bytes = Vec::new();
        VarInt(stack.count).encode(&mut bytes)?;
        VarInt(i32::from(stack.item.id().number())).encode(&mut bytes)?;
        bytes.extend_from_slice(&[0, 0]);
        WireStackWithLength::decode(&mut bytes.as_slice())
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_zero(value: &i32) -> bool {
    *value == 0
}
