use bevy_ecs::message::Messages;
use bevy_ecs::world::World;

use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::dim_outputs;
use crate::muxer::muxer_link::MuxerLink;
use crate::to_muxer::{DimClosed, DimOpened};

pub(crate) fn receive(world: &mut World) {
    let opened: Vec<DimOpened> = world
        .get_resource_mut::<Messages<DimOpened>>()
        .map(|mut messages| messages.drain().collect())
        .unwrap_or_default();
    for DimOpened { dim } in opened {
        world.get_resource_or_init::<DimInputs>().insert(dim);
    }
    let closed: Vec<DimClosed> = world
        .get_resource_mut::<Messages<DimClosed>>()
        .map(|mut messages| messages.drain().collect())
        .unwrap_or_default();
    for DimClosed { dim } in closed {
        if let Some(mut inputs) = world.get_resource_mut::<DimInputs>() {
            inputs.remove(dim);
        }
        if let Some(mut muxer) = world.get_resource_mut::<MuxerLink>() {
            muxer.close_dim(dim);
        }
        dim_outputs::forget_departures(world, |departing| departing.dim == dim);
    }
}
