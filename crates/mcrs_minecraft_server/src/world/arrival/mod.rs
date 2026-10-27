pub mod end_platform;
pub mod nether;

use crate::world::arrival::end_platform::place_end_platform;
use crate::world::arrival::nether::find_or_create_nether_landing;
use crate::world::sub_app_builder::DimInboxDrain;
use bevy_app::{FixedPreUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Query, Res, ResMut, With};
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_engine::bus::{ArrivalCause, InboundEntitySpawn, MovePayload};
use mcrs_minecraft_engine::channel_types::FromDim;
use mcrs_minecraft_level::block_update::BlockSetRequest;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::session::{DimPlayerIndex, Owner, PlayerSession};
use mcrs_minecraft_level::world::channels::FromDimSender;
use mcrs_minecraft_level::world::dimension::{Dimension, InDimension};
use mcrs_minecraft_level::world::storage::view::WorldView;

// Platform at x=8, z=8 ensures the 5×5 floor (6..=10 in each axis) stays
// within a single 16-block chunk section at these default coordinates.
const END_ARRIVAL_DEFAULT: DVec3 = DVec3::new(8.0, 64.0, 8.0);

pub struct ArrivalPlugin;

impl Plugin for ArrivalPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(FixedPreUpdate, resolve_arrivals.after(DimInboxDrain));
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
fn resolve_arrivals(
    mut reader: MessageReader<InboundEntitySpawn>,
    mut block_writer: MessageWriter<BlockSetRequest>,
    blocks: Res<Blocks>,
    sender: Res<FromDimSender<FromDim>>,
    dims: Query<Entity, With<Dimension>>,
    dimension_blocks: WorldView,
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
            &blocks,
            &dimension_blocks,
        );

        let new_entity = match &spawn.payload {
            MovePayload::Player {
                uuid: _,
                username: _,
            } => {
                let session = spawn.player.unwrap_or(PlayerSession(0));
                let entity = commands
                    .spawn((
                        InDimension(dim_entity),
                        Transform::default().with_translation(resolved_pos),
                        Owner(session),
                    ))
                    .id();
                if let Some(session) = spawn.player {
                    dim_player_index.0.insert(session, entity);
                }
                entity
            }
            MovePayload::NonPlayer { .. } => commands
                .spawn((
                    InDimension(dim_entity),
                    Transform::default().with_translation(resolved_pos),
                ))
                .id(),
        };

        let _ = (
            new_entity,
            sender.0.try_send(FromDim::Spawned {
                move_id: spawn.move_id,
            }),
        );
    }
}

fn resolve_position(
    dim_entity: Entity,
    cause: &ArrivalCause,
    block_writer: &mut MessageWriter<BlockSetRequest>,
    blocks: &Blocks,
    dimension_blocks: &WorldView,
) -> DVec3 {
    match cause {
        ArrivalCause::EndPlatform => place_end_platform(
            dim_entity,
            END_ARRIVAL_DEFAULT,
            blocks.default_state_of(Block::Obsidian.id()),
            block_writer,
        ),
        ArrivalCause::NetherPortal { source_pos } => {
            find_or_create_nether_landing(dim_entity, *source_pos, dimension_blocks)
        }
        ArrivalCause::ExactRespawn { pos } => *pos,
    }
}
