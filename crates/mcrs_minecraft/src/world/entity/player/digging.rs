use crate::world::block::Block;
use crate::world::block_update::BlockSetRequest;
use crate::world::entity::attribute::Attribute;
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::attribute::{BlockBreakSpeed, MiningEfficiency};
use crate::world::entity::player::player_action::{
    PlayerAction, PlayerActionKind, PlayerWillDestroyBlock,
};
use crate::world::inventory::PlayerHotbarSlots;
use crate::world::item::ItemStack;
use crate::world::item::component::Tool;
use crate::world::item::component::Enchantments;
use crate::world::loot::BlockLootTables;
use crate::world::loot::context::BlockBreakContext;
use crate::world::palette::BlockPalette;
use bevy_app::{FixedUpdate, Plugin, Update};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_time::{Fixed, Time};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::ChunkIndex;
use mcrs_engine::world::dimension::{DimensionPlayers, InDimension};
use mcrs_network::ServerSideConnection;
use mcrs_protocol::packets::game::clientbound::ClientboundBlockDestruction;
use mcrs_protocol::{BlockStateId, VarInt, WritePacket};
use std::time::Duration;
use tracing::{debug, trace};

pub struct DiggingPlugin;

impl Plugin for DiggingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(FixedUpdate, tick_digging);
        app.add_systems(
            Update,
            (
                player_start_destroy_block,
                handle_player_will_destroy_block,
                player_abort_destroy_block,
                player_stop_destroy_block,
            ),
        );
    }
}

#[derive(Component)]
pub struct Digging {
    pub chunk: Entity,
    pub block_pos: BlockPos,
    pub started_time: Duration,
    pub expected_end_time: Duration,
    pub block_state: BlockStateId,
    pub last_sent_progress: i8,
}

impl Digging {
    pub fn progress(&self, current_time: Duration) -> f32 {
        let total_duration = self.expected_end_time - self.started_time;
        let elapsed = current_time - self.started_time;
        let progress = elapsed.as_secs_f32() / total_duration.as_secs_f32();
        progress.min(1.0)
    }
}

fn tick_digging(
    time: Res<Time<Fixed>>,
    mut players: Query<(Entity, &InDimension, &mut Digging, &Transform)>,
    chunks: Query<(&BlockPalette)>,
    mut packet_queue: Local<Vec<(Entity, Entity, BlockPos, i8)>>,
    mut send: SendDestroyBlockProgress,
    mut commands: Commands,
) {
    players
        .iter_mut()
        .for_each(|(player, dim, mut digging, pos)| {
            let Some(chunk) = chunks.get(digging.chunk).ok() else {
                return;
            };
            let block_state = chunk.get(digging.block_pos);
            if block_state == digging.block_state {
                let progress = digging.progress(time.elapsed());
                let stage = (progress * 10.0).floor() as i8;
                trace!("progress: {:?}", progress);
                if stage != digging.last_sent_progress {
                    packet_queue.push((dim.entity(), player, digging.block_pos, stage));
                    digging.last_sent_progress = stage;
                    trace!("started: {:?}", digging.started_time);
                    trace!("expected: {:?}", digging.expected_end_time);
                    trace!(
                        "actual: {:?}",
                        digging.expected_end_time - digging.started_time
                    );
                }
            } else {
                trace!(
                    "block state changed: {:?} -> {:?}",
                    digging.block_state, block_state
                );
                packet_queue.push((dim.entity(), player, digging.block_pos, -1));
                commands.entity(player).remove::<Digging>();
            }
        });
    packet_queue
        .drain(..)
        .for_each(|(dim, player, pos, stage)| {
            send.execute(dim, player, pos, stage);
        })
}

