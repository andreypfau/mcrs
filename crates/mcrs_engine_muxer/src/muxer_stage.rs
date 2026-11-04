use std::collections::{BTreeMap, VecDeque};

use mcrs_engine_link::{Stage, Step, TaskOutcome};

use crate::entity_id::{ViewerId, WorldId};
use crate::muxer::Muxer;
use crate::muxer_settings::MuxerSettings;
use crate::replication_codec::ReplicationCodec;
use crate::tile_delta::TileDeltas;
use crate::view::View;
use crate::viewer_bytes::ViewerBytes;

/// A call to the [`Muxer`] of one world, sent over a link.
#[non_exhaustive]
pub enum MuxerInput<C: ReplicationCodec> {
    /// Starts the muxer of `world` with `codec` and `settings`. A world that is open already
    /// keeps its muxer.
    Open {
        world: WorldId,
        codec: C,
        settings: MuxerSettings,
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
        match input {
            MuxerInput::Open {
                world,
                codec,
                settings,
            } => {
                self.worlds
                    .entry(world)
                    .or_insert_with(|| Muxer::new(codec, settings));
            }
            MuxerInput::Close { world } => {
                self.worlds.remove(&world);
            }
            MuxerInput::Publish { world, deltas } => {
                if let Some(muxer) = self.worlds.get_mut(&world) {
                    muxer.publish(&deltas);
                }
            }
            MuxerInput::Subscribe {
                world,
                viewer,
                view,
            } => {
                if let Some(muxer) = self.worlds.get_mut(&world) {
                    muxer.subscribe(viewer, view);
                }
            }
            MuxerInput::Unsubscribe { world, viewer } => {
                if let Some(muxer) = self.worlds.get_mut(&world) {
                    muxer.unsubscribe(viewer);
                }
            }
            MuxerInput::Codec { world, input } => {
                if let Some(muxer) = self.worlds.get_mut(&world) {
                    muxer.codec_mut().receive(input);
                }
            }
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

    fn finish(&mut self, _outcome: TaskOutcome<()>) {}

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

    fn is_idle(&self) -> bool {
        self.inputs.is_empty()
    }
}
