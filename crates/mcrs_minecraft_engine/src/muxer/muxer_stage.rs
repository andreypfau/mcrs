use std::collections::VecDeque;
use std::mem;

use bevy_ecs::entity::Entity;
use mcrs_engine_link::{Stage, Step};
use mcrs_engine_muxer::{MuxerInput, Muxers, ViewerBytes};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_replication::PlayerCodec;
use rustc_hash::FxHashSet;

use crate::bus::OutboundPlayerPacket;
use crate::muxer::column_views::{ColumnViews, DimColumns, DimRequests};

/// A call to the [`MuxerStage`], sent over the link of the muxer.
pub enum MuxerCall {
    /// What the dimension `dim` published for the views in one tick.
    Dim { dim: Entity, columns: DimColumns },
    /// The client of `session` took a batch and managed `desired_columns_per_tick`.
    Acknowledge {
        session: PlayerSession,
        desired_columns_per_tick: f32,
    },
    /// The dimension `dim` closed.
    CloseDim { dim: Entity },
    /// The end of what the host took between two ticks: the stage sends the next batches to
    /// every player that is not `blocked`, and the requests for the dimensions.
    Send { blocked: Vec<PlayerSession> },
    /// A call to the entity muxer of one world.
    Entities(MuxerInput<PlayerCodec>),
}

/// What the [`MuxerStage`] hands back to the host.
pub enum MuxerOutput {
    /// Packets of the views for the clients, in the order they go out, not stamped yet.
    Packets(Vec<OutboundPlayerPacket>),
    Requests(DimRequests),
    /// The entity bytes of one viewer.
    Entities(ViewerBytes),
}

/// The views of the players and the entity muxers of the worlds as one [`Stage`], so that the
/// placement of the muxer moves them off the server thread. It applies the calls in the order
/// they came, then emits the entity bytes of each viewer.
#[derive(Default)]
pub struct MuxerStage {
    views: ColumnViews,
    entities: Muxers<PlayerCodec>,
    calls: VecDeque<MuxerCall>,
    blocked: FxHashSet<PlayerSession>,
    out: Vec<OutboundPlayerPacket>,
}

impl MuxerStage {
    /// The views that the stage serves.
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
                self.blocked.clear();
                self.blocked.extend(blocked);
                self.views.send(&self.blocked, &mut self.out);
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
        // A view that a dimension opened or closed without a send after it still tells its
        // client at once.
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
