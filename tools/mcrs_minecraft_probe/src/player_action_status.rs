use mcrs_minecraft_protocol_game::player::PlayerAction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerActionStatus {
    StartDestroyBlock,
    ChangeDestroyDirection,
    AbortDestroyBlock,
    StopDestroyBlock,
    DropAllItems,
    DropItem,
    ReleaseUseItem,
    SwapItemWithOffhand,
    Stab,
}

impl From<PlayerActionStatus> for PlayerAction {
    fn from(status: PlayerActionStatus) -> Self {
        match status {
            PlayerActionStatus::StartDestroyBlock => PlayerAction::StartDestroyBlock,
            PlayerActionStatus::ChangeDestroyDirection => PlayerAction::ChangeDestroyDirection,
            PlayerActionStatus::AbortDestroyBlock => PlayerAction::AbortDestroyBlock,
            PlayerActionStatus::StopDestroyBlock => PlayerAction::StopDestroyBlock,
            PlayerActionStatus::DropAllItems => PlayerAction::DropAllItems,
            PlayerActionStatus::DropItem => PlayerAction::DropItem,
            PlayerActionStatus::ReleaseUseItem => PlayerAction::ReleaseUseItem,
            PlayerActionStatus::SwapItemWithOffhand => PlayerAction::SwapItemWithOffhand,
            PlayerActionStatus::Stab => PlayerAction::Stab,
        }
    }
}
