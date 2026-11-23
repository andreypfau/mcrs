use std::collections::VecDeque;
use std::mem;

use bevy_ecs::entity::Entity;
use mcrs_engine_link::{Stage, Step};
use mcrs_engine_muxer::{MuxerInput, Muxers, ViewerBytes};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_replication::EntityCodec;
use rustc_hash::FxHashSet;

use crate::bus::OutboundPlayerPacket;
use crate::muxer::column_views::{ColumnViews, DimColumns, DimRequests};

pub enum MuxerCall {
    Dim {
        dim: Entity,
        columns: DimColumns,
    },
    Acknowledge {
        session: PlayerSession,
        desired_columns_per_tick: f32,
    },
    CloseDim {
        dim: Entity,
    },
    Send {
        blocked: FxHashSet<PlayerSession>,
    },
    Flush,
    Entities(MuxerInput<EntityCodec>),
}

pub enum MuxerOutput {
    Packets(Vec<OutboundPlayerPacket>),
    Requests(DimRequests),
    Entities(ViewerBytes),
}

#[derive(Default)]
pub struct MuxerStage {
    views: ColumnViews,
    entities: Muxers<EntityCodec>,
    calls: VecDeque<MuxerCall>,
    views_out: Vec<OutboundPlayerPacket>,
    changes_out: Vec<OutboundPlayerPacket>,
}

impl MuxerStage {
    #[cfg(feature = "world-access")]
    #[must_use]
    pub fn views(&self) -> &ColumnViews {
        &self.views
    }

    fn apply(&mut self, call: MuxerCall, step: &mut Step<'_, Self>) {
        match call {
            MuxerCall::Dim { dim, columns } => {
                self.views
                    .take_dim(dim, columns, &mut self.views_out, &mut self.changes_out);
            }
            MuxerCall::Acknowledge {
                session,
                desired_columns_per_tick,
            } => self
                .views
                .acknowledge_batch(session, desired_columns_per_tick),
            MuxerCall::CloseDim { dim } => self.views.close_dim(dim),
            MuxerCall::Send { blocked } => {
                self.views.send(&blocked, &mut self.views_out);
                self.flush_outputs(step);
                for requests in self.views.take_requests() {
                    step.emit(MuxerOutput::Requests(requests));
                }
            }
            MuxerCall::Flush => self.flush_outputs(step),
            MuxerCall::Entities(input) => self.entities.apply(input),
        }
    }

    fn flush_outputs(&mut self, step: &mut Step<'_, Self>) {
        if !self.views_out.is_empty() {
            step.emit(MuxerOutput::Packets(mem::take(&mut self.views_out)));
        }
        for bytes in self.entities.take_output() {
            step.emit(MuxerOutput::Entities(bytes));
        }
        if !self.changes_out.is_empty() {
            step.emit(MuxerOutput::Packets(mem::take(&mut self.changes_out)));
        }
    }
}

impl Stage for MuxerStage {
    const NAME: &'static str = "muxer";

    type Input = MuxerCall;
    type Output = MuxerOutput;
    type TaskOutput = ();

    fn receive(&mut self, call: Self::Input) {
        self.calls.push_back(call);
    }

    fn step(&mut self, step: &mut Step<'_, Self>) {
        while let Some(call) = self.calls.pop_front() {
            self.apply(call, step);
        }
    }

