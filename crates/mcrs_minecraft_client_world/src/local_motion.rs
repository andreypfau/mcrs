use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::{EntityTypeData, SPRINT_MODIFIER};
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeInstances, AttributeModifier, HomeSlot, Instance, InstanceModifier, Modifier, fold,
};
use mcrs_minecraft_entity_visual::SyncedData;
use mcrs_minecraft_game_mode::{Abilities, GameMode};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_physics::{MotionAttributes, PlayerState, SprintSpeed};
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundLogin, ClientboundRespawn, ClientboundSetEntityData,
};
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity_data_index::POSE;
use mcrs_minecraft_resource::ResourceLocation;

use crate::abilities;
use crate::apply_entity_data::apply_entity_data;
use crate::joined_game::JoinedGame;
use crate::local_player::{LastSentInput, Sprint};
use crate::player::Player;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LocalMotion(pub PlayerState);

#[must_use]
pub fn eye_height(motion: Option<&LocalMotion>) -> f32 {
    motion.map_or(Pose::Standing.player_dimensions().eye_height, |motion| {
        let scale = motion.0.attributes.scale as f32;
        motion.0.pose.player_dimensions().scaled(scale).eye_height
    })
}

pub(crate) fn on_set_entity_data(
    event: On<Received<ClientboundSetEntityData<'static>>>,
    connections: Query<&JoinedGame>,
    mut player: Query<(&mut LocalMotion, &mut SyncedData), With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let own = connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == packet.entity_id.0);
    if !own {
        return;
    }
    let Ok((motion, mut synced)) = player.single_mut() else {
        return;
    };
    apply_entity_data(
        &mut synced,
        EntityTypeData::of(EntityType::Player),
        &packet.metadata.0,
    );
    let sends_pose = packet
        .metadata
        .0
        .iter()
        .any(|entry| entry.index == POSE && matches!(entry.value, MetaDataValue::Pose(_)));
    if sends_pose {
        motion
            .map_unchanged(|motion| &mut motion.0.pose)
            .set_if_neq(synced.pose);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Speeds {
    walking: f64,
    sprint: f64,
}

pub(crate) fn on_login(
    event: On<Received<ClientboundLogin>>,
    player: Query<(Entity, &PhysicsTransform), With<Player>>,
    commands: Commands,
) {
    if let Some(login) = event.packet() {
        spawned(login.player_spawn_info.game_mode, player, commands);
    }
}

pub(crate) fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    player: Query<(Entity, &PhysicsTransform), With<Player>>,
    commands: Commands,
) {
    if let Some(respawn) = event.packet() {
        spawned(respawn.player_spawn_info.game_mode, player, commands);
    }
}

fn spawned(
    mode: GameMode,
    player: Query<(Entity, &PhysicsTransform), With<Player>>,
    mut commands: Commands,
) {
    if let Ok((player, transform)) = player.single() {
        commands.entity(player).insert((
            LocalMotion::spawned(mode, transform.translation),
            SyncedData::default(),
            LastSentInput::default(),
        ));
    }
}

impl LocalMotion {
    pub(crate) fn spawned(mode: GameMode, position: DVec3) -> Self {
        Self(PlayerState {
            flight0: abilities::flight_after_change(mode, false),
            ..PlayerState::new(position)
        })
    }
}

pub(crate) fn write_motion_inputs(
    mut players: Populated<
        (&mut LocalMotion, &AttributeInstances, &Sprint, &Abilities),
        (
            With<Player>,
            Or<(
                Changed<AttributeInstances>,
                Changed<Sprint>,
                Changed<Abilities>,
                Added<LocalMotion>,
            )>,
        ),
    >,
) {
    for (mut motion, instances, sprint, abilities) in &mut players {
        let mut next = motion.0;
        let speeds = speeds(instances, *sprint).unwrap_or(Speeds {
            walking: next.attributes.movement_speed,
            sprint: next.sprint_speed.0,
        });
        next.attributes = motion_attributes(instances, speeds.walking, next.attributes);
        next.sprint_speed = SprintSpeed(speeds.sprint);
        next.may_fly = abilities.may_fly;
        next.flight_speed = abilities.flying_speed;
        motion.set_if_neq(LocalMotion(next));
    }
}

