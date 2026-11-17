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
    out: Vec<OutboundPlayerPacket>,
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
                self.views.take_dim(dim, columns, &mut self.out);
            }
            MuxerCall::Acknowledge {
                session,
                desired_columns_per_tick,
            } => self
                .views
                .acknowledge_batch(session, desired_columns_per_tick),
            MuxerCall::CloseDim { dim } => self.views.close_dim(dim),
            MuxerCall::Send { blocked } => {
                self.views.send(&blocked, &mut self.out);
                if !self.out.is_empty() {
                    step.emit(MuxerOutput::Packets(mem::take(&mut self.out)));
                }
                for requests in self.views.take_requests() {
                    step.emit(MuxerOutput::Requests(requests));
                }
            }
            MuxerCall::Entities(input) => self.entities.apply(input),
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
        if !self.out.is_empty() {
            step.emit(MuxerOutput::Packets(mem::take(&mut self.out)));
        }
        for bytes in self.entities.take_output() {
            step.emit(MuxerOutput::Entities(bytes));
        }
    }

    fn has_work(&self) -> bool {
        !self.calls.is_empty()
    }
}
