use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_damage::{DamageSource, Hit};

use crate::events::DamageRequest;
use crate::health::{Health, KnockbackResistance};
use crate::position::CombatPosition;

const DAMAGE_TYPE_EXPLOSION: u32 = 9;
const DAMAGE_TYPE_PLAYER_EXPLOSION: u32 = 35;

/// A request to detonate an explosion in the world.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ExplosionRequest {
    pub center: DVec3,
    pub power: f32,
    pub cause: Option<Entity>,
    pub create_fire: bool,
}

impl ExplosionRequest {
    #[must_use]
    pub const fn new(center: DVec3, power: f32, cause: Option<Entity>) -> Self {
        Self {
            center,
            power,
            cause,
            create_fire: false,
        }
    }
}

/// Message emitted after an explosion has occurred.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ExplosionOccurred {
    pub center: DVec3,
    pub power: f32,
}

/// A primed TNT entity ticking down to detonation.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PrimedTnt {
    pub fuse_ticks: i32,
    pub power: f32,
    pub owner: Option<Entity>,
}

impl Default for PrimedTnt {
    fn default() -> Self {
        Self {
            fuse_ticks: 80,
            power: 4.0,
            owner: None,
        }
    }
}

impl PrimedTnt {
    #[must_use]
    pub const fn new(fuse_ticks: i32, owner: Option<Entity>) -> Self {
        Self {
            fuse_ticks,
            power: 4.0,
            owner,
        }
    }
}

/// Processes explosion requests: calculates damage and knockback on living entities.
pub fn process_explosions(
    mut requests: MessageReader<ExplosionRequest>,
    entities: Query<(
        Entity,
        &CombatPosition,
        &Health,
        Option<&KnockbackResistance>,
    )>,
    mut damage_writer: MessageWriter<DamageRequest>,
    mut occurred_writer: MessageWriter<ExplosionOccurred>,
) {
    for req in requests.read() {
        occurred_writer.write(ExplosionOccurred {
            center: req.center,
            power: req.power,
        });

        let radius = f64::from(req.power) * 2.0;
        let radius_sq = radius * radius;

        let damage_type = if req.cause.is_some() {
            DAMAGE_TYPE_PLAYER_EXPLOSION
        } else {
            DAMAGE_TYPE_EXPLOSION
        };

        for (entity, pos, _health, _res) in &entities {
            let dist_sq = pos.0.distance_squared(req.center);
            if dist_sq <= radius_sq {
                let dist = dist_sq.sqrt();
                let exposure = (1.0 - dist / radius).clamp(0.0, 1.0) as f32;
                let damage = ((exposure * exposure + exposure) / 2.0) * 7.0 * req.power + 1.0;

                let source = DamageSource::new(damage_type);
                damage_writer.write(DamageRequest::new(
                    entity,
                    Hit::from_source(damage, source),
                ));
            }
        }
    }
}

/// Advances primed TNT entities, triggering explosions when fuse expires.
pub fn tick_primed_tnt(
    mut commands: Commands,
    mut tnt_query: Query<(Entity, &mut PrimedTnt, &CombatPosition)>,
    mut explosion_writer: MessageWriter<ExplosionRequest>,
) {
    for (entity, mut tnt, pos) in &mut tnt_query {
        tnt.fuse_ticks = tnt.fuse_ticks.saturating_sub(1);
        if tnt.fuse_ticks <= 0 {
            explosion_writer.write(ExplosionRequest::new(pos.0, tnt.power, tnt.owner));
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use crate::health::Armor;

    #[test]
    fn test_explosion_damages_nearby_entities() {
        let mut app = App::new();
        app.add_message::<ExplosionRequest>();
        app.add_message::<ExplosionOccurred>();
        app.add_message::<DamageRequest>();
        app.add_systems(Update, process_explosions);

        let _entity = app
            .world_mut()
            .spawn((
                Health::new(20.0),
                CombatPosition::from_coords(2.0, 64.0, 0.0),
                Armor(0.0),
            ))
            .id();

        let mut req_writer = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<ExplosionRequest>>();
        req_writer.write(ExplosionRequest::new(
            DVec3::new(0.0, 64.0, 0.0),
            4.0,
            None,
        ));

        app.update();

        let damage_messages = app
            .world()
            .resource::<bevy_ecs::message::Messages<DamageRequest>>();
        assert_eq!(damage_messages.len(), 1);
    }
}
