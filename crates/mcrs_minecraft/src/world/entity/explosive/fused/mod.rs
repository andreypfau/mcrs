use bevy_ecs::component::Component;
use bevy_reflect::Reflect;
use derive_more::{Deref, DerefMut};

pub mod primed_tnt;

#[derive(bevy_ecs::bundle::Bundle, Default)]
pub struct FusedExplosiveBundle {
    pub fused_explosive: FusedExplosive,
    pub fuse_duration: FuseDuration,
    pub ticks_remaining: TicksRemaining,
}

#[derive(Component, Reflect, Default)]
#[component(storage = "SparseSet")]
pub struct FusedExplosive;

/// Whether a [FusedExplosive] is currently primed.
#[derive(Component, Reflect, Default)]
#[component(storage = "SparseSet")]
pub struct IsPrimed;

#[derive(Component, Reflect, Default, Deref, DerefMut)]
pub struct FuseDuration(pub u16);

#[derive(Component, Reflect, Default, Deref, DerefMut)]
pub struct TicksRemaining(pub u16);
