use std::collections::vec_deque::Drain;

use bevy_ecs::resource::Resource;
use mcrs_engine_link::Link;

use crate::generation_input::GenerationInput;
use crate::generation_output::GenerationOutput;
use crate::generation_stage::GenerationStage;

/// A dimension's end of the link to its generation stage, which climbs the requested columns up
/// their ladder and sends each one back once it is generated.
///
/// The requests go to the stage during the tick, and the generated columns land at the start
/// of a later one.
#[derive(Resource)]
pub struct GenerationLink {
    link: Link<GenerationStage>,
}

impl GenerationLink {
    pub(crate) fn new(link: Link<GenerationStage>) -> Self {
        Self { link }
    }

    /// Waits until the stage has answered every request sent so far, and lands none of it.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    pub(crate) fn send(&mut self, input: GenerationInput) {
        self.link.send(input);
    }

    /// Every output of the stage not landed yet, in the order it emitted them.
    pub(crate) fn outputs(&mut self) -> Drain<'_, GenerationOutput> {
        self.link.outputs()
    }
}
