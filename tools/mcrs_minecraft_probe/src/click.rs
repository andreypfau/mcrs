use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_protocol_game::item::stack::HashedPatchMap;
use mcrs_minecraft_protocol_game::item::{ContainerInput, DelimitedStack, HashedStack, ProtoStack};
use mcrs_minecraft_registries::WireId;
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

impl From<ClickMode> for ContainerInput {
    fn from(mode: ClickMode) -> Self {
        match mode {
            ClickMode::Pickup => ContainerInput::Pickup,
            ClickMode::QuickMove => ContainerInput::QuickMove,
            ClickMode::Swap => ContainerInput::Swap,
            ClickMode::Clone => ContainerInput::Clone,
            ClickMode::Throw => ContainerInput::Throw,
            ClickMode::QuickCraft => ContainerInput::QuickCraft,
            ClickMode::PickupAll => ContainerInput::PickupAll,
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
            id: self.item.id(),
            count: self.count,
            components: HashedPatchMap::default(),
        }
    }

    pub(crate) fn delimited(stack: Option<Self>) -> DelimitedStack {
        let Some(stack) = stack else {
            return DelimitedStack::default();
        };
        DelimitedStack(ProtoStack {
            id: WireId::from(stack.item.id()),
            count: stack.count,
            components: Default::default(),
        })
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_zero(value: &i32) -> bool {
    *value == 0
}
