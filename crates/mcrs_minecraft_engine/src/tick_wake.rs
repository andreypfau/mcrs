use std::time::Instant;

use bevy_ecs::resource::Resource;
use flume::{Receiver, Sender};

/// Where the host waits between ticks. A part of the host that hands out bytes for the clients
/// off the tick, such as the muxer thread, wakes the wait through [`Self::waker`].
#[derive(Resource)]
pub struct TickWake {
    /// Keeps the channel open, so that the wait lasts until its deadline when nothing wakes it.
    sender: Sender<()>,
    receiver: Receiver<()>,
}

impl Default for TickWake {
    fn default() -> Self {
        // One wake stands for every arrival since the last wait, so the host drains them at once.
        let (sender, receiver) = flume::bounded(1);
        Self { sender, receiver }
    }
}

impl TickWake {
    #[must_use]
    pub fn waker(&self) -> Sender<()> {
        self.sender.clone()
    }

    /// Waits until a wake arrives or `deadline` passes, and tells whether a wake arrived.
    pub fn wait(&self, deadline: Instant) -> bool {
        self.receiver.recv_deadline(deadline).is_ok()
    }
}
