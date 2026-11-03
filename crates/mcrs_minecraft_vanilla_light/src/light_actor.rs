use std::collections::VecDeque;

use bevy_ecs::resource::Resource;
use flume::{Receiver, Sender};

use mcrs_minecraft_light_engine::queue::{DEFAULT_PRIORITY, Priority};
use mcrs_minecraft_light_engine::world::Edit;

use crate::from_light::FromLight;
use crate::light_inbox::LightInbox;
use crate::to_light::{QueuedEdit, ToLight};

/// A dimension's end of its light actor, the thread that owns the dimension's block and light
/// data and computes its light.
///
/// Push edits from any system ordered before [`LightSet::Intake`](crate::LightSet::Intake),
/// which is where they go to the actor. The ECS holds only what the actor has published.
#[derive(Resource)]
pub struct LightActor {
    inbox: Sender<LightInbox>,
    replies: Receiver<FromLight>,
    /// Replies taken off the channel and not published yet, oldest first.
    held: VecDeque<FromLight>,
    edits: Vec<QueuedEdit>,
    sent: u64,
    idle_at: u64,
}

impl LightActor {
    pub(crate) fn new(inbox: Sender<LightInbox>, replies: Receiver<FromLight>) -> Self {
        Self {
            inbox,
            replies,
            held: VecDeque::new(),
            edits: Vec::new(),
            sent: 0,
            idle_at: 0,
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

    /// Whether the actor has published the light of everything it was handed, as far as the
    /// replies taken so far tell.
    ///
    /// Systems whose behaviour has to be reproducible — mob spawning, crop growth, saving a
    /// region — should run only when it is, because light published a tick later than an
    /// edit is fine for rendering and not fine for simulation.
    pub fn is_idle(&self) -> bool {
        self.edits.is_empty() && self.idle_at == self.sent
    }

    pub(crate) fn has_replies(&self) -> bool {
        !self.held.is_empty() || !self.replies.is_empty()
    }

    /// Takes the replies the actor sent so far off their channel, so that [`Self::is_idle`]
    /// counts all of them. It publishes none of them: what the world holds does not change.
    pub fn take_in_replies(&mut self) {
        for reply in self.replies.try_iter() {
            if let FromLight::Idle { seen } = reply {
                self.idle_at = seen;
            }
            self.held.push_back(reply);
        }
    }

    #[cfg(test)]
    pub(crate) fn replies(&self) -> Receiver<FromLight> {
        self.replies.clone()
    }

    pub(crate) fn send(&mut self, message: ToLight) {
        // Fails only once the actor has stopped, and then no light comes back to wait for.
        if self.inbox.send(LightInbox::Dimension(message)).is_ok() {
            self.sent = self.sent.wrapping_add(1);
        }
    }

    pub(crate) fn send_edits(&mut self) {
        if !self.edits.is_empty() {
            let edits = std::mem::take(&mut self.edits);
            self.send(ToLight::Edits(edits));
        }
    }

    /// The next reply of the actor, in the order it sent them.
    pub(crate) fn receive(&mut self) -> Option<FromLight> {
        if self.held.is_empty() {
            self.take_in_replies();
        }
        self.held.pop_front()
    }
}
