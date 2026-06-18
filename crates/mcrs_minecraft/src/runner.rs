use crate::world::sub_app_builder::{drain_dim_despawn_queue, drain_dim_spawn_queue};
use crate::DEFAULT_TPS;
use bevy_app::App;
use bevy_ecs::message::Messages;
use std::time::{Duration, Instant};

pub fn run_server_loop(mut app: App) {
    let tick = Duration::from_secs_f64(1.0 / DEFAULT_TPS.get() as f64);
    app.finish();
    app.cleanup();
    loop {
        let start = Instant::now();
        app.update();
        pump_channels(&mut app);
        drain_dim_spawn_queue(&mut app);
        drain_dim_despawn_queue(&mut app);
        if app.should_exit().is_some() {
            break;
        }
        let elapsed = start.elapsed();
        if elapsed < tick {
            std::thread::sleep(tick - elapsed);
        }
    }
}

pub fn pump_channels(app: &mut App) {
    use crate::world::bus::{
        OutboundPlayerAttached, OutboundPlayerPacket, OutboundPlayerTransfer,
        OutboundPlayerTransferRequest, PacketTarget,
    };
    use crate::world::channel_types::{DimChannelsResource, FromDim};
    use mcrs_engine::session::SessionRegistry;

    let world = app.world_mut();

    let dim_entries: Vec<(bevy_ecs::entity::Entity, Vec<FromDim>)> = {
        let Some(channels) = world.get_resource::<DimChannelsResource>() else {
            return;
        };
        channels
            .iter()
            .map(|(entity, entry)| {
                let msgs: Vec<FromDim> = entry.from_dim_receiver.try_iter().collect();
                (*entity, msgs)
            })
            .collect()
    };

    for (_dim_entity, msgs) in dim_entries {
        for msg in msgs {
            match msg {
                FromDim::Clientbound {
                    target,
                    priority,
                    data,
                    session: _,
                    epoch: _,
                } => {
                    let (stamped_session, stamped_epoch) =
                        if let PacketTarget::SinglePlayer(entity) = &target {
                            let session_registry = world.resource::<SessionRegistry>();
                            let session_opt = session_registry
                                .get_by_anchor(entity)
                                .map(|(s, _)| *s);
                            if let Some(session) = session_opt {
                                let epoch = session_registry
                                    .get(&session)
                                    .map(|e| e.epoch)
                                    .unwrap_or(0);
                                (session, epoch)
                            } else {
                                (mcrs_engine::session::PlayerSession(0), 0)
                            }
                        } else {
                            (mcrs_engine::session::PlayerSession(0), 0)
                        };

                    let pkt = OutboundPlayerPacket {
                        target,
                        priority,
                        data,
                        session: stamped_session,
                        epoch: stamped_epoch,
                    };
                    world
                        .resource_mut::<Messages<OutboundPlayerPacket>>()
                        .write(pkt);
                }
                FromDim::Transfer {
                    host_anchor,
                    dest_dim,
                    snapshot,
                } => {
                    world
                        .resource_mut::<Messages<OutboundPlayerTransfer>>()
                        .write(OutboundPlayerTransfer {
                            host_anchor,
                            dest_dim,
                            snapshot,
                        });
                }
                FromDim::TransferRequest {
                    host_anchor,
                    dim_name,
                    snapshot,
                } => {
                    world
                        .resource_mut::<Messages<OutboundPlayerTransferRequest>>()
                        .write(OutboundPlayerTransferRequest {
                            host_anchor,
                            dim_name,
                            snapshot,
                        });
                }
                FromDim::Attached {
                    host_anchor,
                    new_in_dim_entity,
                } => {
                    world
                        .resource_mut::<Messages<OutboundPlayerAttached>>()
                        .write(OutboundPlayerAttached {
                            host_anchor,
                            new_in_dim_entity,
                        });
                }
            }
        }
    }
}
