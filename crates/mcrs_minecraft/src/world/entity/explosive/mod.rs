pub mod primed_tnt;

use crate::world::explosion::ExplosionRadius;
use bevy_ecs::component::Component;

#[derive(bevy_ecs::bundle::Bundle, Default)]
pub struct ExplosiveBundle {
    pub explosive: Explosive,
    pub explosion_radius: ExplosionRadius,
}

#[derive(Component, Default, Debug)]
#[component(storage = "SparseSet")]
pub struct Explosive;
