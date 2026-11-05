use std::collections::vec_deque::Drain;

use bevy_ecs::resource::Resource;
use mcrs_engine_link::Link;
use mcrs_voxel::ColumnPos;

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
    /// Columns that wait for their light, handed over after the edits of the same tick: the
    /// stage answers a column once nothing it was handed is owed to it, so an await that
    /// overtakes the load of a neighbour that landed in the same tick is answered without the
    /// light of that neighbour.
    awaits: Vec<ColumnPos>,
}

impl LightLink {
    pub(crate) fn new(link: Link<LightStage>) -> Self {
        Self {
            link,
            edits: Vec::new(),
            awaits: Vec::new(),
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
        self.edits.is_empty() && self.awaits.is_empty() && self.link.is_answered()
    }

    /// Waits until the stage has answered everything handed to it so far, and publishes none
    /// of it. Edits pushed and not handed to the stage yet are not waited for.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    pub(crate) fn send(&mut self, input: LightInput) {
        self.link.send(input);
    }

    /// Asks for the light of `columns` once it settles, with the edits pushed this tick.
    pub(crate) fn await_columns(&mut self, columns: impl IntoIterator<Item = ColumnPos>) {
        self.awaits.extend(columns);
    }

    pub(crate) fn send_edits(&mut self) {
        if !self.edits.is_empty() {
            let edits = std::mem::take(&mut self.edits);
            self.send(LightInput::Edits(edits));
        }
        if !self.awaits.is_empty() {
            let awaits = std::mem::take(&mut self.awaits);
            self.send(LightInput::AwaitColumns(awaits));
        }
    }

    /// Every output of the stage not published yet, in the order it emitted them.
    pub(crate) fn outputs(&mut self) -> Drain<'_, LightOutput> {
        self.link.outputs()
    }
}