fn speeds(instances: &AttributeInstances, sprint: Sprint) -> Option<Speeds> {
    let current = instances.value(Attribute::MovementSpeed)?;
    let speeds = if sprint.active {
        Speeds {
            walking: value_replacing(
                instances,
                Attribute::MovementSpeed,
                SPRINT_MODIFIER.id,
                None,
            )?,
            sprint: current,
        }
    } else {
        Speeds {
            walking: current,
            sprint: value_replacing(
                instances,
                Attribute::MovementSpeed,
                SPRINT_MODIFIER.id,
                Some(SPRINT_MODIFIER),
            )?,
        }
    };
    Some(speeds)
}

fn value_replacing(
    instances: &AttributeInstances,
    attribute: Attribute,
    id: ResourceLocation<'_>,
    replacement: Option<AttributeModifier<'static>>,
) -> Option<f64> {
    let base = instances.base(attribute)?;
    let modifiers: &[InstanceModifier] = instances
        .set()
        .instance(attribute)
        .map_or(&[], Instance::modifiers);
    let appended = replacement.map(Modifier::from);
    let kept = modifiers
        .iter()
        .filter(move |kept| kept.id() != id)
        .map(|kept| Modifier {
            operation: kept.operation(),
            amount: kept.amount(),
            slot: HomeSlot::from(kept.id()),
        });
    let value = fold(base, kept.chain(appended));
    Some(attribute.definition().range.clamp(value))
}

