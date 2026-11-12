use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::prelude::*;
use glam::{DVec3, Vec3};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_damage::{DamageSource, DamageTypeFlags, Hit};
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_physics::{MobChanges, hurt_mob};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockPos;

use crate::damage_rules::DamageTypeRulesResource;
use crate::damage_types;
use crate::events::DamageRequest;
use crate::explosions::ExplosionRequest;
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

impl ProjectileKind {
    #[must_use]
    pub const fn display_key(self) -> &'static str {
        match self {
            Self::Arrow => "item.minecraft.arrow",
            Self::SpectralArrow => "item.minecraft.spectral_arrow",
            Self::Trident => "item.minecraft.trident",
            Self::Snowball => "item.minecraft.snowball",
            Self::Egg => "item.minecraft.egg",
            Self::Fireball => "entity.minecraft.fireball",
            Self::SmallFireball => "entity.minecraft.small_fireball",
            Self::WitherSkull => "entity.minecraft.wither_skull",
            Self::EnderPearl => "item.minecraft.ender_pearl",
            Self::ExperienceBottle => "item.minecraft.experience_bottle",
            Self::Potion => "item.minecraft.splash_potion",
        }
    }
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
    pub const fn new(kind: ProjectileKind, owner: Option<Entity>, velocity: Vec3) -> Self {
        let (gravity, drag, base_damage, knockback_strength) = match kind {
            ProjectileKind::Arrow | ProjectileKind::SpectralArrow => (0.05, 0.01, 2.0, 0.6),
            ProjectileKind::Trident => (0.05, 0.01, 8.0, 0.6),
            ProjectileKind::Snowball | ProjectileKind::Egg => (0.03, 0.01, 0.0, 0.4),
            ProjectileKind::Fireball => (0.0, 0.0, 6.0, 0.0),
            ProjectileKind::SmallFireball => (0.0, 0.0, 5.0, 0.0),
            ProjectileKind::WitherSkull => (0.0, 0.0, 8.0, 0.0),
            ProjectileKind::EnderPearl => (0.03, 0.01, 0.0, 0.0),
            ProjectileKind::ExperienceBottle | ProjectileKind::Potion => (0.07, 0.01, 0.0, 0.0),
        };
        Self {
            kind,
            owner,
            velocity,
            gravity,
            drag,
            base_damage,
            knockback_strength,
            in_ground: false,
            life_ticks: 0,
            pierce_count: 0,
        }
    }

    #[must_use]
    pub const fn arrow(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::Arrow, owner, velocity)
    }

    #[must_use]
    pub const fn snowball(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::Snowball, owner, velocity)
    }

    #[must_use]
    pub const fn fireball(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::Fireball, owner, velocity)
    }

    #[must_use]
    pub const fn small_fireball(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::SmallFireball, owner, velocity)
    }

    #[must_use]
    pub const fn trident(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::Trident, owner, velocity)
    }

    #[must_use]
    pub const fn wither_skull(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::WitherSkull, owner, velocity)
    }

    #[must_use]
    pub const fn ender_pearl(owner: Option<Entity>, velocity: Vec3) -> Self {
        Self::new(ProjectileKind::EnderPearl, owner, velocity)
    }
}

impl ProjectileKind {
    const fn damage_type(self) -> u32 {
        match self {
            Self::Arrow | Self::SpectralArrow => damage_types::ARROW,
            Self::Trident => damage_types::TRIDENT,
            Self::Fireball | Self::SmallFireball => damage_types::FIREBALL,
            Self::WitherSkull => damage_types::WITHER_SKULL,
            Self::Snowball
            | Self::Egg
            | Self::EnderPearl
            | Self::ExperienceBottle
            | Self::Potion => damage_types::THROWN,
        }
    }

    const fn hurts(self) -> bool {
        matches!(
            self,
            Self::Arrow
                | Self::SpectralArrow
                | Self::Trident
                | Self::Fireball
                | Self::SmallFireball
                | Self::WitherSkull
        )
    }

