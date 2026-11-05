use std::collections::VecDeque;
use std::mem;

use bevy_ecs::entity::Entity;
use mcrs_engine_link::{Stage, Step, TaskOutcome};
use mcrs_minecraft_engine_world::session::PlayerSession;
use rustc_hash::FxHashSet;

use crate::bus::OutboundPlayerPacket;
use crate::muxer::column_views::{ColumnViews, DimColumns, DimRequests};

/// A call to the [`ColumnViewStage`], sent over the link of the muxer.
#[derive(Clone, Debug)]
pub enum ColumnViewInput {
    /// What the dimension `dim` published in one tick.
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
}

/// What the [`ColumnViewStage`] hands back to the host.
#[derive(Clone, Debug)]
pub enum ColumnViewOutput {
    /// Packets for the clients, in the order they go out, not stamped yet.
    Packets(Vec<OutboundPlayerPacket>),
    Requests(DimRequests),
}

/// The views of the players as a [`Stage`], so that the placement of the muxer moves them off
/// the server thread.
#[derive(Default)]
pub struct ColumnViewStage {
    views: ColumnViews,
    inputs: VecDeque<ColumnViewInput>,
    blocked: FxHashSet<PlayerSession>,
    out: Vec<OutboundPlayerPacket>,
}

impl ColumnViewStage {
    /// The views that the stage serves.
    #[must_use]
    pub fn views(&self) -> &ColumnViews {
        &self.views
    }

    fn apply(&mut self, input: ColumnViewInput, step: &mut Step<'_, Self>) {
        match input {
            ColumnViewInput::Dim { dim, columns } => {
                self.views.take_dim(dim, columns, &mut self.out);
            }
            ColumnViewInput::Acknowledge {
                session,
                desired_columns_per_tick,
            } => self
                .views
                .acknowledge_batch(session, desired_columns_per_tick),
            ColumnViewInput::CloseDim { dim } => self.views.close_dim(dim),
            ColumnViewInput::Send { blocked } => {
                self.blocked.clear();
                self.blocked.extend(blocked);
                self.views.send(&self.blocked, &mut self.out);
                if !self.out.is_empty() {
                    step.emit(ColumnViewOutput::Packets(mem::take(&mut self.out)));
                }
                for requests in self.views.take_requests() {
                    step.emit(ColumnViewOutput::Requests(requests));
                }
            }
        }
    }
}

impl Stage for ColumnViewStage {
    const NAME: &'static str = "column views";

    type Input = ColumnViewInput;
    type Output = ColumnViewOutput;
    type TaskOutput = ();

    fn receive(&mut self, input: Self::Input) {
        self.inputs.push_back(input);
    }

    fn finish(&mut self, _outcome: TaskOutcome<()>) {}

    fn step(&mut self, step: &mut Step<'_, Self>) {
        while let Some(input) = self.inputs.pop_front() {
            self.apply(input, step);
        }
        // A view that a dimension opened or closed without a send after it still tells its
        // client at once.
        if !self.out.is_empty() {
            step.emit(ColumnViewOutput::Packets(mem::take(&mut self.out)));
        }
    }

    fn has_work(&self) -> bool {
        !self.inputs.is_empty()
    }

    fn is_idle(&self) -> bool {
        self.inputs.is_empty()
    }
}
