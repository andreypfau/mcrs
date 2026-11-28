use std::time::Instant;

use bevy_ecs::resource::Resource;
use flume::{Receiver, Sender};

#[derive(Resource)]
pub struct TickWake {
    sender: Sender<()>,
    receiver: Receiver<()>,
}

impl Default for TickWake {
    fn default() -> Self {
        let (sender, receiver) = flume::bounded(1);
        Self { sender, receiver }
    }
}

impl TickWake {
    #[must_use]
    pub fn waker(&self) -> Sender<()> {
        self.sender.clone()
    }

    pub fn wait(&self, deadline: Instant) -> bool {
        self.receiver.recv_deadline(deadline).is_ok()
    }
}
