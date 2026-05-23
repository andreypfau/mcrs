use crate::world::block::Block;
use mcrs_minecraft_block::block_update::BlockSetRequest;
use crate::world::bus::PendingInboundLifecycle;
use crate::world::entity::attribute::Attribute;
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::attribute::{BlockBreakSpeed, MiningEfficiency};
use crate::world::entity::player::player_action::{
    PlayerAction, PlayerActionKind, PlayerWillDestroyBlock,
};
use crate::world::inventory::PlayerHotbarSlots;
use crate::world::item::{Item, ItemStack};
use crate::world::item::component::Tool;
use crate::world::item::component::Enchantments;
use crate::world::player_index::{HostAnchorRef, PlayerIndex};
use crate::enchantment::EnchantmentData;
use crate::world::loot::BlockLootTables;
use crate::world::loot::context::BlockBreakContext;
use mcrs_minecraft_block::palette::BlockPalette;
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
use mcrs_core::StaticRegistry;
use mcrs_core::tag::registry::TagRegistry;
use mcrs_vanilla::block::Block as VanillaBlock;
use rand::RngExt;
use std::time::Duration;
use tracing::{debug, trace};

/// Route a clone of `event` into `PendingInboundLifecycle.block_events`
/// under the destination per-dim bucket. The destination is the
/// `current_dim` of the player's `PlayerLocation`, reverse-looked up via
/// `HostAnchorRef` on the connection entity. Bails (no-op) if the player
/// has no `HostAnchorRef` (not yet logged in), no `PlayerIndex` entry, or
/// a `current_dim == Entity::PLACEHOLDER` value (login-time placeholder
/// that a later dim-selection step will fill — pushing into the placeholder
/// bucket would leak because no sub-app extracts under that label).
fn route_block_event_via_lifecycle(
    event: &PlayerWillDestroyBlock,
    host_anchor_refs: &Query<&HostAnchorRef>,
    player_index: &PlayerIndex,
    lifecycle: &mut PendingInboundLifecycle,
) {
    let Ok(host_anchor_ref) = host_anchor_refs.get(event.player) else {
        return;
    };
    let host_anchor = host_anchor_ref.0;
    let Some(location) = player_index.get(&host_anchor) else {
        return;
    };
    let current_dim = location.current_dim;
    if current_dim == Entity::PLACEHOLDER {
        trace!(
            ?host_anchor,
            "block-event route skipped: current_dim is PLACEHOLDER"
        );
        return;
    }
    lifecycle
        .per_dim
        .entry(current_dim)
        .or_default()
        .block_events
        .push(*event);
}

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
    chunks: Query<&BlockPalette >,
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
    chunks: Query<&BlockPalette >,
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
    tag_registry: Res<TagRegistry<VanillaBlock>>,
    block_registry: Res<StaticRegistry<VanillaBlock>>,
    time: Res<Time<Fixed>>,
    mut player_will_destroy_block: MessageWriter<PlayerWillDestroyBlock>,
    mut lifecycle: ResMut<PendingInboundLifecycle>,
    player_index: Res<PlayerIndex>,
    host_anchor_refs: Query<&HostAnchorRef>,
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
        let Ok(block_states ) = chunks.get(chunk) else {
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
                &tag_registry,
                &block_registry,
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
            route_block_event_via_lifecycle(
                &event,
                &host_anchor_refs,
                &player_index,
                &mut lifecycle,
            );
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
    mut lifecycle: ResMut<PendingInboundLifecycle>,
    player_index: Res<PlayerIndex>,
    host_anchor_refs: Query<&HostAnchorRef>,
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
            let event = PlayerWillDestroyBlock {
                player,
                chunk: digging.chunk,
                block_pos: digging.block_pos,
                block_state: digging.block_state,
            };
            player_will_destroy_block.write(event);
            route_block_event_via_lifecycle(
                &event,
                &host_anchor_refs,
                &player_index,
                &mut lifecycle,
            );
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
    tag_registry: &TagRegistry<VanillaBlock>,
    block_registry: &StaticRegistry<VanillaBlock>,
) -> f32
where
    B: AsRef<Block>,
{
    let block = block.as_ref();
    let hardness = block.hardness();
    if hardness == -1.0 {
        return 0.0;
    }
    let (has_correct_tool, mut speed) = extract_tool_data(block, hotbar, items, tag_registry, block_registry);
    if speed > 1.0 {
        speed += mining_efficiency.value();
    }
    speed *= block_break_speed.value();
    let modifier = if has_correct_tool { 30.0 } else { 100.0 };
    speed / hardness / modifier
}

