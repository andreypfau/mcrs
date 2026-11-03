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
    edits: Vec<QueuedEdit>,
    sent: u64,
    idle_at: u64,
    watch: ActorWatch,
}

/// What the dimension knows of its actor's thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActorWatch {
    Running,
    /// The thread stopped, and the dimension has logged it.
    StopReported,
}

impl LightActor {
    pub(crate) fn new(inbox: Sender<LightInbox>, replies: Receiver<FromLight>) -> Self {
        Self {
            inbox,
            replies,
            edits: Vec::new(),
            sent: 0,
            idle_at: 0,
            watch: ActorWatch::Running,
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
        !self.replies.is_empty()
    }

    /// Whether the actor's thread stopped while the dimension still holds its end, with every
    /// reply it sent taken and the stop not logged yet. Only the dimension letting go ends the
    /// thread on purpose, so a stop seen from here is a panic or a thread that never started.
    pub(crate) fn stopped_unreported(&self) -> bool {
        self.watch == ActorWatch::Running
            && self.replies.is_disconnected()
            && self.replies.is_empty()
    }

    pub(crate) fn report_stop(&mut self) {
        self.watch = ActorWatch::StopReported;
    }

    pub(crate) fn send(&mut self, message: ToLight) {
        // Fails only once the actor has stopped, which `stopped_unreported` catches.
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
        let reply = self.replies.try_recv().ok()?;
        if let FromLight::Idle { seen } = reply {
            self.idle_at = seen;
        }
        Some(reply)
    }
}
