use std::collections::vec_deque::Drain;

use bevy_ecs::resource::Resource;
use mcrs_engine_link::Link;

use mcrs_minecraft_light_engine::queue::{DEFAULT_PRIORITY, Priority};
use mcrs_minecraft_light_engine::world::Edit;

use crate::light_input::{LightInput, QueuedEdit};
use crate::light_output::LightOutput;
use crate::light_stage::LightStage;

/// A dimension's end of the link to its light stage, which owns the dimension's block and light
/// data and computes its light.
///
/// Push edits from any system ordered before [`LightSet::Intake`](crate::LightSet::Intake),
/// which is where they go to the stage. The ECS holds only what the stage has published.
#[derive(Resource)]
pub struct LightLink {
    link: Link<LightStage>,
    edits: Vec<QueuedEdit>,
}

impl LightLink {
    pub(crate) fn new(link: Link<LightStage>) -> Self {
        Self {
            link,
            edits: Vec::new(),
        }
    }

    pub fn push(&mut self, edit: Edit) {
        self.push_with_priority(edit, DEFAULT_PRIORITY);
    }

    /// Lower is more urgent. A column keeps the most urgent priority it was
    /// given while it waits.
    pub fn push_with_priority(&mut self, edit: Edit, priority: Priority) {
        self.edits.push(QueuedEdit { edit, priority });
    }

    /// Whether the stage has published the light of everything it was handed, as far as the
    /// outputs taken in so far tell.
    ///
    /// Systems whose behaviour has to be reproducible — mob spawning, crop growth, saving a
    /// region — should run only when it is, because light published a tick later than an
    /// edit is fine for rendering and not fine for simulation.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.edits.is_empty() && self.link.is_answered()
    }

    /// Takes in what the stage has sent so far, so that [`Self::is_idle`] counts all of it. It
    /// publishes none of it: what the world holds does not change.
    pub fn take_in(&mut self) {
        self.link.take_in();
    }

    pub(crate) fn send(&mut self, input: LightInput) {
        self.link.send(input);
    }

    pub(crate) fn send_edits(&mut self) {
        if !self.edits.is_empty() {
            let edits = std::mem::take(&mut self.edits);
            self.send(LightInput::Edits(edits));
        }
    }

    /// Every output of the stage not published yet, in the order it emitted them.
    pub(crate) fn outputs(&mut self) -> Drain<'_, LightOutput> {
        self.link.outputs()
    }
}
