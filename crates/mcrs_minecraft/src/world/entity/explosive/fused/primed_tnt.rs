use crate::world::entity::explosive::fused::{
    FuseDuration, FusedExplosiveBundle, IsPrimed, TicksRemaining,
};
use crate::world::entity::explosive::{ExplosionRadius, ExplosiveBundle};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_reflect::Reflect;

pub const DEFAULT_EXPLOSION_RADIUS: u32 = 4;
pub const DEFAULT_FUSE_DURATION: u16 = 80;

#[derive(Bundle)]
pub struct PrimedTntBundle {
    pub explosive: ExplosiveBundle,
    pub fused_explosive: FusedExplosiveBundle,
    pub marker: PrimedTnt,
    pub is_primed: IsPrimed,
}

impl Default for PrimedTntBundle {
    fn default() -> Self {
        Self {
            explosive: ExplosiveBundle {
                explosion_radius: ExplosionRadius(Some(DEFAULT_EXPLOSION_RADIUS)),
                ..Default::default()
            },
            fused_explosive: FusedExplosiveBundle {
                fuse_duration: FuseDuration(DEFAULT_FUSE_DURATION),
                ticks_remaining: TicksRemaining(DEFAULT_FUSE_DURATION),
                ..Default::default()
            },
            marker: PrimedTnt,
            is_primed: IsPrimed,
        }
    }
}

#[derive(Component, Debug, Default, Reflect)]
pub struct PrimedTnt;

/// The detonator entity
#[derive(Component, Debug, Reflect)]
pub struct Detonator {
    pub entity: Entity,
}
