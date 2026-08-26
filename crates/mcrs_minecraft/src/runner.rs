use crate::world::bus::{ArrivalCause, MovePayload, OutboundPlayerPacket, PacketTarget};
use crate::world::channel_types::{FromDim, ToDim};
use crate::world::sub_app_builder::{
    DimLabel, DimSubAppHandle, drain_dim_despawn_queue, drain_dim_spawn_queue,
};
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::world::World;
use mcrs_voxel_server::dim::{DimProtocol, DimRequest};
use mcrs_voxel_world::session::{MoveId, PlayerSession, SessionRegistry};
use std::num::NonZeroU32;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(n) => n,
    None => unreachable!(),
};

pub fn run_server_loop(app: App) {
    mcrs_voxel_server::run_server_loop(app, DEFAULT_TPS, |app| {
        pump_channels(app);
        drain_dim_spawn_queue(app);
        drain_dim_despawn_queue(app);
    });
}

pub fn pump_channels(app: &mut App) {
    mcrs_voxel_server::dim::pump_dim_channels::<MinecraftDims>(app);
}

pub struct MinecraftDims;

impl DimProtocol for MinecraftDims {
    type ToDim = ToDim;
    type FromDim = FromDim;
    type Departure = (ArrivalCause, MovePayload);

    fn classify(world: &World, message: FromDim) -> Option<DimRequest<Self>> {
        match message {
            FromDim::Spawned { move_id } => Some(DimRequest::Arrived { move_id }),
            FromDim::Clientbound { .. } => Some(DimRequest::Other(message)),
            FromDim::MoveEntity {
                move_id,
                target,
                cause,
                payload,
                player,
            } => {
                let destination = world
                    .try_query_filtered::<(Entity, &DimLabel), With<DimSubAppHandle>>()
                    .and_then(|mut dims| {
                        dims.iter(world)
                            .find(|(_, label)| label.0 == target)
                            .map(|(entity, _)| entity)
                    });
                let Some(destination) = destination else {
                    tracing::warn!(
                        target_dim = %target,
                        "MoveEntity names an unknown dim; dropping (no rollback entry)"
                    );
                    return None;
                };
                Some(DimRequest::Move {
                    move_id,
                    destination,
                    session: player,
                    departure: (cause, payload),
                })
            }
        }
    }

    fn depart(
        move_id: MoveId,
        session: Option<PlayerSession>,
        epoch: u32,
        (cause, payload): Self::Departure,
    ) -> ToDim {
        ToDim::SpawnEntity {
            move_id,
            epoch,
            cause,
            payload,
            player: session,
        }
    }

    fn confirm(move_id: MoveId) -> ToDim {
        ToDim::ConfirmMove { move_id }
    }

    fn roll_back(move_id: MoveId) -> ToDim {
        ToDim::RollbackMove { move_id }
    }

    fn deliver(world: &mut World, _source_dim: Entity, message: FromDim) {
        let FromDim::Clientbound {
            target,
            priority,
            data,
            ..
        } = message
        else {
            tracing::warn!("deliver reached a message the move protocol does not route; dropping");
            return;
        };

        let (session, epoch) = match &target {
            PacketTarget::SinglePlayer(anchor) => {
                let registry = world.resource::<SessionRegistry>();
                registry
                    .get_by_anchor(anchor)
                    .map(|(session, entry)| (*session, entry.epoch))
                    .unwrap_or((PlayerSession(0), 0))
            }
            _ => (PlayerSession(0), 0),
        };

        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .write(OutboundPlayerPacket {
                target,
                priority,
                data,
                session,
                epoch,
            });
    }
}
