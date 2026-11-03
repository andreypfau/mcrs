use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::schedule::ScheduleLabel;

/// Hands the host what a dimension's tick wrote for the clients. It runs at the end of every
/// tick, and between ticks for a dimension whose full channel held packets back.
///
/// It moves packets and changes nothing a tick reads, so it may run at any moment between
/// ticks. The writes of the generation and light steps land only at the start of a tick.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutboxDrain;

/// Where the server loop waits for the dimensions to ask for an [`OutboxDrain`] between ticks.
#[derive(Resource)]
pub struct DrainWakes {
    sender: flume::Sender<Entity>,
    receiver: flume::Receiver<Entity>,
}

impl Default for DrainWakes {
    fn default() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }
}

impl DrainWakes {
    /// The wake of the dimension whose sub-app is labelled with `dim`.
    pub fn for_dimension(&self, dim: Entity) -> DrainWake {
        DrainWake {
            dim,
            sender: self.sender.clone(),
        }
    }

    /// The labels of the dimensions that asked for a drain, as they ask.
    pub fn receiver(&self) -> &flume::Receiver<Entity> {
        &self.receiver
    }
}

/// Asks the server loop to run this dimension's [`OutboxDrain`] before the next tick.
#[derive(Resource, Clone)]
pub struct DrainWake {
    dim: Entity,
    sender: flume::Sender<Entity>,
}

impl DrainWake {
    pub fn wake(&self) {
        // Fails only once the host has gone, and then nobody drains anything.
        let _ = self.sender.send(self.dim);
    }
}
