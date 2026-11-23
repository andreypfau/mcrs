use std::collections::vec_deque::Drain;

use bevy_ecs::resource::Resource;
use mcrs_engine_link::Link;
use mcrs_voxel::ColumnPos;

use mcrs_minecraft_light_engine::world::Edit;

use crate::light_input::LightInput;
use crate::light_output::LightOutput;
use crate::light_stage::LightStage;

#[derive(Resource)]
pub struct LightLink {
    link: Link<LightStage>,
    edits: Vec<Edit>,
    awaits: Vec<ColumnPos>,
    players: Vec<ColumnPos>,
    next_players: Vec<ColumnPos>,
}

impl LightLink {
    pub(crate) fn new(link: Link<LightStage>) -> Self {
        Self {
            link,
            edits: Vec::new(),
            awaits: Vec::new(),
            players: Vec::new(),
            next_players: Vec::new(),
        }
    }

    pub fn push(&mut self, edit: Edit) {
        self.edits.push(edit);
    }

    pub(crate) fn update_players(&mut self, players: impl IntoIterator<Item = ColumnPos>) {
        self.next_players.clear();
        self.next_players.extend(players);
        if self.next_players.is_empty() || self.next_players == self.players {
            return;
        }
        std::mem::swap(&mut self.players, &mut self.next_players);
        self.link.send(LightInput::Players(self.players.clone()));
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.edits.is_empty() && self.awaits.is_empty() && self.link.is_answered()
    }

    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    pub(crate) fn send(&mut self, input: LightInput) {
        self.link.send(input);
    }

    pub(crate) fn await_columns(&mut self, columns: impl IntoIterator<Item = ColumnPos>) {
        self.awaits.extend(columns);
    }

    pub(crate) fn has_unsent(&self) -> bool {
        !self.edits.is_empty() || !self.awaits.is_empty()
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

    pub(crate) fn outputs(&mut self) -> Drain<'_, LightOutput> {
        self.link.outputs()
    }
}