    const fn sticks(self) -> bool {
        matches!(self, Self::Arrow | Self::SpectralArrow | Self::Trident)
    }

    const fn explosion_power(self) -> Option<f32> {
        match self {
            Self::Fireball | Self::WitherSkull => Some(1.0),
            Self::SmallFireball => Some(0.5),
            _ => None,
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

enum ProjectileTarget {
    Block(DVec3),
    Entity(Entity),
    Mob(Placed),
}

/// Ticks projectile physics and hit detection against target entities.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: the world view, the targets, the owners, the mob rows and the writers are separate parameters"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the flight, collision and impact of a projectile are one pass"
)]
pub fn tick_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(
        Entity,
        &mut Projectile,
        &mut CombatPosition,
        Option<&InDimension>,
    )>,
    targets: Query<
        (Entity, &CombatPosition, Option<&InDimension>),
        (With<Health>, Without<Projectile>),
    >,
    owners: Query<(&EntityUuid, &Transform)>,
    blocks: Option<Res<Blocks>>,
    world: WorldView,
    mut rows: Option<ResMut<EntityRows>>,
    mut changes: Option<ResMut<MobChanges>>,
    rules: Option<Res<DamageTypeRulesResource>>,
    clock: Option<Res<BirthClock>>,
    mut damage_events: MessageWriter<DamageRequest>,
    mut hit_events: MessageWriter<ProjectileHitEvent>,
    mut explosions: Option<MessageWriter<ExplosionRequest>>,
) {
    for (proj_entity, mut proj, mut proj_pos, dimension) in &mut projectiles {
        proj.life_ticks = proj.life_ticks.saturating_add(1);

        if proj.life_ticks >= 1200 {
            commands.entity(proj_entity).despawn();
            continue;
        }

        if proj.in_ground {
            continue;
        }

        proj.velocity.y -= proj.gravity;
        let drag = proj.drag;
        proj.velocity *= 1.0 - drag;

        let start_pos = proj_pos.to_vec3();
        let next_pos = start_pos + proj.velocity;
        let mut target = None;

        if let (Some(dimension), Some(blocks)) = (dimension, blocks.as_deref())
            && let Some(view) = world.get(dimension.0)
            && let Some(hit) = block_hit(&view, blocks, start_pos.as_dvec3(), next_pos.as_dvec3())
        {
            let t = (hit - start_pos.as_dvec3()).length()
                / (next_pos.as_dvec3() - start_pos.as_dvec3()).length();
            target = Some((t as f32, ProjectileTarget::Block(hit)));
        }

        let projectile_dimension = dimension.map(|dimension| dimension.0);
        for (target_entity, target_pos, target_dimension) in &targets {
            if target_entity == proj_entity || Some(target_entity) == proj.owner {
                continue;
            }
            if target_dimension.map(|dimension| dimension.0) != projectile_dimension {
                continue;
            }
            let half = 0.3;
            let min = target_pos.to_vec3() - Vec3::new(half, 0.0, half);
            let max = target_pos.to_vec3() + Vec3::new(half, 1.8, half);
            if let Some(t) = segment_box(start_pos, next_pos, min, max)
                && target.as_ref().is_none_or(|(closest, _)| t < *closest)
            {
                target = Some((t, ProjectileTarget::Entity(target_entity)));
            }
        }

        if let Some(rows) = rows.as_deref() {
            for (at, state) in rows.living_rows() {
                if !state.living.is_some_and(|living| living.is_alive()) {
                    continue;
                }
                let Some(row) = rows
                    .tile(at.tile)
                    .and_then(|tile| tile.rows().find(|row| row.id == at.id))
                else {
                    continue;
                };
                let scale = state.living.map_or(1.0, |living| living.attributes.scale) as f32;
                let dimensions = row.motion.dimensions.scaled(scale);
                let half = dimensions.width / 2.0;
                let position = state.body.position.as_vec3();
                let min = position - Vec3::new(half, 0.0, half);
                let max = position + Vec3::new(half, dimensions.height, half);
                if let Some(t) = segment_box(start_pos, next_pos, min, max)
                    && target.as_ref().is_none_or(|(closest, _)| t < *closest)
                {
                    target = Some((t, ProjectileTarget::Mob(at)));
                }
            }
        }

        let Some((hit_t, target)) = target else {
            proj_pos.0 = next_pos.as_dvec3();
            continue;
        };
        let hit_position = start_pos.lerp(next_pos, hit_t.clamp(0.0, 1.0));

        match target {
            ProjectileTarget::Block(hit) => {
                proj_pos.0 = hit;
                hit_events.write(ProjectileHitEvent {
                    projectile: proj_entity,
                    hit_entity: None,
                    hit_position: hit.as_vec3(),
                });
                if let Some(power) = proj.kind.explosion_power()
                    && let Some(explosions) = explosions.as_mut()
                {
                    explosions.write(ExplosionRequest::new(hit, power, proj.owner));
                }
                if proj.kind.sticks() {
                    proj.in_ground = true;
                    proj.velocity = Vec3::ZERO;
                } else {
                    commands.entity(proj_entity).despawn();
                }
            }
            ProjectileTarget::Entity(target) => {
                if let Some(power) = proj.kind.explosion_power()
                    && let Some(explosions) = explosions.as_mut()
                {
                    explosions.write(ExplosionRequest::new(
                        next_pos.as_dvec3(),
                        power,
                        proj.owner,
                    ));
                }
                proj_pos.0 = hit_position.as_dvec3();
                let damage = projectile_damage(&proj);
                if proj.kind.hurts() && damage > 0.0 {
                    let source = projectile_source(
                        proj.as_ref(),
                        proj_entity,
                        hit_position.as_dvec3(),
                        &owners,
                    );
                    damage_events
                        .write(DamageRequest::new(target, Hit::from_source(damage, source)));
                }
                projectile_impact(&mut commands, &mut proj, proj_entity);
                hit_events.write(ProjectileHitEvent {
                    projectile: proj_entity,
                    hit_entity: Some(target),
                    hit_position,
                });
            }
            ProjectileTarget::Mob(at) => {
                if let Some(power) = proj.kind.explosion_power()
                    && let Some(explosions) = explosions.as_mut()
                {
                    explosions.write(ExplosionRequest::new(
                        next_pos.as_dvec3(),
                        power,
                        proj.owner,
                    ));
                }
                if let (Some(rows), Some(changes)) = (rows.as_deref_mut(), changes.as_deref_mut()) {
                    let damage = projectile_damage(&proj);
                    if proj.kind.hurts() && damage > 0.0 {
                        let flags = rules.as_deref().map_or(
                            DamageTypeFlags::builtin_for(proj.kind.damage_type()),
                            |rules| rules.0.flags(proj.kind.damage_type()),
                        );
                        let source = projectile_source(
                            proj.as_ref(),
                            proj_entity,
                            hit_position.as_dvec3(),
                            &owners,
                        );
                        let hit = Hit::from_source(damage, source).with_flags(flags);
                        hurt_mob(
                            rows,
                            changes,
                            at,
                            hit,
                            clock.as_deref().map_or(0, |clock| clock.0),
                        );
                    }
                }
                proj_pos.0 = hit_position.as_dvec3();
                projectile_impact(&mut commands, &mut proj, proj_entity);
                hit_events.write(ProjectileHitEvent {
                    projectile: proj_entity,
                    hit_entity: None,
                    hit_position,
                });
            }
        }
    }
}

