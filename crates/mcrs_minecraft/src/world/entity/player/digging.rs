use crate::world::entity::attribute::Attribute;
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::attribute::{BlockBreakSpeed, MiningEfficiency};
use crate::world::entity::player::player_action::{
    PlayerAction, PlayerActionKind, PlayerWillDestroyBlock,
};
use crate::world::experience::BlockDestroyed;
use crate::world::inventory::PlayerHotbarSlots;
use mcrs_vanilla::item::component::Enchantments;
use mcrs_vanilla::item::component::Tool;
use mcrs_vanilla::item::{Item, ItemStack};
use crate::world::loot::BlockLootTables;
use crate::world::loot::context::BlockBreakContext;
use bevy_app::{FixedUpdate, Plugin, Update};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_time::{Fixed, Time};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::session::PlayerSession;
use mcrs_voxel_math::BlockPos;
use mcrs_engine::world::dimension::{DimensionPlayers, InDimension};
use mcrs_engine::world::storage::chunk::ChunkIndex;
use mcrs_minecraft_block::block_update::{BlockSetRequest, remove_block};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;
use mcrs_core::tag::registry::DynTagRegistry;
use mcrs_vanilla::block::Block as VanillaBlock;
use mcrs_vanilla::block::definition::{BlockDefinitions, BlockStateFlags, Blocks};
use std::time::Duration;
use tracing::{debug, trace};

pub struct DiggingPlugin;

