use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::FxHashMap;

/// Non-blocking send capacity for the sheddable `Serverbound` channel.
/// Two-channel split enforces the control reserve structurally: the
/// `Serverbound` channel fills to this bound, leaving `TO_DIM_CONTROL_CAPACITY`
/// slots in the separate control channel always available for lifecycle messages.
pub const TO_DIM_CAPACITY: usize = 224;

/// Capacity of the non-sheddable control channel (`Spawn`/`Despawn`/`Attach`).
/// Sized at 32 to absorb burst lifecycle events without stalling the hub; hard
/// overload (control channel full) triggers whole-dim teardown.
pub const TO_DIM_CONTROL_CAPACITY: usize = 32;

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