fn projectile_damage(proj: &Projectile) -> f32 {
    match proj.kind {
        ProjectileKind::Arrow | ProjectileKind::SpectralArrow | ProjectileKind::Trident => {
            proj.base_damage * proj.velocity.length()
        }
        _ => proj.base_damage,
    }
}

fn projectile_source(
    proj: &Projectile,
    projectile: Entity,
    hit_position: DVec3,
    owners: &Query<(&EntityUuid, &Transform)>,
) -> DamageSource {
    let mut source = DamageSource::new(proj.kind.damage_type());
    if let Some(owner) = proj.owner
        && let Ok((uuid, _)) = owners.get(owner)
    {
        source = source.with_causing(EntityKey(uuid.0.as_u128()));
    }
    if let Ok((uuid, _)) = owners.get(projectile) {
        source = source.with_direct(EntityKey(uuid.0.as_u128()));
    }
    source.source_position = Some(hit_position);
    source
}

fn projectile_impact(commands: &mut Commands, proj: &mut Projectile, projectile: Entity) {
    if proj.pierce_count > 0 {
        proj.pierce_count = proj.pierce_count.saturating_sub(1);
    } else if proj.kind.sticks() {
        proj.in_ground = true;
        proj.velocity = Vec3::ZERO;
    } else {
        commands.entity(projectile).despawn();
    }
}

