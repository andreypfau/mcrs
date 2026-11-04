use std::collections::vec_deque::Drain;

use bevy_ecs::entity::Entity;
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
///
/// `T` tags each requested section: the section entity of the dimension, or the section row
/// of a column that the data layer generates whole.
#[derive(Resource)]
pub struct GenerationLink<T: Copy + Send + Sync + 'static = Entity> {
    link: Link<GenerationStage<T>>,
}

impl<T: Copy + Send + Sync + 'static> GenerationLink<T> {
    pub(crate) fn new(link: Link<GenerationStage<T>>) -> Self {
        Self { link }
    }

    /// Waits until the stage has answered every request sent so far, and lands none of it.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    pub(crate) fn send(&mut self, input: GenerationInput<T>) {
        self.link.send(input);
    }

    /// Every output of the stage not landed yet, in the order it emitted them.
    pub(crate) fn outputs(&mut self) -> Drain<'_, GenerationOutput<T>> {
        self.link.outputs()
    }
}
