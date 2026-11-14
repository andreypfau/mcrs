use core::marker::PhantomData;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;

use crate::server_ticking::ServerTicking;
use crate::tick::{ClientEntityTick, consume_step, run_world_ticks, tick_entities};
use crate::ticking_area::TickingArea;

/// Ticks each entity in `FixedUpdate` in [`ClientEntityTick`], while the world ticks. An entity
/// ticks when its column is in the area `A`, or when the world has no resource `A`.
pub struct EntityTickPlugin<A: TickingArea>(PhantomData<fn() -> A>);

impl<A: TickingArea> Default for EntityTickPlugin<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A: TickingArea> Plugin for EntityTickPlugin<A> {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerTicking>().add_systems(
            FixedUpdate,
            (tick_entities::<A>, consume_step)
                .chain()
                .in_set(ClientEntityTick)
                .run_if(run_world_ticks),
        );
    }
}
