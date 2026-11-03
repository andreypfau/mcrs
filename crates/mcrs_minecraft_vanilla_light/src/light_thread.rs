use flume::{Receiver, SendError, Sender, TryRecvError, WeakSender};

use bevy_tasks::AsyncComputeTaskPool;
use mcrs_minecraft_engine_world::world::column_drain::DrainWake;
use mcrs_minecraft_light_engine::epoch::PublishedLight;

use crate::from_light::FromLight;
use crate::light_actor::LightActor;
use crate::light_inbox::LightInbox;
use crate::light_state::LightState;

/// The thread of a light actor. It blocks on its inbox, and each time something arrives it
/// takes in what waits, retires the epochs that finished, hands out the next ones and answers
/// the dimension.
///
/// It stops once the dimension's end is gone: its inbox disconnects, or a reply finds nobody.
pub(crate) struct LightThread {
    state: LightState,
    inbox: Receiver<LightInbox>,
    /// The epochs' way into the inbox. Weak, so that the inbox disconnects once the dimension
    /// and the epochs in flight have all let go of it.
    epochs: WeakSender<LightInbox>,
    replies: Sender<FromLight>,
    wake: Option<DrainWake>,
    relit: Vec<PublishedLight>,
    seen: u64,
    reported: u64,
}

impl LightThread {
    /// Starts the actor of `state` and hands back the dimension's end of it. `wake` asks the
    /// dimension to take the actor's replies between ticks.
    pub(crate) fn spawn(state: LightState, wake: Option<DrainWake>) -> LightActor {
        let (inbox_sender, inbox) = flume::unbounded();
        let (replies, replies_receiver) = flume::unbounded();
        let thread = Self {
            state,
            inbox,
            epochs: inbox_sender.downgrade(),
            replies,
            wake,
            relit: Vec::new(),
            seen: 0,
            reported: 0,
        };
        let spawned = std::thread::Builder::new()
            .name("light".to_owned())
            .spawn(move || thread.run());
        if let Err(error) = spawned {
            tracing::error!(%error, "the light actor did not start, so this dimension stays unlit");
        }
        LightActor::new(inbox_sender, replies_receiver)
    }

    fn run(mut self) {
        loop {
            // Edits held back by the intake budget are work in themselves, so the actor only
            // blocks once it has taken everything in.
            if !self.state.has_intake() {
                let Ok(message) = self.inbox.recv() else {
                    return;
                };
                self.handle(message);
            }
            loop {
                match self.inbox.try_recv() {
                    Ok(message) => self.handle(message),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if self.round().is_err() {
                return;
            }
        }
    }

    fn handle(&mut self, message: LightInbox) {
        match message {
            LightInbox::Dimension(message) => {
                self.seen = self.seen.wrapping_add(1);
                self.state.receive(message);
            }
            LightInbox::Finished(finished) => {
                let published = self.state.retire(finished);
                self.relit.extend(published);
            }
        }
    }

    fn round(&mut self) -> Result<(), SendError<FromLight>> {
        self.state.intake();
        // Without a task pool there is no worker to hand a batch to, so the queue keeps it
        // and the light lags until one exists.
        if let Some(pool) = AsyncComputeTaskPool::try_get()
            && let Some(inbox) = self.epochs.upgrade()
        {
            self.state.dispatch(pool, &inbox);
        }

        let mut answered = false;
        if !self.relit.is_empty() {
            let relit = std::mem::take(&mut self.relit);
            self.replies.send(FromLight::Relit(relit))?;
            answered = true;
        }
        // After the light it waited on, so the dimension has that light before it sends the
        // column.
        let settled = self.state.take_settled();
        if !settled.is_empty() {
            self.replies.send(FromLight::Settled(settled))?;
            answered = true;
        }
        if answered && let Some(wake) = &self.wake {
            wake.wake();
        }
        if self.seen != self.reported && self.state.is_idle() {
            self.replies.send(FromLight::Idle { seen: self.seen })?;
            self.reported = self.seen;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use bevy_ecs::entity::Entity;
    use mcrs_minecraft_chunk::VoxelId;
    use mcrs_minecraft_engine_world::world::column_drain::DrainWakes;
    use mcrs_minecraft_light_engine::block::{LightProperties, LightRegistry, SpecialBlocks};
    use mcrs_minecraft_light_engine::level::LightBounds;
    use mcrs_minecraft_light_engine::world::LightWorld;
    use mcrs_voxel::ColumnPos;

    use crate::to_light::ToLight;

    #[test]
    fn the_actor_stops_once_its_dimension_lets_go_of_it() {
        let registry = Arc::new(LightRegistry::new(
            vec![LightProperties::AIR, LightProperties::SOLID],
            SpecialBlocks {
                unloaded: VoxelId(1),
                outside: VoxelId(0),
            },
        ));
        let world = LightWorld::new(registry, LightBounds::new(0, 0));
        let wakes = DrainWakes::default();
        let mut actor = LightThread::spawn(
            LightState::new(world),
            Some(wakes.for_dimension(Entity::PLACEHOLDER)),
        );
        actor.send(ToLight::AwaitColumns(vec![ColumnPos::new(0, 0)]));
        assert_eq!(
            wakes.receiver().recv_timeout(Duration::from_secs(5)),
            Ok(Entity::PLACEHOLDER),
            "the actor answers the column and wakes the dimension"
        );

        drop(actor);

        // The thread holds the only other sender of the wakes.
        let deadline = Instant::now() + Duration::from_secs(5);
        while wakes.receiver().sender_count() > 1 {
            assert!(Instant::now() < deadline, "the actor thread never ended");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