fn player_start_destroy_block(
    mut reader: MessageReader<PlayerAction>,
    dimensions: Query<&ChunkIndex>,
    chunks: Query<(&BlockPalette)>,
    mut players: Query<(
        &InDimension,
        &Transform,
        &Reposition,
        Has<InstantBuild>,
        &MiningEfficiency,
        &BlockBreakSpeed,
        &PlayerHotbarSlots,
    )>,
    items: Query<(&ItemStack, Option<&Tool>)>,
    time: Res<Time<Fixed>>,
    mut player_will_destroy_block: MessageWriter<PlayerWillDestroyBlock>,
    mut commands: Commands,
) {
    reader.read().for_each(|event| {
        let player = event.player;
        let (dim, pos, rep, instant_build, mining_efficiency, block_break_speed, hotbar) =
            match players.get_mut(player) {
                Ok(value) => value,
                Err(_) => return,
            };
        let PlayerActionKind::StartDestroyBlock {
            block_pos,
            direction,
        } = event.kind
        else {
            return;
        };
        let block_pos = rep.unconvert_block_pos(block_pos);

        let Some(chunk_index) = dimensions.get(dim.entity()).ok() else {
            return;
        };
        let Some(chunk) = chunk_index.get(block_pos) else {
            return;
        };
        let Ok((block_states)) = chunks.get(chunk) else {
            return;
        };

        let block_state = block_states.get(block_pos);
        if block_state.is_air() {
            return;
        };

        let mut damage = 1.0;
        if block_state.0 != 0 {
            damage = get_destroy_speed(
                block_state,
                hotbar,
                &items,
                mining_efficiency,
                block_break_speed,
            );
        }

        if damage >= 1.0 {
            player_will_destroy_block.write(PlayerWillDestroyBlock {
                player,
                chunk,
                block_pos,
                block_state,
            });
        } else {
            let damage_ticks = (1.0 / damage).ceil() as u32;
            let damage_duration = time.timestep() * damage_ticks;
            let now = time.elapsed();
            commands.entity(player).insert(Digging {
                chunk,
                block_pos,
                started_time: now,
                expected_end_time: now + damage_duration,
                block_state,
                last_sent_progress: -1,
            });
        }
    });
}

fn player_abort_destroy_block(
    mut reader: MessageReader<PlayerAction>,
    digging_players: Query<(Entity, &InDimension, &Digging)>,
    mut destroy_block_progress: SendDestroyBlockProgress,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
) {
    reader.read().for_each(|event| {
        let PlayerActionKind::AbortDestroyBlock { block_pos } = event.kind else {
            return;
        };
        let player = event.player;
        debug!("abort destroy block: {:?}", block_pos);
        let Ok((player, dim, digging)) = digging_players.get(player) else {
            debug!("player {} not found", player);
            return;
        };
        destroy_block_progress.execute(dim.entity(), player, digging.block_pos, -1);
        if digging.block_pos != block_pos {
            return;
        }
        debug!("aborted progress: {:?}", digging.progress(time.elapsed()));
        commands.entity(player).remove::<Digging>();
    });
}
fn player_stop_destroy_block(
    time: Res<Time<Fixed>>,
    mut reader: MessageReader<PlayerAction>,
    digging_players: Query<(&InDimension, &Digging)>,
    mut player_will_destroy_block: MessageWriter<PlayerWillDestroyBlock>,
    mut destroy_block_progress: SendDestroyBlockProgress,
    mut commands: Commands,
) {
    reader.read().for_each(|event| {
        let PlayerActionKind::StopDestroyBlock { block_pos, .. } = event.kind else {
            return;
        };
        let player = event.player;
        let Ok((dim, digging)) = digging_players.get(player) else {
            return;
        };
        if digging.block_pos != block_pos {
            return;
        }
        let progress = digging.progress(time.elapsed() + time.timestep());
        if progress >= 0.7 {
            debug!("destroy block: {:?}", block_pos);
            destroy_block_progress.execute(dim.entity(), player, digging.block_pos, -1);
            player_will_destroy_block.write(PlayerWillDestroyBlock {
                player,
                chunk: digging.chunk,
                block_pos: digging.block_pos,
                block_state: digging.block_state,
            });
        }
        commands.entity(player).remove::<Digging>();
    });
}

