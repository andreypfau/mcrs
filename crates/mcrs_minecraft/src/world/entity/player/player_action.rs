use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::prelude::{ContainsEntity, MessageWriter, On};
use mcrs_engine::world::block::BlockPos;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::serverbound::ServerboundPlayerAction;
use mcrs_protocol::{BlockStateId, Direction};

pub struct PlayerActionPlugin;

impl Plugin for PlayerActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerAction>();
        app.add_message::<PlayerWillDestroyBlock>();
        app.add_observer(handle_player_action_packet);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Message)]
pub struct PlayerAction {
    pub player: Entity,
    pub kind: PlayerActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerActionKind {
    StartDestroyBlock {
        block_pos: BlockPos,
        direction: Direction,
    },
    AbortDestroyBlock {
        block_pos: BlockPos,
    },
    StopDestroyBlock {
        block_pos: BlockPos,
        direction: Direction,
    },
    DropItem,
    DropAllItems,
    ReleaseUseItem,
    SwapItemWithOffhand,
    Stab,
}

impl From<ServerboundPlayerAction> for PlayerActionKind {
    fn from(value: ServerboundPlayerAction) -> Self {
        match value.action {
            mcrs_protocol::entity::player::PlayerAction::StartDestroyBlock => {
                PlayerActionKind::StartDestroyBlock {
                    block_pos: value.pos,
                    direction: value.direction,
                }
            }
            mcrs_protocol::entity::player::PlayerAction::AbortDestroyBlock => {
                PlayerActionKind::AbortDestroyBlock {
                    block_pos: value.pos,
                }
            }
            mcrs_protocol::entity::player::PlayerAction::StopDestroyBlock => {
                PlayerActionKind::StopDestroyBlock {
                    block_pos: value.pos,
                    direction: value.direction,
                }
            }
            mcrs_protocol::entity::player::PlayerAction::DropItem => PlayerActionKind::DropItem,
            mcrs_protocol::entity::player::PlayerAction::DropAllItems => {
                PlayerActionKind::DropAllItems
            }
            mcrs_protocol::entity::player::PlayerAction::ReleaseUseItem => {
                PlayerActionKind::ReleaseUseItem
            }
            mcrs_protocol::entity::player::PlayerAction::SwapItemWithOffhand => {
                PlayerActionKind::SwapItemWithOffhand
            }
            mcrs_protocol::entity::player::PlayerAction::Stab => PlayerActionKind::Stab,
        }
    }
}

fn handle_player_action_packet(
    event: On<ReceivedPacketEvent>,
    mut writer: MessageWriter<PlayerAction>,
) {
    let Some(pkt) = event.decode::<ServerboundPlayerAction>() else {
        return;
    };
    writer.write(PlayerAction {
        player: event.entity,
        kind: PlayerActionKind::from(pkt),
    });
}

#[derive(Clone, Copy, Debug, Message)]
pub struct PlayerWillDestroyBlock {
    pub player: Entity,
    pub chunk: Entity,
    pub block_pos: BlockPos,
    pub block_state: BlockStateId,
}
