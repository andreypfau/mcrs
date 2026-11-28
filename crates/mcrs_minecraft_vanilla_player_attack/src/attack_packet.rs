use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::change_detection::Ref;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::query::AnyOf;
use bevy_ecs::query::Has;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine::eye_position;
use mcrs_minecraft_engine::loaded_client::UnloadedClient;
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_entity_motion_replication::NetworkIds;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity::damage_type_keys::PLAYER_ATTACK;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::registry_keys::DAMAGE_TYPE;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::Cause;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_protocol_game::serverbound::ServerboundAttack;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_text::Text;

use crate::AttackCounter;
use crate::attack_amount;
use crate::attack_knockback::AttackKnockback;
use crate::attack_knockbacks::AttackKnockbacks;
use crate::attack_strength;
use crate::birth_tick::birth_tick;
use crate::ignores_attacks::ignores_attacks;
use crate::in_attack_reach;

const FULL_STRENGTH: f32 = 0.9;
const SPRINT_KNOCKBACK: f32 = 0.5;

type Attackers<'w, 's> = Query<
    'w,
    's,
    (
        &'static EntityUuid,
        &'static Owner,
        &'static PlayerGameMode,
        &'static AttributeInstances,
        &'static mut AttackCounter,
        &'static Transform,
        PlayerBody,
        Ref<'static, PlayerSprinting>,
        Option<&'static Vitals>,
        Has<UnloadedClient>,
    ),
>;

type PlayerTargets<'w, 's> =
    Query<'w, 's, (&'static Transform, PlayerBody, Option<&'static Vitals>), With<Owner>>;

type OtherTargets<'w, 's> = Query<
    'w,
    's,
    (
        AnyOf<(&'static Appearance, &'static Mob)>,
        &'static MotionState,
        &'static MotionKind,
        Option<&'static Vitals>,
    ),
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetClass {
    Invalid,
    Ignored,
    Attackable,
}

#[derive(Clone, Copy, Debug)]
struct Target {
    entity: Entity,
    kind: EntityType,
    alive: bool,
    bounds: Aabb,
}

fn target_class(kind: EntityType) -> TargetClass {
    match kind {
        EntityType::Item
        | EntityType::ExperienceOrb
        | EntityType::Arrow
        | EntityType::SpectralArrow
        | EntityType::Trident => TargetClass::Invalid,
        EntityType::FallingBlock | EntityType::FireworkRocket | EntityType::EyeOfEnder => {
            TargetClass::Ignored
        }
        _ => TargetClass::Attackable,
    }
}

fn is_alive(vitals: Option<&Vitals>) -> bool {
    vitals.is_none_or(Vitals::is_alive)
}

fn find_target(
    id: i32,
    ids: &NetworkIds,
    players: &PlayerTargets,
    others: &OtherTargets,
) -> Option<Target> {
    let entity = ids.get(id)?;
    if let Ok((transform, body, vitals)) = players.get(entity) {
        let dimensions = body.dimensions();
        return Some(Target {
            entity,
            kind: EntityType::Player,
            alive: is_alive(vitals),
            bounds: Aabb::of_body(transform.translation, dimensions.width, dimensions.height),
        });
    }
    let ((appearance, mob), MotionState(state), MotionKind(row), vitals) =
        others.get(entity).ok()?;
    let kind = mob
        .map(|mob| mob.kind)
        .or(appearance.map(|appearance| appearance.kind))?;
    Some(Target {
        entity,
        kind: kind.entity_type,
        alive: is_alive(vitals),
        bounds: Aabb::of_body(
            state.body.position,
            row.dimensions.width,
            row.dimensions.height,
        ),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "a system takes each resource that it reads"
)]
pub(crate) fn on_attack(
    received: On<Received<ServerboundAttack>>,
    mut attackers: Attackers,
    players: PlayerTargets,
    others: OtherTargets,
    ids: Option<Res<NetworkIds>>,
    rules: Option<Res<GameRules>>,
    registries: Option<Res<CurrentRegistries>>,
    clock: Option<Res<BirthClock>>,
    mut pending: ResMut<PendingHits>,
    mut knockbacks: ResMut<AttackKnockbacks>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let attacker = received.entity;
    let Ok((
        uuid,
        owner,
        mode,
        attributes,
        mut counter,
        transform,
        body,
        sprinting,
        vitals,
        unloaded,
    )) = attackers.get_mut(attacker)
    else {
        return;
    };
    if ignores_attacks(unloaded, mode, vitals) {
        return;
    }
    let Some(target) = ids
        .as_deref()
        .and_then(|ids| find_target(packet.entity_id.0, ids, &players, &others))
    else {
        return;
    };
    let eye = eye_position(transform.translation, body.dimensions());
    let range = attributes.value(Attribute::EntityInteractionRange);
    if !range.is_some_and(|range| in_attack_reach(eye, target.bounds, range)) {
        return;
    }
    let class = target_class(target.kind);
    if target.entity == attacker || class == TargetClass::Invalid {
        packets.write(to(
            owner.0,
            PacketPayload::Disconnect(Text::translate(
                "multiplayer.disconnect.invalid_entity_attacked",
                Vec::new(),
            )),
        ));
        return;
    }
    if class == TargetClass::Ignored || !target.alive {
        return;
    }
    let now = birth_tick(clock.as_deref());
    let ticks = counter.ticks(now);
    counter.set_if_neq(AttackCounter { reset_at: now });
    let pvp = rules
        .as_deref()
        .is_none_or(|rules| rules.is_on(GameRule::Pvp));
    if target.kind == EntityType::Player && !pvp {
        return;
    }
    let Some((speed, damage)) = attributes
        .value(Attribute::AttackSpeed)
        .zip(attributes.value(Attribute::AttackDamage))
    else {
        return;
    };
    let strength = attack_strength(ticks, speed);
    let Some(amount) = attack_amount(damage, strength) else {
        return;
    };
    let Some(damage_type) = registries
        .and_then(|registries| registries.id_of(DAMAGE_TYPE.location(), PLAYER_ATTACK.location()))
        .map(u32::from)
    else {
        return;
    };
    let mut source = DamageSource::of(damage_type);
    source.cause = Cause {
        causing: Some(*uuid),
        direct: Some(*uuid),
    };
    source.by_creative_player = mode.0 == GameMode::Creative;
    let sequence = u32::try_from(pending.len()).unwrap_or(u32::MAX);
    pending.push(DamageRequest::new(
        target.entity,
        Hit::new(amount, source),
        WriterKey::Entity { key: *uuid },
        now,
        sequence,
    ));
    let mut knockback = attributes.value(Attribute::AttackKnockback).unwrap_or(0.0) as f32 / 2.0;
    if sprinting.0 && strength > FULL_STRENGTH {
        knockback += SPRINT_KNOCKBACK;
    }
    knockbacks.0.push(AttackKnockback {
        attacker,
        attacker_uuid: *uuid,
        target: target.entity,
        strength: knockback,
        yaw: transform.rotation.yaw(),
        position: transform.translation,
        sprint_changed: sprinting.last_changed(),
    });
}
