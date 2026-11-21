use bevy_ecs::component::Component;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::world::DeferredWorld;
use mcrs_minecraft_physics::GridRow;

use crate::regrids::Regrids;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_remove = leave_grid)]
pub struct GridSlot(pub(crate) Option<GridRow>);

fn leave_grid(mut world: DeferredWorld, context: HookContext) {
    let Some(row) = (world.get::<GridSlot>(context.entity)).and_then(|slot| slot.0) else {
        return;
    };
    if let Some(mut regrids) = world.get_resource_mut::<Regrids>() {
        regrids.departures.push(row);
    }
}