fn motion_attributes(
    instances: &AttributeInstances,
    movement_speed: f64,
    current: MotionAttributes,
) -> MotionAttributes {
    let value = |attribute: Attribute, current: f64| instances.value(attribute).unwrap_or(current);
    MotionAttributes {
        gravity: value(Attribute::Gravity, current.gravity),
        step_height: value(Attribute::StepHeight, current.step_height),
        movement_speed,
        jump_strength: value(Attribute::JumpStrength, current.jump_strength),
        scale: value(Attribute::Scale, current.scale),
        sneaking_speed: value(Attribute::SneakingSpeed, current.sneaking_speed),
        movement_efficiency: value(Attribute::MovementEfficiency, current.movement_efficiency),
        bounciness: value(Attribute::Bounciness, current.bounciness),
        friction_modifier: value(Attribute::FrictionModifier, current.friction_modifier),
        air_drag_modifier: value(Attribute::AirDragModifier, current.air_drag_modifier),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement_input::MovementInput;
    use crate::test_session::{FLOOR_TOP, OWN_PLAYER, TestSession, login, spawn_info};
    use bevy::math::DVec3;
    use mcrs_minecraft_entity_attribute::AttributeModifier;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol_game::clientbound::{
        AttributeSnapshot, ClientboundPlayerAbilities, ClientboundUpdateAttributes,
    };

    fn movement_speed(base: f64, sprinting: bool) -> AttributeSnapshot<'static> {
        AttributeSnapshot {
            attribute: Attribute::MovementSpeed.into(),
            base,
            modifiers: if sprinting {
                vec![SPRINT_MODIFIER]
            } else {
                Vec::new()
            },
        }
    }

    fn entry(attribute: Attribute, base: f64) -> AttributeSnapshot<'static> {
        AttributeSnapshot {
            attribute: attribute.into(),
            base,
            modifiers: Vec::new(),
        }
    }

    fn after_two_ticks(
        input: MovementInput,
        entries: &[AttributeSnapshot<'static>],
        abilities: Option<ClientboundPlayerAbilities>,
        respawn: Option<ClientboundRespawn>,
    ) -> PlayerState {
        let mut session = TestSession::new(DVec3::new(8.5, FLOOR_TOP + 4.0, 8.5));
        session.receive(login(GameMode::Survival));
        session.ticks(input, 1);
        session.receive(ClientboundUpdateAttributes {
            entity_id: VarInt(OWN_PLAYER),
            attributes: entries.to_vec(),
        });
        if let Some(abilities) = abilities {
            session.receive(abilities);
        }
        if let Some(respawn) = respawn {
            session.receive(respawn);
        }
        session.ticks(input, 1);
        session.motion()
    }

    struct Case {
        name: &'static str,
        input: MovementInput,
        entries: Vec<AttributeSnapshot<'static>>,
        abilities: Option<ClientboundPlayerAbilities>,
        respawn: Option<ClientboundRespawn>,
        attributes: MotionAttributes,
        sprint_speed: f64,
        may_fly: bool,
        flight_speed: f32,
        flight0: bool,
    }

    fn cases() -> [Case; 6] {
        let walking = f64::from(0.1_f32);
        let with_sprint = |speed: f64| speed * (1.0 + f64::from(0.3_f32));
        let sprinting = MovementInput {
            forward: true,
            sprint: true,
            ..MovementInput::EMPTY
        };
        let attributes = |movement_speed: f64| MotionAttributes {
            movement_speed,
            ..MotionAttributes::default()
        };
        let base = || Case {
            name: "a new player",
            input: MovementInput::EMPTY,
            entries: Vec::new(),
            abilities: None,
            respawn: None,
            attributes: attributes(walking),
            sprint_speed: with_sprint(walking),
            may_fly: false,
            flight_speed: 0.05_f32,
            flight0: false,
        };
        [
            Case {
                name: "an entry drops the modifier of a sprinting player",
                input: sprinting,
                entries: vec![movement_speed(walking, false)],
                sprint_speed: walking,
                ..base()
            },
            Case {
                name: "an entry gives the modifier to a player that does not sprint",
                entries: vec![movement_speed(walking, true)],
                attributes: attributes(with_sprint(walking)),
                sprint_speed: with_sprint(walking),
                ..base()
            },
            Case {
                name: "the sprint speed of a player that does not sprint replaces a sprint modifier \
                       of another amount",
                entries: vec![AttributeSnapshot {
                    modifiers: vec![AttributeModifier {
                        amount: 0.5,
                        ..SPRINT_MODIFIER
                    }],
                    ..movement_speed(walking, false)
                }],
                attributes: attributes(walking * 1.5),
                sprint_speed: with_sprint(walking),
                ..base()
            },
            Case {
                name: "gravity and step height",
                entries: vec![
                    entry(Attribute::Gravity, 0.05),
                    entry(Attribute::StepHeight, 1.0),
                ],
                attributes: MotionAttributes {
                    gravity: 0.05,
                    step_height: 1.0,
                    ..attributes(walking)
                },
                ..base()
            },
            Case {
                name: "player_abilities",
                abilities: Some(ClientboundPlayerAbilities {
                    invulnerable: true,
                    flying: true,
                    may_fly: true,
                    instant_build: true,
                    flying_speed: 0.1_f32,
                    walking_speed: 0.2_f32,
                }),
                may_fly: true,
                flight_speed: 0.1_f32,
                flight0: true,
                ..base()
            },
            Case {
                name: "a base of 0.2 before a respawn",
                entries: vec![movement_speed(0.2, false)],
                respawn: Some(ClientboundRespawn {
                    player_spawn_info: spawn_info(GameMode::Survival),
                    data_to_keep: 0,
                }),
                attributes: attributes(0.2),
                sprint_speed: with_sprint(0.2),
                ..base()
            },
        ]
    }

    #[test]
    fn attribute_entries_the_sprint_state_and_abilities_reach_the_kernel_state() {
        for case in cases() {
            let state = after_two_ticks(case.input, &case.entries, case.abilities, case.respawn);
            let name = case.name;
            assert_eq!(state.attributes, case.attributes, "{name}");
            assert_eq!(
                state.sprint_speed.0.to_bits(),
                case.sprint_speed.to_bits(),
                "{name}"
            );
            assert_eq!(
                (state.may_fly, state.flight_speed.to_bits(), state.flight0),
                (case.may_fly, case.flight_speed.to_bits(), case.flight0),
                "{name}"
            );
        }
    }
}
