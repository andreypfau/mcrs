pub mod end_platform;
pub mod nether;

use crate::world::arrival::end_platform::place_end_platform;
use crate::world::arrival::nether::find_or_create_nether_landing;
use crate::world::bus::{ArrivalCause, InboundEntitySpawn, MovePayload};
use crate::world::channel_types::FromDim;
use bevy_app::{FixedPreUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Query, Res, ResMut, With};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::session::{DimPlayerIndex, Owner, PlayerSession};
use mcrs_engine::world::chunk::ChunkIndex;
use mcrs_engine::world::dimension::{Dimension, InDimension};
use mcrs_engine::world::channels::FromDimSender;
use mcrs_minecraft_block::block_update::BlockSetRequest;
use mcrs_minecraft_block::palette::BlockPalette;

const END_ARRIVAL_DEFAULT: DVec3 = DVec3::new(0.0, 64.0, 0.0);

pub struct ArrivalPlugin;

impl Plugin for ArrivalPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(FixedPreUpdate, resolve_arrivals);
    }
}

fn resolve_arrivals(
    mut reader: MessageReader<InboundEntitySpawn>,
    mut block_writer: MessageWriter<BlockSetRequest>,
    sender: Res<FromDimSender<FromDim>>,
    dims: Query<Entity, With<Dimension>>,
    chunk_index_query: Query<&ChunkIndex>,
    palette_query: Query<&BlockPalette>,
    mut dim_player_index: ResMut<DimPlayerIndex>,
    mut commands: Commands,
) {
    let dim_entity = match dims.iter().next() {
        Some(e) => e,
        None => return,
    };

    for spawn in reader.read() {
        let resolved_pos = resolve_position(
            dim_entity,
            &spawn.cause,
            &mut block_writer,
            &chunk_index_query,
            &palette_query,
        );

        let new_entity = match &spawn.payload {
            MovePayload::Player { uuid: _, username: _ } => {
                let session = spawn.player.unwrap_or(PlayerSession(0));
                let entity = commands.spawn((
                    InDimension(dim_entity),
                    Transform::default().with_translation(resolved_pos),
                    Owner(session),
                )).id();
                if let Some(session) = spawn.player {
                    dim_player_index.0.insert(session, entity);
                }
                entity
            }
            MovePayload::NonPlayer { .. } => {
                commands.spawn((
                    InDimension(dim_entity),
                    Transform::default().with_translation(resolved_pos),
                )).id()
            }
        };

        let _ = (new_entity, sender.0.try_send(FromDim::Spawned { move_id: spawn.move_id }));
    }
}

fn resolve_position(
    dim_entity: Entity,
    cause: &ArrivalCause,
    block_writer: &mut MessageWriter<BlockSetRequest>,
    chunk_index_query: &Query<&ChunkIndex>,
    palette_query: &Query<&BlockPalette>,
) -> DVec3 {
    match cause {
        ArrivalCause::EndPlatform => {
            place_end_platform(dim_entity, END_ARRIVAL_DEFAULT, block_writer)
        }
        ArrivalCause::NetherPortal { source_pos } => {
            find_or_create_nether_landing(dim_entity, *source_pos, chunk_index_query, palette_query)
        }
        ArrivalCause::ExactRespawn { pos } => *pos,
        ArrivalCause::CommandTeleport { pos } => *pos,
    }
}