#[derive(SystemParam)]
struct SendDestroyBlockProgress<'w, 's> {
    dim_players: Query<'w, 's, &'static DimensionPlayers>,
    all_players: Query<
        'w,
        's,
        (
            Entity,
            &'static mut ServerSideConnection,
            &'static Reposition,
        ),
    >,
}

impl SendDestroyBlockProgress<'_, '_> {
    fn execute(&mut self, dim: Entity, id: Entity, block_pos: BlockPos, progress: i8) {
        let Some(dim_players) = self.dim_players.get(dim.entity()).ok() else {
            return;
        };
        let mut iter = self.all_players.iter_many_mut(dim_players.iter());
        while let Some((player, mut conn, rep)) = iter.fetch_next() {
            if player == id {
                continue;
            }
            let packet = ClientboundBlockDestruction {
                id: VarInt(id.index_u32() as i32),
                pos: rep.convert_block_pos(block_pos),
                progress,
            };
            conn.write_packet(&packet);
        }
    }
}

fn get_destroy_speed<B>(
    block: B,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
    mining_efficiency: &MiningEfficiency,
    block_break_speed: &BlockBreakSpeed,
) -> f32
where
    B: AsRef<Block>,
{
    let block = block.as_ref();
    let hardness = block.hardness();
    if hardness == -1.0 {
        return 0.0;
    }
    let mut tool_speed = get_tool_destroy_speed(block, hotbar, items);
    if tool_speed > 1.0 {
        tool_speed += mining_efficiency.value();
    }
    tool_speed *= block_break_speed.value();
    tool_speed / hardness
}

pub fn extract_tool_data(
    block: &Block,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
) -> (bool, f32) {
    if !block.requires_correct_tool_for_drops() {
        return (true, 1.0);
    }
    let Some(slot) = hotbar.get_selected_slot() else {
        return (false, 1.0);
    };
    let Ok((stack, tool)) = items.get(slot) else {
        return (false, 1.0);
    };
    let Some(tool) = tool.or_else(|| stack.item_id().as_ref().components.tool.as_ref()) else {
        return (false, 1.0);
    };
    (
        tool.is_correct_block_for_drops(block),
        tool.get_mining_speed(block),
    )
}

pub fn get_tool_destroy_speed(
    block: &Block,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
) -> f32 {
    let (has_correct_tool, speed) = extract_tool_data(block, hotbar, items);
    let modifier = if has_correct_tool { 30.0 } else { 100.0 };
    speed / modifier
}

fn handle_player_will_destroy_block(
    mut reader: MessageReader<PlayerWillDestroyBlock>,
    mut writer: MessageWriter<BlockSetRequest>,
    players: Query<(&InDimension, &PlayerHotbarSlots)>,
    items: Query<(&ItemStack, Option<&Enchantments>)>,
    mut loot_tables: ResMut<BlockLootTables>,
    asset_server: Res<AssetServer>,
) {
    reader.read().for_each(|event| {
        // TODO: spawn destroy particles
        // TODO: anger piglin if block is guarded by piglins
        let Ok((dim, hotbar)) = players.get(event.player) else {
            return;
        };

        // Evaluate loot table
        let block: &Block = event.block_state.as_ref();
        let block_id = block.identifier;

        let tool_enchantments = hotbar
            .get_selected_slot()
            .and_then(|slot| items.get(slot).ok())
            .and_then(|(_, enchantments)| enchantments);

        if let Some(table) = loot_tables.tables.get(block_id.as_str()) {
            let ctx = BlockBreakContext {
                tool_enchantments,
            };
            let drops = table.evaluate(&ctx);
            for drop in &drops {
                debug!(
                    block = %block_id,
                    item = %drop.item_name,
                    count = drop.count,
                    "Loot drop"
                );
            }
        } else {
            // Trigger lazy load for blocks not yet loaded
            loot_tables.request(&block_id.to_string_ident(), &asset_server);
        }

        writer.write(BlockSetRequest::remove_block(**dim, event.block_pos));
    });
}