fn segment_box(from: Vec3, to: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let delta = to - from;
    let axes = [
        (from.x, delta.x, min.x, max.x),
        (from.y, delta.y, min.y, max.y),
        (from.z, delta.z, min.z, max.z),
    ];
    let mut enter = 0.0_f32;
    let mut leave = 1.0_f32;
    for (origin, delta, min, max) in axes {
        if delta.abs() <= f32::EPSILON {
            if origin < min || origin > max {
                return None;
            }
            continue;
        }
        let first = (min - origin) / delta;
        let second = (max - origin) / delta;
        enter = enter.max(first.min(second));
        leave = leave.min(first.max(second));
        if enter > leave {
            return None;
        }
    }
    Some(enter)
}

fn block_hit(view: &impl BlockVolume, blocks: &Blocks, from: DVec3, to: DVec3) -> Option<DVec3> {
    let delta = to - from;
    let distance = delta.length();
    if distance <= f64::EPSILON {
        return None;
    }
    let steps = (distance / 0.25).ceil() as u32;
    for step in 1..=steps {
        let point = from + delta * (f64::from(step) / f64::from(steps));
        let Ok(state) = view.block(BlockPos::new(
            point.x.floor() as i32,
            point.y.floor() as i32,
            point.z.floor() as i32,
        )) else {
            continue;
        };
        let definition = blocks.state(state);
        for shape in blocks.shape(definition.collision_shape) {
            let local = point - DVec3::new(point.x.floor(), point.y.floor(), point.z.floor());
            if local.x >= f64::from(shape.min.x)
                && local.x <= f64::from(shape.max.x)
                && local.y >= f64::from(shape.min.y)
                && local.y <= f64::from(shape.max.y)
                && local.z >= f64::from(shape.min.z)
                && local.z <= f64::from(shape.max.z)
            {
                return Some(point);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, FixedUpdate};
    use mcrs_minecraft_engine_physics::{mob_body, mob_row};
    use mcrs_minecraft_entity::EntityTypeData;
    use mcrs_minecraft_entity_key::EntityId;

    #[test]
    fn test_projectile_builder() {
        let arrow = Projectile::arrow(None, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(arrow.kind, ProjectileKind::Arrow);
        assert!(!arrow.in_ground);
        assert_eq!(arrow.life_ticks, 0);
    }

    #[test]
    fn a_projectile_does_not_hit_its_shooter() {
        let mut app = App::new();
        app.add_plugins(crate::CombatPlugin);
        let shooter = app
            .world_mut()
            .spawn((
                Health::default(),
                CombatPosition::from_coords(0.0, 64.5, 0.0),
                EntityUuid::default(),
                Transform::from_translation(DVec3::new(0.0, 64.5, 0.0)),
            ))
            .id();
        let arrow = app
            .world_mut()
            .spawn((
                Projectile::arrow(Some(shooter), Vec3::new(4.0, 0.0, 0.0)),
                CombatPosition::from_coords(0.0, 64.5, 0.0),
                EntityUuid::default(),
                Transform::from_translation(DVec3::new(0.0, 64.5, 0.0)),
            ))
            .id();

        app.world_mut().run_schedule(FixedUpdate);

        assert!(
            app.world().get_entity(arrow).is_ok(),
            "the arrow passes through its shooter"
        );
        let damage = app
            .world()
            .resource::<bevy_ecs::message::Messages<DamageRequest>>();
        assert!(
            damage.is_empty(),
            "the shooter takes no damage from its arrow"
        );
    }

    #[test]
    fn a_projectile_attributes_the_hit_to_itself_and_its_shooter() {
        let mut app = App::new();
        app.add_plugins(crate::CombatPlugin);
        let shooter = app
            .world_mut()
            .spawn((
                Health::default(),
                CombatPosition::from_coords(0.0, 64.5, 0.0),
                EntityUuid::default(),
                Transform::from_translation(DVec3::new(0.0, 64.5, 0.0)),
            ))
            .id();
        let target = app
            .world_mut()
            .spawn((
                Health::default(),
                CombatPosition::from_coords(2.0, 64.0, 0.0),
                EntityUuid::default(),
            ))
            .id();
        let arrow = app
            .world_mut()
            .spawn((
                Projectile::arrow(Some(shooter), Vec3::new(4.0, 0.0, 0.0)),
                CombatPosition::from_coords(0.0, 64.5, 0.0),
                EntityUuid::default(),
                Transform::from_translation(DVec3::new(0.0, 64.5, 0.0)),
            ))
            .id();
        let shooter_key = EntityKey(
            app.world()
                .get::<EntityUuid>(shooter)
                .expect("the shooter has a uuid")
                .0
                .as_u128(),
        );
        let arrow_key = EntityKey(
            app.world()
                .get::<EntityUuid>(arrow)
                .expect("the arrow has a uuid")
                .0
                .as_u128(),
        );

        app.world_mut().run_schedule(FixedUpdate);

        let damage = app
            .world()
            .resource::<bevy_ecs::message::Messages<DamageRequest>>();
        assert!(!damage.is_empty(), "the arrow hit the target");
        let request = damage
            .get_message(0)
            .map(|(message, _)| message)
            .expect("the damage request is readable");
        assert_eq!(request.target, target);
        assert_eq!(request.hit.source.causing_entity, Some(shooter_key));
        assert_eq!(request.hit.source.direct_entity, Some(arrow_key));
        let position = app
            .world()
            .get::<CombatPosition>(arrow)
            .expect("a stuck arrow keeps its position")
            .0;
        assert!(
            (position.x - 1.7).abs() < 0.01,
            "the stuck arrow stops at the hit point, got {position:?}"
        );
    }

    #[test]
    fn a_projectile_hits_a_mob_row_on_its_path() {
        let mut app = App::new();
        app.add_plugins(crate::CombatPlugin);
        let mut rows = EntityRows::default();
        let kind = &EntityTypeData::PIG;
        let row = mob_row(kind);
        rows.spawn(
            EntityId(2),
            mob_body(kind, &row, EntityKey(2), DVec3::new(2.0, 64.0, 0.0)),
            row,
        );
        let at = rows.living_rows().next().expect("the pig row exists").0;
        app.insert_resource(rows);
        app.insert_resource(MobChanges::default());
        app.insert_resource(BirthClock(1));
        app.world_mut().spawn((
            Projectile::arrow(None, Vec3::new(4.0, 0.0, 0.0)),
            CombatPosition::from_coords(0.0, 64.5, 0.0),
        ));

        app.world_mut().run_schedule(FixedUpdate);

        let position = app
            .world_mut()
            .query::<&CombatPosition>()
            .single(app.world())
            .map(|position| position.0);
        assert!(position.is_ok(), "the arrow stays in flight");
        let rows = app.world().resource::<EntityRows>();
        assert_eq!(rows.living_rows().count(), 1);
        let health = rows
            .state_of(at)
            .and_then(|state| state.living)
            .map(|living| living.health)
            .expect("the pig is alive");
        assert!(health < 10.0, "the arrow damages the pig row");
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::message::Messages<ProjectileHitEvent>>()
                .len(),
            1
        );
    }
}
