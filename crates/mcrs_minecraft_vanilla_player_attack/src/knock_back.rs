use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec2;
use glam::DVec3;
use mcrs_minecraft_damage::DamageTags;
use mcrs_minecraft_damage::DamageTypeRules;
use mcrs_minecraft_damage::Footing;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_damage::knockback;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::send_own_entity_state;
use mcrs_minecraft_engine::client_on_ground::ClientOnGround;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::EntityWrite;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::physics::Velocity;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_numeric::cos_nearest;
use mcrs_minecraft_numeric::radians_from_degrees;
use mcrs_minecraft_numeric::sin_nearest;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetEntityMotion;

use crate::attack_knockback::AttackKnockback;
use crate::attack_knockbacks::AttackKnockbacks;
use crate::birth_tick::birth_tick;

const DEFAULT_STRENGTH: f64 = 0.4;
const ATTACKER_SLOWDOWN: DVec3 = DVec3::new(0.6, 1.0, 0.6);

type Players<'w, 's> = Query<
    'w,
    's,
    (
        &'static EntityUuid,
        &'static Owner,
        &'static Transform,
        Option<&'static mut Velocity>,
        &'static mut PlayerSprinting,
        Option<&'static ClientOnGround>,
        Option<&'static AttributeInstances>,
    ),
>;

fn push(
    velocity: DVec3,
    away: Option<DVec2>,
    attack: &AttackKnockback,
    resistance: f64,
    footing: Footing,
    stream: &KeyedStream,
) -> DVec3 {
    let velocity = away.map_or(velocity, |away| {
        knockback(
            velocity,
            away,
            DEFAULT_STRENGTH,
            resistance,
            footing,
            stream,
        )
    });
    knockback(
        velocity,
        look(attack.yaw),
        f64::from(attack.strength),
        resistance,
        footing,
        stream,
    )
}

fn footing(on_ground: bool) -> Footing {
    if on_ground {
        Footing::Ground
    } else {
        Footing::Air
    }
}

fn horizontal(vector: DVec3) -> DVec2 {
    DVec2::new(vector.x, vector.z)
}

fn look(yaw: f32) -> DVec2 {
    let radians = radians_from_degrees(yaw);
    DVec2::new(
        f64::from(sin_nearest(radians)),
        -f64::from(cos_nearest(radians)),
    )
}

fn take(knockbacks: &mut AttackKnockbacks, hit: &Damaged) -> Option<AttackKnockback> {
    let index = knockbacks.0.iter().position(|knockback| {
        knockback.target == hit.target && hit.source.cause.direct == Some(knockback.attacker_uuid)
    })?;
    Some(knockbacks.0.remove(index))
}

#[expect(
    clippy::too_many_arguments,
    reason = "a system takes each resource that it reads"
)]
pub(crate) fn knock_back(
    mut knockbacks: ResMut<AttackKnockbacks>,
    mut hits: MessageReader<Damaged>,
    rules: Option<Res<DamageTypeRules>>,
    clock: Option<Res<BirthClock>>,
    mut players: Players,
    mobs: Query<(&MotionState, &Mob)>,
    mut writes: Option<ResMut<EntityWrites>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let tick = birth_tick(clock.as_deref());
    let mut composed: Option<(Entity, DVec3)> = None;
    let mut sequence: u32 = 0;
    for hit in hits.read() {
        if hit.report.outcome == HitOutcome::Rejected {
            continue;
        }
        let Some(attack) = take(&mut knockbacks, hit) else {
            continue;
        };
        let full = hit.report.outcome == HitOutcome::Full
            && rules.as_deref().is_none_or(|rules| {
                !rules
                    .tags(hit.source.damage_type)
                    .contains(DamageTags::NO_KNOCKBACK)
            });
        if let Ok((uuid, owner, transform, velocity, _, on_ground, attributes)) =
            players.get(hit.target)
        {
            if full {
                let after = push(
                    velocity.map_or(DVec3::ZERO, |velocity| velocity.0),
                    Some(horizontal(attack.position - transform.translation)),
                    &attack,
                    attributes
                        .and_then(|attributes| attributes.value(Attribute::KnockbackResistance))
                        .unwrap_or(0.0),
                    footing(on_ground.is_some_and(|ground| ground.0)),
                    &KeyedStream::new(uuid, tick),
                );
                let packet = ClientboundSetEntityMotion {
                    entity_id: VarInt(player_network_id(owner.0)),
                    velocity: LpVec3(after),
                };
                send_own_entity_state(&mut packets, owner.0, &packet, "knockback");
            }
        } else if let Ok((MotionState(state), mob)) = mobs.get(hit.target) {
            let before = match composed {
                Some((target, velocity)) if target == hit.target => velocity,
                _ => state.body.velocity,
            };
            let after = push(
                before,
                full.then(|| horizontal(attack.position - state.body.position)),
                &attack,
                mob.kind
                    .attribute_bases()
                    .and_then(|bases| bases.base(Attribute::KnockbackResistance))
                    .unwrap_or(0.0),
                footing(state.body.flags.on_ground),
                &KeyedStream::new(&state.key, tick),
            );
            composed = Some((hit.target, after));
            if let Some(writes) = writes.as_deref_mut()
                && after != before
            {
                writes.push(EntityWrite {
                    target: state.key,
                    writer: WriterKey::Entity {
                        key: attack.attacker_uuid,
                    },
                    sequence,
                    effect: after - before,
                });
                sequence = sequence.wrapping_add(1);
            }
        }
        if attack.strength <= 0.0 {
            continue;
        }
        let Ok((_, _, _, velocity, mut sprinting, ..)) = players.get_mut(attack.attacker) else {
            continue;
        };
        if let Some(mut velocity) = velocity {
            velocity.0 *= ATTACKER_SLOWDOWN;
        }
        if sprinting.last_changed() == attack.sprint_changed {
            sprinting.set_if_neq(PlayerSprinting(false));
        }
    }
    knockbacks.0.clear();
}

#[cfg(test)]
mod tests {
    use bevy_ecs::change_detection::Tick;

    use super::*;

    #[test]
    fn the_extra_knockback_moves_the_target_along_the_look_direction() {
        let cases = [
            (0.0, DVec3::new(0.0, 0.4, 0.5)),
            (90.0, DVec3::new(-0.5, 0.4, 0.0)),
        ];
        let stream = KeyedStream::new(&EntityUuid::from_u128(1), 0);
        for (yaw, expected) in cases {
            let attack = AttackKnockback {
                attacker: Entity::PLACEHOLDER,
                attacker_uuid: EntityUuid::from_u128(1),
                target: Entity::PLACEHOLDER,
                strength: 0.5,
                yaw,
                position: DVec3::ZERO,
                sprint_changed: Tick::new(0),
            };
            let pushed = push(DVec3::ZERO, None, &attack, 0.0, Footing::Ground, &stream);
            assert!(
                pushed.abs_diff_eq(expected, 1.0E-6),
                "yaw {yaw}: {pushed} != {expected}"
            );
        }
    }
}
