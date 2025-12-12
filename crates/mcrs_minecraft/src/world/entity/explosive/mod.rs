use bevy_ecs::component::Component;
use bevy_reflect::Reflect;

pub mod fused;

#[derive(bevy_ecs::bundle::Bundle, Default)]
pub struct ExplosiveBundle {
    pub explosive: Explosive,
    pub explosion_radius: ExplosionRadius,
}

#[derive(Component, Reflect, Default, Debug)]
pub struct Explosive;

/// The radius of the [Explosion] to be created by detonating an [Explosive].
#[derive(Component, Reflect, Default, Debug)]
pub struct ExplosionRadius(pub Option<u32>);
