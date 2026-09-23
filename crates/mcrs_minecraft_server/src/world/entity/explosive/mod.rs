pub mod primed_tnt;

use bevy_ecs::component::Component;
use mcrs_minecraft_level::explosion::ExplosionRadius;

#[derive(bevy_ecs::bundle::Bundle, Default)]
pub struct ExplosiveBundle {
    pub explosive: Explosive,
    pub explosion_radius: ExplosionRadius,
}

#[derive(Component, Default, Debug)]
#[component(storage = "SparseSet")]
pub struct Explosive;