    fn has_work(&self) -> bool {
        !self.calls.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    use bevy_ecs::world::World as EcsWorld;
    use bevy_math::DVec3;
    use bevy_tasks::block_on;
    use bytes::Bytes;
    use mcrs_engine_link::{Link, Placement};
    use mcrs_engine_muxer::{
        EntityEvent, EntityId, SubjectEvent, TileDelta, TileDeltas, TileRadius, View, ViewerId,
        WorldId,
    };
    use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
    use mcrs_minecraft_entity_key::TileKey;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_protocol::uuid::Uuid;
    use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockUpdate;
    use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};
    use mcrs_minecraft_replication::{PlayerData, PlayerEntries, PlayerSpawn, Pose};
    use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};

    use crate::bus::PacketPayload;
    use crate::column_change::ColumnChange;
    use crate::column_version::ColumnVersion;
    use crate::player_view::PlayerView;

    fn setup_muxer(placement: Placement) -> (Entity, ColumnPos, Link<MuxerStage>) {
        let mut world = EcsWorld::new();
        let dim = world.spawn_empty().id();
        let world_id = WorldId(0);
        let observer_session = PlayerSession(2);
        let observer_viewer = ViewerId(2);
        let actor_viewer = ViewerId(1);
        let column = ColumnPos::new(0, 0);
        let view = Some(ChunkTrackingView::with_y_bounds(
            SectionPos::new(0, 0, 0),
            1,
            1,
            0,
            0,
        ));
        let mut link = Link::new(MuxerStage::default(), placement);

        link.send(MuxerCall::Entities(MuxerInput::Open {
            world: world_id,
            tile_width: NonZeroU32::new(16).expect("nonzero width"),
        }));
        link.send(MuxerCall::Dim {
            dim,
            columns: DimColumns {
                views: vec![PlayerView {
                    session: observer_session,
                    view,
                }],
                ..DimColumns::default()
            },
        });
        link.send(MuxerCall::Send {
            blocked: FxHashSet::default(),
        });
        block_on(link.answered());
        let _: Vec<_> = link.outputs().collect();

        link.send(MuxerCall::Dim {
            dim,
            columns: DimColumns {
                versions: vec![ColumnVersion {
                    column,
                    packet: Bytes::from_static(b"chunk"),
                }],
                ..DimColumns::default()
            },
        });
        link.send(MuxerCall::Entities(MuxerInput::Subscribe {
            world: world_id,
            viewer: observer_viewer,
            view: View {
                center: TileKey::new(0, 0, 0),
                radius: TileRadius { x: 2, y: 2, z: 2 },
                subject: None,
            },
        }));
        link.send(MuxerCall::Entities(MuxerInput::Publish {
            world: world_id,
            deltas: TileDeltas {
                tick: 0,
                tiles: vec![TileDelta {
                    tile: TileKey::new(0, 0, 0),
                    events: vec![SubjectEvent {
                        subject: EntityId(1),
                        event: EntityEvent::Spawn(
                            PlayerData::Spawn(PlayerSpawn {
                                viewer: actor_viewer,
                                uuid: Uuid::from_u128(1),
                                username: "Alice".to_owned(),
                                properties: Vec::new(),
                                pose: Pose {
                                    position: DVec3::new(0.5, 64.0, 0.5),
                                    look: Default::default(),
                                },
                                game_mode: GameMode::Survival,
                                attributes: Vec::new(),
                                entries: PlayerEntries::default(),
                            })
                            .into(),
                        ),
                    }],
                }],
            },
        }));
        link.send(MuxerCall::Send {
            blocked: FxHashSet::default(),
        });
        block_on(link.answered());
        let _: Vec<_> = link.outputs().collect();
        (dim, column, link)
    }

    fn swing_delta(tick: u64) -> MuxerCall {
        MuxerCall::Entities(MuxerInput::Publish {
            world: WorldId(0),
            deltas: TileDeltas {
                tick,
                tiles: vec![TileDelta {
                    tile: TileKey::new(0, 0, 0),
                    events: vec![SubjectEvent {
                        subject: EntityId(1),
                        event: EntityEvent::Data(
                            PlayerData::Swing {
                                hand: Hand::Main,
                                animation: SwingAnimationKind::Whack,
                                duration: 6,
                            }
                            .into(),
                        ),
                    }],
                }],
            },
        })
    }

    fn block_change(dim: Entity, column: ColumnPos) -> MuxerCall {
        MuxerCall::Dim {
            dim,
            columns: DimColumns {
                changes: vec![ColumnChange {
                    column,
                    payload: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                        block_pos: BlockPos::new(1, 64, 1),
                        block_state_id: mcrs_minecraft_protocol_game::block::BlockStateNumber(1),
                    }),
                    except: None,
                }],
                ..DimColumns::default()
            },
        }
    }

    fn output_kinds(outputs: &[MuxerOutput]) -> Vec<&'static str> {
        outputs
            .iter()
            .filter_map(|out| match out {
                MuxerOutput::Entities(_) => Some("entities"),
                MuxerOutput::Packets(packets)
                    if packets
                        .iter()
                        .any(|packet| matches!(packet.data, PacketPayload::BlockUpdate(_))) =>
                {
                    Some("block_update")
                }
                MuxerOutput::Packets(packets)
                    if packets
                        .iter()
                        .any(|packet| matches!(packet.data, PacketPayload::ChunkUnload(_))) =>
                {
                    Some("chunk_unload")
                }
                MuxerOutput::Packets(packets)
                    if packets.iter().any(|packet| {
                        matches!(packet.data, PacketPayload::SetChunkCacheCenter(_))
                    }) =>
                {
                    Some("cache_center")
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn entity_replication_precedes_block_updates_in_same_tick() {
        let (dim, column, mut link) = setup_muxer(Placement::Inline);

        link.send(block_change(dim, column));
        link.send(swing_delta(1));
        link.send(MuxerCall::Send {
            blocked: FxHashSet::default(),
        });
        link.send(MuxerCall::Dim {
            dim,
            columns: DimColumns {
                views: vec![PlayerView {
                    session: PlayerSession(2),
                    view: Some(ChunkTrackingView::with_y_bounds(
                        SectionPos::new(10, 0, 10),
                        1,
                        1,
                        0,
                        0,
                    )),
                }],
                ..DimColumns::default()
            },
        });
        link.send(MuxerCall::Send {
            blocked: FxHashSet::default(),
        });
        let outputs: Vec<MuxerOutput> = link.outputs().collect();
        assert_eq!(
            output_kinds(&outputs),
            ["entities", "block_update", "chunk_unload"]
        );
    }

    #[test]
    fn views_entities_and_changes_keep_order_across_threaded_steps() {
        let (dim, column, mut link) = setup_muxer(Placement::Thread);

        link.send(swing_delta(1));
        block_on(link.answered());
        link.send(MuxerCall::Dim {
            dim,
            columns: DimColumns {
                views: vec![PlayerView {
                    session: PlayerSession(2),
                    view: Some(ChunkTrackingView::with_y_bounds(
                        SectionPos::new(1, 0, 0),
                        1,
                        1,
                        0,
                        0,
                    )),
                }],
                changes: vec![ColumnChange {
                    column,
                    payload: PacketPayload::BlockUpdate(ClientboundBlockUpdate {
                        block_pos: BlockPos::new(1, 64, 1),
                        block_state_id: mcrs_minecraft_protocol_game::block::BlockStateNumber(1),
                    }),
                    except: None,
                }],
                ..DimColumns::default()
            },
        });
        block_on(link.answered());
        assert!(
            link.outputs().next().is_none(),
            "outputs stay buffered until an explicit Send or Flush batch boundary"
        );

        link.send(MuxerCall::Send {
            blocked: FxHashSet::default(),
        });
        block_on(link.answered());

        let outputs: Vec<MuxerOutput> = link.outputs().collect();
        assert_eq!(
            output_kinds(&outputs),
            ["cache_center", "entities", "block_update"]
        );
    }
}
