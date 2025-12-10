use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_app::{App, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader};
use bevy_ecs::prelude::{MessageWriter, On};
use bevy_ecs::system::{Query, Res};
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::{BlockPos, BlockStateId, Direction};
use mcrs_protocol::packets::game::serverbound::ServerboundPlayerAction;
use crate::world::block_update::BlockSetRequest;
use crate::world::chunk::{ChunkBlockStates, ChunkIndex};
use crate::world::dimension_time::DimensionTime;
use crate::world::entity::attribute::Attribute;
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::attribute::{BlockBreakSpeed, MiningEfficiency};

pub struct PlayerActionPlugin;

impl Plugin for PlayerActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerAction>();
        app.add_message::<PlayerWillDestroyBlock>();
        app.add_observer(handle_player_action_packet);
        app.add_systems(FixedPreUpdate, (
            player_start_destroy_block,
            handle_player_will_destroy_block
        ).chain());
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Message)]
pub struct PlayerAction {
    player: Entity,
    kind: PlayerActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerActionKind {
    StartDestroyBlock {
        block_pos: BlockPos,
        direction: Direction
    },
    AbortDestroyBlock {
        block_pos: BlockPos,
    },
    StopDestroyBlock {
        block_pos: BlockPos,
        direction: Direction
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
            mcrs_protocol::entity::player::PlayerAction::StartDestroyBlock =>
                PlayerActionKind::StartDestroyBlock {
                    block_pos: value.pos,
                    direction: value.direction
                },
            mcrs_protocol::entity::player::PlayerAction::AbortDestroyBlock =>
                PlayerActionKind::AbortDestroyBlock {
                    block_pos: value.pos
                },
            mcrs_protocol::entity::player::PlayerAction::StopDestroyBlock =>
                PlayerActionKind::StopDestroyBlock {
                    block_pos: value.pos,
                    direction: value.direction
                },
            mcrs_protocol::entity::player::PlayerAction::DropItem =>
                PlayerActionKind::DropItem,
            mcrs_protocol::entity::player::PlayerAction::DropAllItems =>
                PlayerActionKind::DropAllItems,
            mcrs_protocol::entity::player::PlayerAction::ReleaseUseItem =>
                PlayerActionKind::ReleaseUseItem,
            mcrs_protocol::entity::player::PlayerAction::SwapItemWithOffhand =>
                PlayerActionKind::SwapItemWithOffhand,
            mcrs_protocol::entity::player::PlayerAction::Stab =>
                PlayerActionKind::Stab,
        }
    }
}

fn handle_player_action_packet(
    event: On<ReceivedPacketEvent>,
    mut writer: MessageWriter<PlayerAction>,
    time: Res<DimensionTime>
) {
    let Some(pkt) = event.decode::<ServerboundPlayerAction>() else { return };
    println!("[{time:?}] Player sent action {:?} {:?}", event.entity, pkt);
    writer.write(PlayerAction {
        player: event.entity,
        kind: PlayerActionKind::from(pkt)
    });
}

fn player_start_destroy_block(
    mut reader: MessageReader<PlayerAction>,
    chunk_index: Res<ChunkIndex>,
    chunks: Query<(&ChunkBlockStates)>,
    players: Query<(&InstantBuild, &MiningEfficiency, &BlockBreakSpeed)>,
    mut player_will_destroy_block: MessageWriter<PlayerWillDestroyBlock>,
    time: Res<DimensionTime>,
) {
    reader.read().for_each(|event| {
        let player = event.player;
        let Ok((instant_build, mining_efficiency, block_break_speed)) = players.get(player) else { return };
        let PlayerActionKind::StartDestroyBlock { block_pos, direction } = event.kind else { return };
        let Some(chunk) = chunk_index.get(block_pos).map(|e| e.chunk) else { return };
        let Ok((block_states)) = chunks.get(chunk) else { return };
        let block_state = block_states.get(block_pos);

        let mut speed = 1.0;
        if block_state.0 != 0 {
            speed = get_destroy_speed(mining_efficiency, block_break_speed);
        }

        if speed >= 1.0 {
            println!("[{time:?}] PlayerWillDestroyBlock");
            player_will_destroy_block.write(PlayerWillDestroyBlock {
                player,
                chunk,
                block_pos,
                block_state,
            });
        }
    });
}

fn get_destroy_speed(
    mining_efficiency: &MiningEfficiency,
    block_break_speed: &BlockBreakSpeed,
) -> f32 {
    let mut speed = 1.0;
    if speed > 1.0 {
        speed += mining_efficiency.value();
    }
    speed *= block_break_speed.value();
    speed
}

#[derive(Clone, Copy, Debug, Message)]
pub struct PlayerWillDestroyBlock {
    pub player: Entity,
    pub chunk: Entity,
    pub block_pos: BlockPos,
    pub block_state: BlockStateId,
}

fn handle_player_will_destroy_block(
    mut reader: MessageReader<PlayerWillDestroyBlock>,
    mut writer: MessageWriter<BlockSetRequest>,
    time: Res<DimensionTime>,
) {
    reader.read().for_each(|event| {
        // TODO: spawn destroy particles
        // TODO: anger piglin if block is guarded by piglins

        println!("[{time:?}] write Destroying block request at {:?}", event.block_pos);
        writer.write(BlockSetRequest::remove_block(event.block_pos));
    });
}