impl Plugin for DiggingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(FixedUpdate, tick_digging);
        app.add_systems(
            Update,
            (
                (player_start_destroy_block, handle_player_will_destroy_block)
                    .run_if(resource_exists::<DynTagRegistry<VanillaBlock>>),
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
    chunks: Query<&BlockPalette>,
    mut packet_queue: Local<Vec<(Entity, Entity, BlockPos, i8)>>,
    mut send: SendDestroyBlockProgress,
    mut commands: Commands,
) {
    players
        .iter_mut()
        .for_each(|(player, dim, mut digging, _pos)| {
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
    chunks: Query<&BlockPalette>,
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
    tag_registry: Res<DynTagRegistry<VanillaBlock>>,
    blocks: Res<Blocks>,
    time: Res<Time<Fixed>>,
    mut player_will_destroy_block: MessageWriter<PlayerWillDestroyBlock>,
    mut commands: Commands,
) {
    reader.read().for_each(|event| {
        let player = event.player;
        let (dim, _pos, rep, _instant_build, mining_efficiency, block_break_speed, hotbar) =
            match players.get_mut(player) {
                Ok(value) => value,
                Err(_) => return,
            };
        let PlayerActionKind::StartDestroyBlock {
            block_pos,
            direction: _,
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
        let Ok(block_states) = chunks.get(chunk) else {
            return;
        };

        let block_state = block_states.get(block_pos);
        if blocks
            .state(block_state)
            .flags
            .contains(BlockStateFlags::IS_AIR)
        {
            return;
        };

        let mut damage = 1.0;
        if block_state.0 != 0 {
            damage = get_destroy_speed(
                block_state,
                &blocks,
                hotbar,
                &items,
                mining_efficiency,
                block_break_speed,
                &tag_registry,
            );
        }

        if damage >= 1.0 {
            let event = PlayerWillDestroyBlock {
                player,
                chunk,
                block_pos,
                block_state,
            };
            player_will_destroy_block.write(event);
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
        // Vanilla destroys the block only once the full server-computed break
        // duration has elapsed. `progress` saturates at 1.0, and the
        // `+ time.timestep()` lookahead grants exactly one fixed tick of lag
        // tolerance, so requiring `>= 1.0` matches vanilla without the
        // item-duplication window a lower threshold (e.g. 0.7) would open.
        let progress = digging.progress(time.elapsed() + time.timestep());
        if progress >= 1.0 {
            debug!("destroy block: {:?}", block_pos);
            destroy_block_progress.execute(dim.entity(), player, digging.block_pos, -1);
            let event = PlayerWillDestroyBlock {
                player,
                chunk: digging.chunk,
                block_pos: digging.block_pos,
                block_state: digging.block_state,
            };
            player_will_destroy_block.write(event);
        }
        commands.entity(player).remove::<Digging>();
    });
}

#[derive(SystemParam)]
struct SendDestroyBlockProgress<'w, 's> {
    dim_players: Query<'w, 's, &'static DimensionPlayers>,
    all_players: Query<'w, 's, (Entity, &'static HostAnchor, &'static Reposition)>,
    packet_writer: MessageWriter<'w, OutboundPlayerPacket>,
}

impl SendDestroyBlockProgress<'_, '_> {
    fn execute(&mut self, dim: Entity, id: Entity, block_pos: BlockPos, progress: i8) {
        let Some(dim_players) = self.dim_players.get(dim.entity()).ok() else {
            return;
        };
        let mut iter = self.all_players.iter_many(dim_players.iter());
        while let Some((player, anchor, rep)) = iter.fetch_next() {
            if player == id {
                continue;
            }
            self.packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(anchor.0),
                priority: PacketPriority::Normal,
                data: PacketPayload::BlockDestruction {
                    entity_id: id.index_u32() as i32,
                    pos: rep.convert_block_pos(block_pos),
                    progress,
                },
                session: PlayerSession(0),
                epoch: 0,
            });
        }
    }
}

fn get_destroy_speed(
    state: BlockStateId,
    blocks: &BlockDefinitions,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
    mining_efficiency: &MiningEfficiency,
    block_break_speed: &BlockBreakSpeed,
    tag_registry: &DynTagRegistry<VanillaBlock>,
) -> f32 {
    let hardness = blocks.state(state).hardness;
    if hardness < 0.0 {
        return 0.0;
    }
    let (has_correct_tool, mut speed) =
        extract_tool_data(state, blocks, hotbar, items, tag_registry);
    if speed > 1.0 {
        speed += mining_efficiency.value();
    }
    speed *= block_break_speed.value();
    let modifier = if has_correct_tool { 30.0 } else { 100.0 };
    speed / hardness / modifier
}

pub fn extract_tool_data(
    state: BlockStateId,
    blocks: &BlockDefinitions,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
    tag_registry: &DynTagRegistry<VanillaBlock>,
) -> (bool, f32) {
    let block = blocks.owner(state).identifier.as_str();
    let requires_correct_tool = blocks
        .state(state)
        .flags
        .contains(BlockStateFlags::REQUIRES_CORRECT_TOOL_FOR_DROPS);
    let Some(slot) = hotbar.get_selected_slot() else {
        debug!(block, "no selected slot");
        return (!requires_correct_tool, 1.0);
    };
    let Ok((stack, tool)) = items.get(slot) else {
        debug!(block, "slot entity missing ItemStack");
        return (!requires_correct_tool, 1.0);
    };
    let item_id = stack.item_id();
    let item: &Item = item_id.as_ref();
    let Some(tool) = tool.or(item.components.tool.as_ref()) else {
        debug!(block, item = %item.identifier, "no tool component");
        return (!requires_correct_tool, 1.0);
    };
    let has_correct_tool = if requires_correct_tool {
        tool.is_correct_block_for_drops(block, blocks, tag_registry)
    } else {
        true
    };
    let speed = tool.get_mining_speed(block, blocks, tag_registry);
    debug!(
        block,
        item = %item.identifier,
        requires_correct_tool,
        has_correct_tool,
        speed,
        rules = tool.rules.len(),
        "extract_tool_data"
    );
    (has_correct_tool, speed)
}

fn handle_player_will_destroy_block(
    mut reader: MessageReader<PlayerWillDestroyBlock>,
    mut writer: MessageWriter<BlockSetRequest>,
    mut destroyed: MessageWriter<BlockDestroyed>,
    players: Query<(&InDimension, &PlayerHotbarSlots)>,
    items: Query<(&ItemStack, Option<&Enchantments>, Option<&Tool>)>,
    tag_registry: Res<DynTagRegistry<VanillaBlock>>,
    blocks: Res<Blocks>,
    mut loot_tables: ResMut<BlockLootTables>,
    asset_server: Res<AssetServer>,
) {
    reader.read().for_each(|event| {
        // TODO: spawn destroy particles
        // TODO: anger piglin if block is guarded by piglins
        let Ok((dim, hotbar)) = players.get(event.player) else {
            return;
        };

        let state = blocks.state(event.block_state);
        let block_id = blocks.owner(event.block_state).identifier.as_str();
        let held = hotbar.get_selected_slot();

        let has_correct_tool = if state
            .flags
            .contains(BlockStateFlags::REQUIRES_CORRECT_TOOL_FOR_DROPS)
        {
            held.and_then(|slot| items.get(slot).ok())
                .and_then(|(stack, _, tool)| {
                    tool.or_else(|| {
                        AsRef::<Item>::as_ref(&stack.item_id())
                            .components
                            .tool
                            .as_ref()
                    })
                })
                .is_some_and(|tool| {
                    tool.is_correct_block_for_drops(block_id, &blocks, &tag_registry)
                })
        } else {
            true
        };

        if has_correct_tool {
            let tool_enchantments = held
                .and_then(|slot| items.get(slot).ok())
                .and_then(|(_, enchantments, _)| enchantments);

            if let Some(loot) = state.loot {
                match loot_tables.tables.get(&loot) {
                    Some(table) => {
                        let ctx = BlockBreakContext { tool_enchantments };
                        for drop in table.evaluate(&ctx) {
                            debug!(
                                block = %block_id,
                                item = %drop.item_name,
                                count = drop.count,
                                "loot drop"
                            );
                        }
                    }
                    None => {
                        loot_tables.request(loot, &blocks, &asset_server);
                    }
                }
            }

            destroyed.write(BlockDestroyed {
                state: event.block_state,
                pos: event.block_pos,
                dim: dim.entity(),
                tool: held,
                drop_experience: true,
            });
        }

        writer.write(remove_block(**dim, event.block_pos));
    });
}