pub fn extract_tool_data(
    block: &Block,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
    tag_registry: &TagRegistry<VanillaBlock>,
    block_registry: &StaticRegistry<VanillaBlock>,
) -> (bool, f32) {
    let requires_correct_tool = block.requires_correct_tool_for_drops();
    let Some(slot) = hotbar.get_selected_slot() else {
        debug!(block = %block.identifier, "no selected slot");
        return (!requires_correct_tool, 1.0);
    };
    let Ok((stack, tool)) = items.get(slot) else {
        debug!(block = %block.identifier, "slot entity missing ItemStack");
        return (!requires_correct_tool, 1.0);
    };
    let item_id = stack.item_id();
    let item: &Item = item_id.as_ref();
    let Some(tool) = tool.or_else(|| item.components.tool.as_ref()) else {
        debug!(block = %block.identifier, item = %item.identifier, "no tool component");
        return (!requires_correct_tool, 1.0);
    };
    let has_correct_tool = if requires_correct_tool {
        tool.is_correct_block_for_drops(block, tag_registry, block_registry)
    } else {
        true
    };
    let speed = tool.get_mining_speed(block, tag_registry, block_registry);
    debug!(
        block = %block.identifier,
        item = %item.identifier,
        requires_correct_tool,
        has_correct_tool,
        speed,
        rules = tool.rules.len(),
        "extract_tool_data"
    );
    (has_correct_tool, speed)
}

pub fn get_tool_destroy_speed(
    block: &Block,
    hotbar: &PlayerHotbarSlots,
    items: &Query<(&ItemStack, Option<&Tool>)>,
    tag_registry: &TagRegistry<VanillaBlock>,
    block_registry: &StaticRegistry<VanillaBlock>,
) -> f32 {
    let (has_correct_tool, speed) = extract_tool_data(block, hotbar, items, tag_registry, block_registry);
    let modifier = if has_correct_tool { 30.0 } else { 100.0 };
    speed / modifier
}

fn handle_player_will_destroy_block(
    mut reader: MessageReader<PlayerWillDestroyBlock>,
    mut writer: MessageWriter<BlockSetRequest>,
    players: Query<(&InDimension, &PlayerHotbarSlots)>,
    items: Query<(&ItemStack, Option<&Enchantments>, Option<&Tool>)>,
    tag_registry: Res<TagRegistry<VanillaBlock>>,
    block_registry: Res<StaticRegistry<VanillaBlock>>,
    enchantment_registry: Res<StaticRegistry<EnchantmentData>>,
    mut loot_tables: ResMut<BlockLootTables>,
    asset_server: Res<AssetServer>,
    mut silk_touch_id: Local<Option<u16>>,
) {
    if silk_touch_id.is_none() {
        *silk_touch_id = enchantment_registry
            .id_of("minecraft:silk_touch")
            .map(|id| id.raw() as u16);
    }

    reader.read().for_each(|event| {
        // TODO: spawn destroy particles
        // TODO: anger piglin if block is guarded by piglins
        let Ok((dim, hotbar)) = players.get(event.player) else {
            return;
        };

        let block: &Block = event.block_state.as_ref();
        let block_id = block.identifier;

        // Check if the player has the correct tool for drops
        let has_correct_tool = if block.requires_correct_tool_for_drops() {
            if let Some(slot) = hotbar.get_selected_slot() {
                if let Ok((stack, _, tool)) = items.get(slot) {
                    if let Some(tool) = tool.or_else(|| AsRef::<Item>::as_ref(&stack.item_id()).components.tool.as_ref()) {
                        tool.is_correct_block_for_drops(block, &tag_registry, &block_registry)
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            true
        };

        if has_correct_tool {
            let tool_enchantments = hotbar
                .get_selected_slot()
                .and_then(|slot| items.get(slot).ok())
                .and_then(|(_, enchantments, _)| enchantments);

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

            let has_silk_touch = silk_touch_id.is_some_and(|idx| {
                tool_enchantments
                    .map(|e| e.has_enchantment(idx))
                    .unwrap_or(false)
            });

            if !has_silk_touch {
                if let Some((min, max)) = block.xp_range() {
                    let xp = if min == max {
                        min
                    } else {
                        rand::rng().random_range(min..=max)
                    };
                    debug!(block = %block_id, xp = xp, "XP drop");
                }
            }
        }

        writer.write(BlockSetRequest::remove_block(**dim, event.block_pos));
    });
}
