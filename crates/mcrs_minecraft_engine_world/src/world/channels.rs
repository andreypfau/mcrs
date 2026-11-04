use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::FxHashMap;

/// Non-blocking send capacity for the `Serverbound` channel, which every player of a dimension
/// shares. The host fills it for a whole tick before the dimension drains it, so it bounds the
/// packets a dimension takes a tick, not one player's: a vanilla client sends three or more a
/// tick, so a few hundred slots would cap a dimension at some seventy players. A packet that
/// finds the channel full waits in its session's `PendingInbound`.
pub const TO_DIM_CAPACITY: usize = 4096;

/// The serverbound packets one session may hold for its dimension, behind a full channel,
/// before the host disconnects it. The bound is per session, so a dimension that falls behind
/// costs each player only its own backlog, never another player's.
pub const MAX_HELD_SERVERBOUND: usize = 256;

/// Outbound (dim→host) capacity. Sized to hold ~1 second of peak outbound
/// traffic before the dim's own outbox self-throttles it via backpressure.
pub const FROM_DIM_CAPACITY: usize = 512;

pub struct DimChannelEntry<In, Out> {
    pub serverbound_sender: flume::Sender<In>,
    pub control_sender: flume::Sender<In>,
    pub from_dim_receiver: flume::Receiver<Out>,
}

#[derive(Resource)]
pub struct DimChannels<In, Out>
where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
{
    map: FxHashMap<Entity, DimChannelEntry<In, Out>>,
}

impl<In, Out> Default for DimChannels<In, Out>
where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            map: FxHashMap::default(),
        }
    }
}

impl<In, Out> DimChannels<In, Out>
where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
{
    pub fn insert(
        &mut self,
        key: Entity,
        serverbound_sender: flume::Sender<In>,
        control_sender: flume::Sender<In>,
        from_dim_receiver: flume::Receiver<Out>,
    ) {
        self.map.insert(
            key,
            DimChannelEntry {
                serverbound_sender,
                control_sender,
                from_dim_receiver,
            },
        );
    }

    pub fn get(&self, key: Entity) -> Option<&DimChannelEntry<In, Out>> {
        self.map.get(&key)
    }

    pub fn remove(&mut self, key: Entity) -> Option<DimChannelEntry<In, Out>> {
        self.map.remove(&key)
    }

    /// Sends a lifecycle message to the dimension `key`. The control channel is unbounded,
    /// so a burst of joins never loses one. Returns `false` when the dimension is gone.
    pub fn send_control(&self, key: Entity, msg: In) -> bool {
        self.map
            .get(&key)
            .is_some_and(|entry| entry.control_sender.send(msg).is_ok())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Entity, &DimChannelEntry<In, Out>)> {
        self.map.iter()
    }
}

/// Dim-world resource: holds both `ToDim` receivers so dim systems can drain
/// inbound messages from the host. The control channel is drained first to
/// ensure lifecycle messages (`Spawn`/`Despawn`/`Attach`) always precede any
/// `Serverbound` packet from the same tick.
#[derive(Resource)]
pub struct ToDimReceiver<T: Send + Sync + 'static> {
    pub serverbound: flume::Receiver<T>,
    pub control: flume::Receiver<T>,
}

/// Dim-world resource: holds the `FromDim` sender so dim systems can push
/// outbound messages back to the host without blocking.
#[derive(Resource)]
pub struct FromDimSender<T: Send + Sync + 'static>(pub flume::Sender<T>);
