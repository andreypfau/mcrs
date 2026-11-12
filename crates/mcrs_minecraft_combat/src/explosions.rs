use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_block::definition::{BlockStateFlags, Blocks};
use mcrs_minecraft_damage::{DamageSource, DamageTypeFlags, Hit};
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_physics::{MobChanges, hurt_mob};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::block_update::BlockDestroyRequest;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockPos;
use std::collections::BTreeSet;

use crate::damage_rules::DamageTypeRulesResource;
use crate::events::DamageRequest;
use crate::health::{Health, KnockbackResistance};
use crate::position::CombatPosition;

const DAMAGE_TYPE_EXPLOSION: u32 = crate::damage_types::EXPLOSION;
const DAMAGE_TYPE_PLAYER_EXPLOSION: u32 = crate::damage_types::PLAYER_EXPLOSION;

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
    pub cause: Option<Entity>,
}

/// A request to light the TNT block at `pos`, from a flint and steel, a fire charge or a fuse.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct IgniteTntRequest {
    pub dimension: Entity,
    pub pos: BlockPos,
    pub owner: Option<Entity>,
}

/// Lights the TNT block at each requested position with the default fuse.
pub fn ignite_tnt_blocks(
    mut commands: Commands,
    mut requests: MessageReader<IgniteTntRequest>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    mut destroys: Option<MessageWriter<BlockDestroyRequest>>,
) {
    let (Some(definitions), Some(destroys)) = (definitions.as_deref(), destroys.as_mut()) else {
        return;
    };
    let tnt = definitions.block("minecraft:tnt");
    for request in requests.read() {
        let Some(view) = world.get(request.dimension) else {
            continue;
        };
        let Ok(state) = view.block(request.pos) else {
            continue;
        };
        if !tnt.is_some_and(|tnt| tnt.owns(state)) {
            continue;
        }
        destroys.write(BlockDestroyRequest {
            dimension: request.dimension,
            pos: request.pos,
            drops: false,
        });
        let position = request.pos.as_dvec3() + DVec3::splat(0.5);
        commands.spawn((
            PrimedTnt::new(80, request.owner),
            CombatPosition(position),
            mcrs_minecraft_engine_world::entity::physics::Transform::from_translation(position),
        ));
    }
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
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: the requests, the ECS targets, the row targets and the writers are separate parameters"
)]
pub fn process_explosions(
    mut requests: MessageReader<ExplosionRequest>,
    entities: Query<(
        Entity,
        &CombatPosition,
        &Health,
        Option<&KnockbackResistance>,
    )>,
    causes: Query<&EntityUuid>,
    mut tnt: Query<(&mut PrimedTnt, &CombatPosition)>,
    mut rows: Option<ResMut<EntityRows>>,
    mut changes: Option<ResMut<MobChanges>>,
    rules: Option<Res<DamageTypeRulesResource>>,
    clock: Option<Res<BirthClock>>,
    mut damage_writer: MessageWriter<DamageRequest>,
    mut occurred_writer: MessageWriter<ExplosionOccurred>,
) {
    for req in requests.read() {
        occurred_writer.write(ExplosionOccurred {
            center: req.center,
            power: req.power,
            cause: req.cause,
        });

        let radius = f64::from(req.power) * 2.0;
        let radius_sq = radius * radius;

        for (mut fuse, pos) in &mut tnt {
            if pos.0.distance_squared(req.center) <= radius_sq {
                let spread = ((pos.0.x.to_bits() ^ pos.0.z.to_bits()) % 21) as i32;
                fuse.fuse_ticks = fuse.fuse_ticks.min(10 + spread);
            }
        }

        let cause = req
            .cause
            .and_then(|cause| causes.get(cause).ok())
            .map(|uuid| EntityKey(uuid.0.as_u128()));
        let damage_type = if cause.is_some() {
            DAMAGE_TYPE_PLAYER_EXPLOSION
        } else {
            DAMAGE_TYPE_EXPLOSION
        };

        for (entity, pos, _health, _res) in &entities {
            let dist_sq = pos.0.distance_squared(req.center);
            if dist_sq <= radius_sq {
                let dist = dist_sq.sqrt();
                let exposure = (1.0 - dist / radius).clamp(0.0, 1.0) as f32;
                let damage = ((exposure * exposure + exposure) / 2.0) * 7.0 * radius as f32 + 1.0;
                let mut source = DamageSource::new(damage_type);
                source.causing_entity = cause;
                source.direct_entity = cause;
                source.source_position = Some(req.center);
                let mut hit = Hit::from_source(damage, source);
                hit.knockback = exposure;
                damage_writer.write(DamageRequest::new(entity, hit));
            }
        }

        let Some(rows) = rows.as_deref_mut() else {
            continue;
        };
        let flags = rules
            .as_deref()
            .map_or(DamageTypeFlags::builtin_for(damage_type), |rules| {
                rules.0.flags(damage_type)
            });
        let mut struck = Vec::new();
        for (at, state) in rows.living_rows() {
            if !state.living.is_some_and(|living| living.is_alive()) {
                continue;
            }
            let dist_sq = state.body.position.distance_squared(req.center);
            if dist_sq <= radius_sq {
                struck.push((at, state.body.position));
            }
        }
        let Some(changes) = changes.as_deref_mut() else {
            continue;
        };
        for (at, position) in struck {
            let dist = position.distance(req.center);
            let exposure = (1.0 - dist / radius).clamp(0.0, 1.0) as f32;
            let damage = ((exposure * exposure + exposure) / 2.0) * 7.0 * radius as f32 + 1.0;
            let mut source = DamageSource::new(damage_type);
            source.causing_entity = cause;
            source.direct_entity = cause;
            source.source_position = Some(req.center);
            let mut hit = Hit::from_source(damage, source).with_flags(flags);
            hit.knockback = exposure;
            hurt_mob(
                rows,
                changes,
                at,
                hit,
                clock.as_deref().map_or(0, |clock| clock.0),
            );
        }
    }
}

