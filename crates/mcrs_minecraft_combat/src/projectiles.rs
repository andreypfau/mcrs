use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::prelude::*;
use glam::Vec3;
use mcrs_minecraft_damage::{DamageSource, Hit};

use crate::events::DamageRequest;
use crate::health::Health;
use crate::position::CombatPosition;

/// Supported projectile kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectileKind {
    Arrow,
    SpectralArrow,
    Trident,
    Snowball,
    Egg,
    Fireball,
    SmallFireball,
    WitherSkull,
    EnderPearl,
    ExperienceBottle,
    Potion,
}

/// Projectile state component.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Projectile {
    pub kind: ProjectileKind,
    pub owner: Option<Entity>,
    pub velocity: Vec3,
    pub gravity: f32,
    pub drag: f32,
    pub base_damage: f32,
    pub knockback_strength: f32,
    pub in_ground: bool,
    pub life_ticks: u32,
    pub pierce_count: u8,
}

impl Projectile {
    #[must_use]
    pub const fn arrow(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self {
            kind: ProjectileKind::Arrow,
            owner,
            velocity,
            gravity: 0.05,
            drag: 0.01,
            base_damage: 2.0,
            knockback_strength: 0.6,
            in_ground: false,
            life_ticks: 0,
            pierce_count: 0,
        }
    }

    #[must_use]
    pub const fn snowball(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self {
            kind: ProjectileKind::Snowball,
            owner,
            velocity,
            gravity: 0.03,
            drag: 0.01,
            base_damage: 0.0,
            knockback_strength: 0.4,
            in_ground: false,
            life_ticks: 0,
            pierce_count: 0,
        }
    }
}

/// Message emitted when a projectile hits an entity or block.
#[derive(Message, Clone, Debug)]
pub struct ProjectileHitEvent {
    pub projectile: Entity,
    pub hit_entity: Option<Entity>,
    pub hit_position: Vec3,
}

/// Ticks projectile physics and hit detection against target entities.
pub fn tick_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &mut CombatPosition)>,
    targets: Query<(Entity, &CombatPosition), (With<Health>, Without<Projectile>)>,
    mut damage_events: MessageWriter<DamageRequest>,
    mut hit_events: MessageWriter<ProjectileHitEvent>,
) {
    for (proj_entity, mut proj, mut proj_pos) in &mut projectiles {
        proj.life_ticks = proj.life_ticks.saturating_add(1);

        // Despawn after 1200 ticks (1 minute)
        if proj.life_ticks >= 1200 {
            commands.entity(proj_entity).despawn();
            continue;
        }

        if proj.in_ground {
            continue;
        }

        // Apply physics
        proj.velocity.y -= proj.gravity;
        let drag = proj.drag;
        proj.velocity *= 1.0 - drag;

        let start_pos = proj_pos.to_vec3();
        let next_pos = start_pos + proj.velocity;

        // Check entity collision along movement segment
        let mut hit_target: Option<Entity> = None;
        let mut closest_dist_sq = f32::MAX;

        for (target_entity, target_pos) in &targets {
            // Do not hit owner immediately after firing
            if Some(target_entity) == proj.owner && proj.life_ticks < 5 {
                continue;
            }

            // Approximate bounding box hit check (radius 0.6)
            let dist_sq = target_pos.to_vec3().distance_squared(next_pos);
            if dist_sq <= 0.36 && dist_sq < closest_dist_sq {
                closest_dist_sq = dist_sq;
                hit_target = Some(target_entity);
            }
        }

        if let Some(target) = hit_target {
            let speed = proj.velocity.length();
            let damage = (proj.base_damage * speed).max(1.0);

            let source = DamageSource::new(0);

            damage_events.write(DamageRequest::new(
                target,
                Hit::from_source(damage, source),
            ));

            hit_events.write(ProjectileHitEvent {
                projectile: proj_entity,
                hit_entity: Some(target),
                hit_position: next_pos,
            });

            if proj.pierce_count > 0 {
                proj.pierce_count = proj.pierce_count.saturating_sub(1);
            } else {
                match proj.kind {
                    ProjectileKind::Arrow | ProjectileKind::SpectralArrow | ProjectileKind::Trident => {
                        proj.in_ground = true;
                        proj.velocity = Vec3::ZERO;
                    }
                    _ => {
                        commands.entity(proj_entity).despawn();
                        continue;
                    }
                }
            }
        }

        proj_pos.0 = next_pos.as_dvec3();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projectile_builder() {
        let arrow = Projectile::arrow(None, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(arrow.kind, ProjectileKind::Arrow);
        assert!(!arrow.in_ground);
        assert_eq!(arrow.life_ticks, 0);
    }
}
