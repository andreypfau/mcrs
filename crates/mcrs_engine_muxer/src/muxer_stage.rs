use std::collections::VecDeque;

use mcrs_engine_link::{Stage, Step, TaskOutcome};

use crate::entity_id::ViewerId;
use crate::muxer::Muxer;
use crate::replication_codec::ReplicationCodec;
use crate::tile_delta::TileDeltas;
use crate::view::View;
use crate::viewer_bytes::ViewerBytes;

/// A call to a [`Muxer`], sent over a link.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum MuxerInput<D> {
    Publish(TileDeltas<D>),
    Subscribe { viewer: ViewerId, view: View },
    Unsubscribe { viewer: ViewerId },
}

/// A [`Muxer`] as a [`Stage`], so that a link can place it on a thread of its own. It applies
/// the inputs in the order they came, then emits the bytes of each viewer.
pub struct MuxerStage<C: ReplicationCodec> {
    muxer: Muxer<C>,
    inputs: VecDeque<MuxerInput<C::Data>>,
}

impl<C: ReplicationCodec> MuxerStage<C> {
    #[must_use]
    pub fn new(muxer: Muxer<C>) -> Self {
        Self {
            muxer,
            inputs: VecDeque::new(),
        }
    }
}

impl<C> Stage for MuxerStage<C>
where
    C: ReplicationCodec + Send + 'static,
    C::Data: Send + 'static,
{
    const NAME: &'static str = "muxer";

    type Input = MuxerInput<C::Data>;
    type Output = ViewerBytes;
    type TaskOutput = ();

    fn receive(&mut self, input: Self::Input) {
        self.inputs.push_back(input);
    }

    fn finish(&mut self, _outcome: TaskOutcome<()>) {}

    fn step(&mut self, step: &mut Step<'_, Self>) {
        while let Some(input) = self.inputs.pop_front() {
            match input {
                MuxerInput::Publish(deltas) => self.muxer.publish(&deltas),
                MuxerInput::Subscribe { viewer, view } => self.muxer.subscribe(viewer, view),
                MuxerInput::Unsubscribe { viewer } => self.muxer.unsubscribe(viewer),
            }
        }
        for output in self.muxer.take_output() {
            step.emit(output);
        }
    }

    fn has_work(&self) -> bool {
        !self.inputs.is_empty()
    }

    fn is_idle(&self) -> bool {
        self.inputs.is_empty()
    }
}