/// Turns an explosion into block destroys for every block that one of its rays reaches.
#[expect(
    clippy::excessive_nesting,
    reason = "the 16-cube ray grid and the block tests share one walk"
)]
pub fn destroy_explosion_blocks(
    mut commands: Commands,
    mut explosions: MessageReader<ExplosionOccurred>,
    dimension: Query<Entity, With<Dimension>>,
    world: WorldView,
    definitions: Option<Res<Blocks>>,
    requests: Option<MessageWriter<BlockDestroyRequest>>,
) {
    let Ok(dimension) = dimension.single() else {
        return;
    };
    let Some(mut requests) = requests else {
        return;
    };
    let Some(definitions) = definitions.as_deref() else {
        return;
    };
    let Some(view) = world.get(dimension) else {
        return;
    };
    let tnt = definitions.block("minecraft:tnt");
    for explosion in explosions.read() {
        let mut destroyed = BTreeSet::new();
        let mut tnt_blocks = BTreeSet::new();
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    if i != 0 && i != 15 && j != 0 && j != 15 && k != 0 && k != 15 {
                        continue;
                    }
                    let direction = DVec3::new(
                        f64::from(i) / 15.0 * 2.0 - 1.0,
                        f64::from(j) / 15.0 * 2.0 - 1.0,
                        f64::from(k) / 15.0 * 2.0 - 1.0,
                    );
                    let Some(direction) = direction.try_normalize() else {
                        continue;
                    };
                    let step = direction * 0.3;
                    let mut at = explosion.center;
                    let mut intensity = f64::from(explosion.power);
                    for _ in 0..1024 {
                        at += step;
                        let block_pos = BlockPos::new(
                            at.x.floor() as i32,
                            at.y.floor() as i32,
                            at.z.floor() as i32,
                        );
                        let Ok(state) = view.block(block_pos) else {
                            break;
                        };
                        let data = definitions.state(state);
                        if data.flags.contains(BlockStateFlags::IS_AIR) {
                            continue;
                        }
                        intensity -= (f64::from(data.explosion_resistance) + 0.3) * 0.3;
                        if intensity <= 0.0 {
                            break;
                        }
                        if data.explosion_resistance < 100.0 {
                            if tnt.is_some_and(|tnt| tnt.owns(state)) {
                                tnt_blocks.insert((block_pos.x, block_pos.y, block_pos.z));
                            }
                            destroyed.insert((block_pos.x, block_pos.y, block_pos.z));
                        }
                    }
                }
            }
        }
        for &(x, y, z) in &tnt_blocks {
            let position = BlockPos::new(x, y, z).as_dvec3() + DVec3::splat(0.5);
            let spread = ((position.x.to_bits() ^ position.z.to_bits()) % 21) as i32;
            commands.spawn((
                PrimedTnt::new(10 + spread, explosion.cause),
                CombatPosition(position),
                mcrs_minecraft_engine_world::entity::physics::Transform::from_translation(position),
            ));
        }
        for (x, y, z) in destroyed {
            requests.write(BlockDestroyRequest {
                dimension,
                pos: BlockPos::new(x, y, z),
                drops: true,
            });
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
    use crate::health::Armor;
    use bevy_app::{App, Update};
    use mcrs_minecraft_engine_brain::EntityRows;
    use mcrs_minecraft_engine_physics::{MobChanges, mob_body, mob_row};
    use mcrs_minecraft_engine_world::birth_clock::BirthClock;
    use mcrs_minecraft_entity::EntityTypeData;
    use mcrs_minecraft_entity_key::{EntityId, EntityKey};

    #[test]
    fn exp_tnt_001_primed_tnt_explodes_on_the_eightieth_tick() {
        let mut app = App::new();
        app.add_message::<ExplosionRequest>();
        app.add_systems(Update, tick_primed_tnt);
        let tnt = app
            .world_mut()
            .spawn((
                PrimedTnt::new(80, None),
                CombatPosition::from_coords(8.5, 64.0, 8.5),
            ))
            .id();
        for _ in 0..79 {
            app.update();
        }
        assert!(app.world().get_entity(tnt).is_ok());
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::message::Messages<ExplosionRequest>>()
                .len(),
            0
        );
        app.update();
        assert!(app.world().get_entity(tnt).is_err());
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::message::Messages<ExplosionRequest>>()
                .len(),
            1
        );
    }

    #[test]
    fn exp_tnt_003_an_explosion_shortens_the_fuse_of_nearby_tnt() {
        let mut app = App::new();
        app.add_message::<ExplosionRequest>();
        app.add_message::<ExplosionOccurred>();
        app.add_message::<DamageRequest>();
        app.add_systems(Update, process_explosions);
        let tnt = app
            .world_mut()
            .spawn((
                PrimedTnt::new(80, None),
                CombatPosition::from_coords(8.5, 64.0, 8.5),
            ))
            .id();
        app.world_mut()
            .write_message(ExplosionRequest::new(DVec3::new(8.5, 64.0, 8.5), 4.0, None));
        app.update();
        let fuse = app
            .world()
            .get::<PrimedTnt>(tnt)
            .expect("the tnt survives the explosion")
            .fuse_ticks;
        assert!(
            (10..=30).contains(&fuse),
            "the chain fuse is 10 through 30, got {fuse}"
        );
    }

    #[test]
    fn exp_dmg_001_an_explosion_damages_an_entity_in_its_radius() {
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
        req_writer.write(ExplosionRequest::new(DVec3::new(0.0, 64.0, 0.0), 4.0, None));

        app.update();

        let damage_messages = app
            .world()
            .resource::<bevy_ecs::message::Messages<DamageRequest>>();
        assert_eq!(damage_messages.len(), 1);
        assert!(
            (damage_messages
                .iter_current_update_messages()
                .next()
                .map(|request| request.hit.amount)
                .unwrap_or_default()
                - 37.75)
                .abs()
                < 1.0e-3,
            "the exposure at two blocks of an eight block radius gives 37.75 damage"
        );
    }

    #[test]
    fn exp_dmg_001_an_explosion_damages_a_mob_row_in_its_radius() {
        let mut app = App::new();
        app.add_message::<ExplosionRequest>();
        app.add_message::<ExplosionOccurred>();
        app.add_message::<DamageRequest>();
        app.add_systems(Update, process_explosions);
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

        let mut requests = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<ExplosionRequest>>();
        requests.write(ExplosionRequest::new(DVec3::new(2.0, 64.0, 0.0), 4.0, None));

        app.update();

        let health = app
            .world()
            .resource::<EntityRows>()
            .state_of(at)
            .and_then(|state| state.living)
            .map(|living| living.health);
        assert!(health.is_some_and(|health| health <= 0.0));
    }

    #[test]
    fn exp_tnt_003_a_nearby_primed_tnt_is_shortened_into_the_chain_fuse() {
        let mut app = App::new();
        app.add_message::<ExplosionRequest>();
        app.add_message::<ExplosionOccurred>();
        app.add_message::<DamageRequest>();
        app.add_systems(Update, process_explosions);
        let tnt = app
            .world_mut()
            .spawn((
                PrimedTnt::default(),
                CombatPosition::from_coords(2.0, 64.0, 0.0),
            ))
            .id();
        app.world_mut()
            .resource_mut::<bevy_ecs::message::Messages<ExplosionRequest>>()
            .write(ExplosionRequest::new(DVec3::new(0.0, 64.0, 0.0), 4.0, None));
        app.update();
        let fuse = app.world().get::<PrimedTnt>(tnt).unwrap().fuse_ticks;
        assert!((10..=30).contains(&fuse), "{fuse}");
    }
}
