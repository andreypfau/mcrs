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
    use crate::world::bus::{OutboundPlayerPacket, PacketTarget};
    use crate::world::channel_types::{DimChannelsResource, FromDim, ToDim};
    use crate::world::sub_app_builder::{DimLabel, DimSubAppHandle};
    use mcrs_engine::session::SessionRegistry;
    use mcrs_engine::world::in_flight::{InFlightEntry, InFlightMoves};

    let world = app.world_mut();

    // Collect (label_entity, messages) for all dims.
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

    // Collect dim-name → label-entity map for MoveEntity target resolution.
    let dim_name_to_entity: Vec<(String, bevy_ecs::entity::Entity)> = {
        let mut q = world.query_filtered::<
            (bevy_ecs::entity::Entity, &DimLabel),
            bevy_ecs::query::With<DimSubAppHandle>,
        >();
        q.iter(world)
            .map(|(e, label)| (label.0.clone(), e))
            .collect()
    };

    // Accumulate rollback requests from the MoveEntity → Disconnected path so
    // we can send them after the dispatch loop (avoids borrow conflict).
    struct PendingRollback {
        move_id: mcrs_engine::session::MoveId,
        source_dim: bevy_ecs::entity::Entity,
    }
    let mut pending_rollbacks: Vec<PendingRollback> = Vec::new();
    // Accumulate confirm relays from the Spawned path.
    struct PendingConfirm {
        move_id: mcrs_engine::session::MoveId,
        source_dim: bevy_ecs::entity::Entity,
    }
    let mut pending_confirms: Vec<PendingConfirm> = Vec::new();

    for (dim_entity, msgs) in dim_entries {
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
                FromDim::MoveEntity {
                    move_id: _,
                    target,
                    cause,
                    payload,
                    player,
                } => {
                    // Resolve target dim name to a label entity.
                    let Some(dest_dim) = dim_name_to_entity
                        .iter()
                        .find(|(name, _)| *name == target)
                        .map(|(_, e)| *e)
                    else {
                        // Unknown target — immediate rollback (no entity to send to).
                        // We don't have a hidden entity in InFlightMoves yet, so no
                        // InFlightEntry to clean up.
                        tracing::warn!(
                            target_dim = %target,
                            "MoveEntity names an unknown dim; dropping (no rollback entry)"
                        );
                        continue;
                    };

                    // Epoch bump for player moves (must precede SpawnEntity forward).
                    let post_bump_epoch: u32;
                    if let Some(session) = player {
                        let mut reg = world.resource_mut::<SessionRegistry>();
                        let Some(entry) = reg.get_mut(&session) else {
                            tracing::warn!(?session, "MoveEntity player session not found; dropping");
                            continue;
                        };
                        if entry.dim != dest_dim {
                            entry.epoch = entry.epoch.wrapping_add(1);
                        }
                        post_bump_epoch = entry.epoch;
                        entry.dim = dest_dim;
                        entry.in_dim_entity = None;
                    } else {
                        post_bump_epoch = 0;
                    }

                    // Allocate an in-flight entry with the host-assigned move id.
                    // hidden_entity is Entity::PLACEHOLDER because the source sub-app
                    // owns the entity; the host tracks it by source_dim + session only.
                    let allocated_id = world
                        .resource_mut::<InFlightMoves>()
                        .alloc(InFlightEntry {
                            source_dim: dim_entity,
                            hidden_entity: bevy_ecs::entity::Entity::PLACEHOLDER,
                            session: player,
                            ticks_elapsed: 0,
                        });

                    // Forward SpawnEntity to the destination dim's control channel.
                    let send_result = {
                        let channels = world.resource::<DimChannelsResource>();
                        channels.get(dest_dim).map(|chan| {
                            chan.control_sender.try_send(ToDim::SpawnEntity {
                                move_id: allocated_id,
                                epoch: post_bump_epoch,
                                cause,
                                payload,
                                player,
                            })
                        })
                    };

                    match send_result {
                        Some(Ok(())) => {}
                        Some(Err(flume::TrySendError::Disconnected(_))) => {
                            // Target dim channel is gone — immediate rollback.
                            world.resource_mut::<InFlightMoves>().remove(allocated_id);
                            pending_rollbacks.push(PendingRollback {
                                move_id: allocated_id,
                                source_dim: dim_entity,
                            });
                        }
                        Some(Err(flume::TrySendError::Full(_))) => {
                            // Control channel full — treat as saturated, tear down target.
                            world.resource_mut::<InFlightMoves>().remove(allocated_id);
                            pending_rollbacks.push(PendingRollback {
                                move_id: allocated_id,
                                source_dim: dim_entity,
                            });
                            let mut despawn_queue =
                                world.resource_mut::<mcrs_engine::world::sub_app::DimDespawnQueue>();
                            if !despawn_queue.0.contains(&dest_dim) {
                                despawn_queue.0.push(dest_dim);
                            }
                        }
                        None => {
                            // dest_dim has no channel entry — immediate rollback.
                            world.resource_mut::<InFlightMoves>().remove(allocated_id);
                            pending_rollbacks.push(PendingRollback {
                                move_id: allocated_id,
                                source_dim: dim_entity,
                            });
                        }
                    }
                }
                FromDim::Spawned { move_id } => {
                    let entry = world.resource_mut::<InFlightMoves>().remove(move_id);
                    if let Some(entry) = entry {
                        pending_confirms.push(PendingConfirm {
                            move_id,
                            source_dim: entry.source_dim,
                        });
                    }
                }
            }
        }
    }

    // Send deferred rollbacks (from Disconnected / Full sends in the dispatch loop).
    for rb in pending_rollbacks {
        let send_result = {
            let channels = world.resource::<DimChannelsResource>();
            channels.get(rb.source_dim).map(|chan| {
                chan.control_sender
                    .try_send(ToDim::RollbackMove { move_id: rb.move_id })
            })
        };
        if let Some(Err(flume::TrySendError::Full(_))) = send_result {
            let mut despawn_queue =
                world.resource_mut::<mcrs_engine::world::sub_app::DimDespawnQueue>();
            if !despawn_queue.0.contains(&rb.source_dim) {
                despawn_queue.0.push(rb.source_dim);
            }
        }
    }

    // Send deferred confirms.
    for cf in pending_confirms {
        let send_result = {
            let channels = world.resource::<DimChannelsResource>();
            channels.get(cf.source_dim).map(|chan| {
                chan.control_sender
                    .try_send(ToDim::ConfirmMove { move_id: cf.move_id })
            })
        };
        if let Some(Err(flume::TrySendError::Full(_))) = send_result {
            let mut despawn_queue =
                world.resource_mut::<mcrs_engine::world::sub_app::DimDespawnQueue>();
            if !despawn_queue.0.contains(&cf.source_dim) {
                despawn_queue.0.push(cf.source_dim);
            }
        }
    }

    // Advance tick counters on all in-flight entries; send RollbackMove to any
    // that have reached the timeout threshold.
    let timed_out = world.resource_mut::<InFlightMoves>().tick_all();
    for move_id in timed_out {
        let entry = world.resource_mut::<InFlightMoves>().remove(move_id);
        let Some(entry) = entry else { continue };
        let send_result = {
            let channels = world.resource::<DimChannelsResource>();
            channels.get(entry.source_dim).map(|chan| {
                chan.control_sender
                    .try_send(ToDim::RollbackMove { move_id })
            })
        };
        if let Some(Err(flume::TrySendError::Full(_))) = send_result {
            let mut despawn_queue =
                world.resource_mut::<mcrs_engine::world::sub_app::DimDespawnQueue>();
            if !despawn_queue.0.contains(&entry.source_dim) {
                despawn_queue.0.push(entry.source_dim);
            }
        }
    }
}
