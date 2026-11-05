use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroU32;

use mcrs_engine_link::{Stage, Step};

use crate::entity_id::{ViewerId, WorldId};
use crate::muxer::Muxer;
use crate::replication_codec::ReplicationCodec;
use crate::tile_delta::TileDeltas;
use crate::view::View;
use crate::viewer_bytes::ViewerBytes;

/// A call to the [`Muxer`] of one world, sent over a link.
#[non_exhaustive]
pub enum MuxerInput<C: ReplicationCodec> {
    /// Starts the muxer of `world` with `codec` and tiles `tile_width` wide. A world that is
    /// open already keeps its muxer.
    Open {
        world: WorldId,
        codec: C,
        tile_width: NonZeroU32,
    },
    /// Drops the muxer of `world` and the bytes it has not handed out.
    Close {
        world: WorldId,
    },
    Publish {
        world: WorldId,
        deltas: TileDeltas<C::Data>,
    },
    Subscribe {
        world: WorldId,
        viewer: ViewerId,
        view: View,
    },
    Unsubscribe {
        world: WorldId,
        viewer: ViewerId,
    },
    Codec {
        world: WorldId,
        input: C::Input,
    },
}

/// The [`Muxer`]s of many worlds as a [`Stage`], so that a link can place them on a thread of
/// their own. It applies the inputs in the order they came, then emits the bytes of each viewer,
/// world by world in the order of [`WorldId`]. An input for a world that is not open does
/// nothing.
pub struct MuxerStage<C: ReplicationCodec> {
    worlds: BTreeMap<WorldId, Muxer<C>>,
    inputs: VecDeque<MuxerInput<C>>,
}

impl<C: ReplicationCodec> Default for MuxerStage<C> {
    fn default() -> Self {
        Self {
            worlds: BTreeMap::new(),
            inputs: VecDeque::new(),
        }
    }
}

impl<C: ReplicationCodec> MuxerStage<C> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn apply(&mut self, input: MuxerInput<C>) {
        let world = match input {
            MuxerInput::Open {
                world,
                codec,
                tile_width,
            } => {
                self.worlds
                    .entry(world)
                    .or_insert_with(|| Muxer::new(codec, tile_width));
                return;
            }
            MuxerInput::Close { world } => {
                self.worlds.remove(&world);
                return;
            }
            MuxerInput::Publish { world, .. }
            | MuxerInput::Subscribe { world, .. }
            | MuxerInput::Unsubscribe { world, .. }
            | MuxerInput::Codec { world, .. } => world,
        };
        let Some(muxer) = self.worlds.get_mut(&world) else {
            return;
        };
        match input {
            MuxerInput::Publish { deltas, .. } => muxer.publish(&deltas),
            MuxerInput::Subscribe { viewer, view, .. } => muxer.subscribe(viewer, view),
            MuxerInput::Unsubscribe { viewer, .. } => muxer.unsubscribe(viewer),
            MuxerInput::Codec { input, .. } => muxer.codec_mut().receive(input),
            MuxerInput::Open { .. } | MuxerInput::Close { .. } => {}
        }
    }
}

impl<C> Stage for MuxerStage<C>
where
    C: ReplicationCodec + Send + 'static,
    C::Data: Send + 'static,
    C::Input: Send + 'static,
{
    const NAME: &'static str = "muxer";

    type Input = MuxerInput<C>;
    type Output = ViewerBytes;
    type TaskOutput = ();

    fn receive(&mut self, input: Self::Input) {
        self.inputs.push_back(input);
    }

    fn step(&mut self, step: &mut Step<'_, Self>) {
        while let Some(input) = self.inputs.pop_front() {
            self.apply(input);
        }
        for muxer in self.worlds.values_mut() {
            for output in muxer.take_output() {
                step.emit(output);
            }
        }
    }

    fn has_work(&self) -> bool {
        !self.inputs.is_empty()
    }
}
