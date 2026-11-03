use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLightUpdate;
use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Last, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use mcrs_minecraft_engine::light_codec::{LightCodecParams, build_delta_light_data};
use mcrs_minecraft_engine_world::aoi::PlayerObservers;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStageChanged;
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnAwaitsLight, ColumnLightGate, LightGated, SectionRelit,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnIndex, ColumnPosComponent};
use mcrs_minecraft_light_engine::block::LightRegistry;
use mcrs_minecraft_light_engine::prelude::{Edit, LightBounds};
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::light_actor::LightActor;
use crate::light_plugin::{LightPlugin, LightSet};
use crate::light_state::player_priority;
use crate::to_light::ToLight;

use mcrs_minecraft_heightmap::SurfaceHeightmap;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};

/// A dimension's side of the light engine: what it hands the light actor before intake, and
/// what it sends once the actor's light has been published.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DimLightSet {
    Feed,
    Emit,
}

pub(crate) struct DimLightPlugin {
    pub(crate) registry: Arc<LightRegistry>,
    pub(crate) bounds: LightBounds,
    pub(crate) sky: bool,
}

impl Plugin for DimLightPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LightPlugin {
            registry: Arc::clone(&self.registry),
            bounds: self.bounds,
            sky: self.sky,
            publish: FixedUpdate.intern(),
        })
        .configure_sets(
            FixedUpdate,
            LightSet::Publish.in_set(TickSet::ServiceResults),
        )
        .configure_sets(Last, DimLightSet::Feed.before(LightSet::Intake))
        // `Last` sees what the whole tick landed and placed, and hands it to the actor before
        // the tick ends.
        .add_systems(
            Last,
            (
                (
                    feed_light_edits,
                    reprioritize_light_work,
                    // A surface bound must never describe blocks the light world has not
                    // been handed yet.
                    feed_column_surfaces
                        .after(mcrs_minecraft_engine::heightmap::update_column_heightmaps),
                )
                    .chain()
                    .in_set(DimLightSet::Feed),
                emit_light_updates.in_set(DimLightSet::Emit),
            ),
        )
        .insert_resource(LightGated)
        .configure_sets(FixedUpdate, ColumnLightGate.in_set(TickSet::Project))
        .add_systems(FixedUpdate, await_light.in_set(ColumnLightGate));
    }
}

/// Hands the light actor each column that waits for its light. The actor answers once the
/// light around the column has settled, and [`publish_light`] passes the answer on.
fn await_light(mut awaiting: MessageReader<ColumnAwaitsLight>, mut actor: ResMut<LightActor>) {
    let columns: Vec<ColumnPos> = awaiting
        .read()
        .map(|&ColumnAwaitsLight(column)| column)
        .collect();
    if !columns.is_empty() {
        actor.send(ToLight::AwaitColumns(columns));
    }
}

/// Hands the light actor where the players stand once one of them has moved to another
/// column, so the waiting light work is re-scored against them.
fn reprioritize_light_work(
    mut actor: ResMut<LightActor>,
    players: Query<&Transform, With<Player>>,
    mut scored_for: Local<Vec<ColumnPos>>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    if player_columns.is_empty() || *scored_for == *player_columns {
        return;
    }
    scored_for.clone_from(&*player_columns);
    actor.send(ToLight::Players(player_columns.clone()));
}

fn feed_light_edits(
    mut actor: ResMut<LightActor>,
    mut stages: MessageReader<SectionStageChanged>,
    blocks: Query<&ChunkBlocks>,
    players: Query<&Transform, With<Player>>,
    mut placed: MessageReader<BlockPlaced>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    let mut queue = |edit: Edit| {
        let priority = player_priority(edit.column(), &player_columns);
        actor.push_with_priority(edit, priority);
    };

    for change in stages.read() {
        if change.left() {
            queue(Edit::UnloadSection { pos: change.pos });
        } else if change.landed()
            && let Ok(section_blocks) = blocks.get(change.section)
        {
            queue(Edit::LoadSection {
                pos: change.pos,
                entity: change.section.to_bits(),
                blocks: Arc::clone(&section_blocks.0),
            });
        }
    }
    for placed in placed.read() {
        queue(Edit::SetBlock {
            pos: placed.block_pos,
            block: placed.new_state,
        });
    }
}

/// Hands the sky scan each column's surface, so a section holding no block of a
/// column costs it two seam tests rather than sixteen reads.
fn feed_column_surfaces(
    mut actor: ResMut<LightActor>,
    columns: Query<(&ColumnPosComponent, &SurfaceHeightmap), Changed<SurfaceHeightmap>>,
    players: Query<&Transform, With<Player>>,
    mut player_columns: Local<Vec<ColumnPos>>,
) {
    if columns.is_empty() {
        return;
    }
    player_columns.clear();
    player_columns.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    for (pos, surface) in &columns {
        actor.push_with_priority(
            Edit::SetColumnSurface {
                column: pos.0,
                surface: Arc::new(surface.0.clone()),
            },
            player_priority(pos.0, &player_columns),
        );
    }
}

/// Turns a published light change into a `ClientboundLightUpdate` carrying only
/// the rows that changed, for the players that already hold the column.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub fn emit_light_updates(
    mut relit: MessageReader<SectionRelit>,
    sections: Query<(&SectionPos, &InDimension)>,
    column_indices: Query<&ColumnIndex>,
    observers: Query<&PlayerObservers>,
    owners: Query<&Owner>,
    codec_params: LightCodecParams,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut by_column: Local<FxHashMap<(Entity, ColumnPos), (Vec<Entity>, Vec<Entity>)>>,
) {
    for change in relit.read() {
        let Ok((pos, in_dim)) = sections.get(change.section) else {
            continue;
        };
        let rows = by_column
            .entry((in_dim.0, ColumnPos::from(*pos)))
            .or_default();
        if change.block && !rows.0.contains(&change.section) {
            rows.0.push(change.section);
        }
        if change.sky && !rows.1.contains(&change.section) {
            rows.1.push(change.section);
        }
    }

    for ((dim, column_pos), (block_rows, sky_rows)) in by_column.drain() {
        let Some(column_entity) = column_indices
            .get(dim)
            .ok()
            .and_then(|index| index.0.get(&column_pos).map(|slot| slot.entity))
        else {
            continue;
        };
        let targets: SmallVec<[PlayerSession; 8]> = observers
            .get(column_entity)
            .map(|held| {
                held.0
                    .iter()
                    .filter_map(|player| owners.get(*player).ok().map(|owner| owner.0))
                    .collect()
            })
            .unwrap_or_default();
        if targets.is_empty() {
            continue;
        }

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::PlayerSet(targets),
            // A column's light is sent in full exactly once, so a shed delta is
            // never re-sent; Normal is the class `dispatch_encode` drops first.
            priority: PacketPriority::High,
            data: PacketPayload::LightUpdate(ClientboundLightUpdate {
                x: VarInt(column_pos.x),
                z: VarInt(column_pos.z),
                light_data: build_delta_light_data(
                    column_entity,
                    &block_rows,
                    &sky_rows,
                    &codec_params,
                ),
            }),
            session: PlayerSession(0),
            epoch: 0,
        });
    }
}
