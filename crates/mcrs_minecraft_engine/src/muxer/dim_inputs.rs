use std::mem;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::world::channels::TO_DIM_CAPACITY;
use rustc_hash::FxHashMap;

use crate::bus::InboundPlayerPacket;
use crate::channel_types::ToDim;

/// What the host holds for each dimension world until its next tick, by the label entity of the
/// dimension.
#[derive(Resource, Default)]
pub struct DimInputs(FxHashMap<Entity, PendingInput>);

/// The messages of one dimension for its next tick, each class in the order it was sent.
#[derive(Default)]
pub(crate) struct PendingInput {
    pub(crate) control: Vec<ToDim>,
    pub(crate) serverbound: Vec<InboundPlayerPacket>,
}

impl DimInputs {
    /// Takes input for the dimension `dim` from now on.
    pub fn insert(&mut self, dim: Entity) {
        self.0.entry(dim).or_default();
    }

    /// Stops taking input for the dimension `dim` and drops what it holds.
    pub fn remove(&mut self, dim: Entity) {
        self.0.remove(&dim);
    }

    #[must_use]
    pub fn contains(&self, dim: Entity) -> bool {
        self.0.contains_key(&dim)
    }

    /// Queues a lifecycle message for the dimension `dim`. A control message is never refused,
    /// so a burst of joins never loses one. Returns `false` when the dimension is gone.
    pub fn send_control(&mut self, dim: Entity, message: ToDim) -> bool {
        let Some(pending) = self.0.get_mut(&dim) else {
            return false;
        };
        pending.control.push(message);
        true
    }

    /// Queues a serverbound packet for the dimension `dim`. Hands the packet back when the
    /// dimension is gone or holds [`TO_DIM_CAPACITY`] packets for its next tick already.
    pub fn try_send_serverbound(
        &mut self,
        dim: Entity,
        packet: InboundPlayerPacket,
    ) -> Result<(), InboundPlayerPacket> {
        match self.0.get_mut(&dim) {
            Some(pending) if pending.serverbound.len() < TO_DIM_CAPACITY => {
                pending.serverbound.push(packet);
                Ok(())
            }
            _ => Err(packet),
        }
    }

    /// The control messages that wait for the next tick of `dim`.
    pub fn control(&self, dim: Entity) -> &[ToDim] {
        self.0
            .get(&dim)
            .map_or(&[], |pending| pending.control.as_slice())
    }

    /// The serverbound packets that wait for the next tick of `dim`.
    pub fn serverbound(&self, dim: Entity) -> &[InboundPlayerPacket] {
        self.0
            .get(&dim)
            .map_or(&[], |pending| pending.serverbound.as_slice())
    }

    pub(crate) fn take(&mut self, dim: Entity) -> PendingInput {
        self.0.get_mut(&dim).map(mem::take).unwrap_or_default()
    }
}
