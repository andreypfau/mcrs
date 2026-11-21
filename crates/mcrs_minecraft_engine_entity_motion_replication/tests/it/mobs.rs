use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use glam::DVec3;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_mob_attributes::AttributeEdit;
use mcrs_minecraft_engine_mob_attributes::MobAttributesPlugin;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::MobExtras;
use mcrs_minecraft_engine_physics::MobIndex;
use mcrs_minecraft_engine_physics::MobSpawnRequest;
use mcrs_minecraft_engine_physics::MobSwing;
use mcrs_minecraft_engine_physics::MobSwingSpec;
use mcrs_minecraft_engine_physics::SpawnSetup;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_physics::hurt_mob;
use mcrs_minecraft_engine_physics::swing_mob;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::InstanceModifier;
use mcrs_minecraft_entity_attribute::ModifierKind;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::ReplicationData;

const PIG: u128 = 0x006d_6273_0000_0000_0000_0000_0000_0002;
const FIRST_ID: EntityId = EntityId(2);

#[derive(Clone, Debug, PartialEq)]
enum Seen {
    Spawn {
        id: EntityId,
        position: DVec3,
        health: f32,
    },
    Move {
        id: EntityId,
        position: DVec3,
        on_ground: bool,
        health: f32,
        dying: bool,
    },
    Signal {
        id: EntityId,
        signal: MobSignal,
    },
    Attribute {
        id: EntityId,
        attribute: Attribute,
        modifiers: usize,
    },
    Despawn {
        id: EntityId,
    },
    Other,
}

fn scene() -> TestScene {
    let mut scene = TestScene::new(1, 0, 0, 0);
    scene
        .app
        .world_mut()
        .resource_mut::<CellEntityIds>()
        .add(EntityIdRange::starting_at(FIRST_ID.0).expect("a small range"));
    scene
        .app
        .add_plugins((MobAttributesPlugin, EntityMotionReplicationPlugin));
    scene.app.edit_schedule(FixedUpdate, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..ScheduleBuildSettings::default()
        });
    });
    scene
}

fn summon(scene: &mut TestScene, position: DVec3) -> Entity {
    scene.app.world_mut().write_message(MobSpawnRequest {
        kind: &EntityTypeData::PIG,
        position,
        uuid: PIG,
        setup: SpawnSetup::None,
        extras: MobExtras::default(),
    });
    let seen = tick(scene);
    assert!(
        matches!(seen.first(), Some(Seen::Spawn { .. })),
        "the summon spawns in its tick: {seen:?}"
    );
    scene
        .app
        .world()
        .resource::<MobIndex>()
        .get(EntityUuid::from_u128(PIG))
        .expect("the pig is indexed")
}

fn tick(scene: &mut TestScene) -> Vec<Seen> {
    scene.step();
    let deltas = scene.app.world_mut().resource_mut::<TileEvents>().take();
    deltas
        .tiles
        .into_iter()
        .flat_map(|delta| delta.events)
        .map(|event| {
            let id = event.subject;
            match event.event {
                EntityEvent::Spawn(ReplicationData::Mob(MobReport::Spawn(spawn))) => Seen::Spawn {
                    id,
                    position: spawn.state.position,
                    health: spawn.state.health,
                },
                EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => Seen::Move {
                    id,
                    position: state.position,
                    on_ground: state.on_ground,
                    health: state.health,
                    dying: state.dying,
                },
                EntityEvent::Data(ReplicationData::Mob(MobReport::Signal(signal))) => {
                    Seen::Signal { id, signal }
                }
                EntityEvent::Data(ReplicationData::Mob(MobReport::Attribute(attribute))) => {
                    Seen::Attribute {
                        id,
                        attribute: attribute.attribute,
                        modifiers: attribute
                            .instance
                            .map_or(0, |instance| instance.modifiers().len()),
                    }
                }
                EntityEvent::Despawn => Seen::Despawn { id },
                _ => Seen::Other,
            }
        })
        .collect()
}

fn ticks(scene: &mut TestScene, count: usize) -> Vec<Seen> {
    (0..count).flat_map(|_| tick(scene)).collect()
}

fn hurt(scene: &mut TestScene, pig: Entity, amount: f32) -> Option<HitOutcome> {
    let tick = scene.tick;
    let world = scene.app.world_mut();
    world.resource_scope(|world, mut changes: bevy_ecs::world::Mut<MobChanges>| {
        let mut state = world.get_mut::<MotionState>(pig)?;
        hurt_mob(pig, &mut state.0, &mut changes, Hit::new(amount, 7), tick)
    })
}

fn swing(scene: &mut TestScene, pig: Entity, spec: MobSwingSpec) -> bool {
    let tick = scene.tick;
    let world = scene.app.world_mut();
    world.resource_scope(|world, mut changes: bevy_ecs::world::Mut<MobChanges>| {
        let state = world.get::<MotionState>(pig).expect("pig state").0;
        let mut tracker = world.get_mut::<MobSwing>(pig).expect("pig swing");
        swing_mob(pig, &state, &mut tracker, &mut changes, spec, tick)
    })
}

#[test]
fn a_summoned_pig_spawns_with_its_start_and_falls_to_the_floor() {
    let mut scene = scene();
    let start = DVec3::new(4.5, f64::from(FLOOR) + 6.0, 4.5);
    scene.app.world_mut().write_message(MobSpawnRequest {
        kind: &EntityTypeData::PIG,
        position: start,
        uuid: PIG,
        setup: SpawnSetup::None,
        extras: MobExtras::default(),
    });
    let first = tick(&mut scene);
    assert_eq!(
        first.first(),
        Some(&Seen::Spawn {
            id: FIRST_ID,
            position: start,
            health: 10.0,
        }),
        "{first:#?}"
    );
    let seen = ticks(&mut scene, 60);
    let floor = DVec3::new(4.5, f64::from(FLOOR), 4.5);
    assert!(
        seen.iter().any(|seen| matches!(
            seen,
            Seen::Move { position, on_ground: true, .. } if *position == floor
        )),
        "the landing reports the floor with the ground flag: {seen:#?}"
    );
    assert_eq!(ticks(&mut scene, 5), [], "a pig at rest publishes nothing");
}

#[test]
fn a_changed_syncable_instance_of_a_mob_is_published_once() {
    let mut scene = scene();
    let pig = summon(&mut scene, DVec3::new(4.5, f64::from(FLOOR), 4.5));
    ticks(&mut scene, 5);
    let slow = |attribute| AttributeEdit {
        target: pig,
        attribute,
        edit: InstanceEdit::Put(InstanceModifier::new(
            "test:slow".parse().expect("a key"),
            -0.5,
            ModifierOperation::AddMultipliedTotal,
            ModifierKind::Permanent,
        )),
    };
    let attributes = |seen: Vec<Seen>| -> Vec<Seen> {
        seen.into_iter()
            .filter(|seen| matches!(seen, Seen::Attribute { .. }))
            .collect()
    };

    scene
        .app
        .world_mut()
        .write_message(slow(Attribute::MovementSpeed));
    assert_eq!(
        attributes(ticks(&mut scene, 3)),
        [Seen::Attribute {
            id: FIRST_ID,
            attribute: Attribute::MovementSpeed,
            modifiers: 1,
        }],
        "the next update holds the entry"
    );
    assert_eq!(attributes(ticks(&mut scene, 10)), [], "the entry goes once");

    scene
        .app
        .world_mut()
        .write_message(slow(Attribute::FollowRange));
    assert_eq!(
        attributes(ticks(&mut scene, 10)),
        [],
        "follow_range is not syncable"
    );
}

#[test]
fn a_killed_mob_is_hurt_dies_and_is_removed_after_its_death_time() {
    let mut scene = scene();
    let pig = summon(&mut scene, DVec3::new(4.5, f64::from(FLOOR), 4.5));
    ticks(&mut scene, 5);

    assert_eq!(hurt(&mut scene, pig, 1.0), Some(HitOutcome::Full));
    let hurt_tick = tick(&mut scene);
    assert_eq!(
        hurt_tick.get(..2),
        Some(
            &[
                Seen::Signal {
                    id: FIRST_ID,
                    signal: MobSignal::Hurt { damage_type: 7 },
                },
                Seen::Move {
                    id: FIRST_ID,
                    position: DVec3::new(4.5, f64::from(FLOOR), 4.5),
                    on_ground: true,
                    health: 9.0,
                    dying: false,
                },
            ][..]
        ),
        "the hit precedes the state that holds it: {hurt_tick:#?}"
    );
    ticks(&mut scene, 25);

    hurt(&mut scene, pig, f32::MAX);
    let death = tick(&mut scene);
    let died = death.iter().position(|seen| {
        *seen
            == Seen::Signal {
                id: FIRST_ID,
                signal: MobSignal::Died,
            }
    });
    let dying = death
        .iter()
        .position(|seen| matches!(seen, Seen::Move { health, dying: true, .. } if *health == 0.0));
    assert!(
        died.is_some() && died < dying,
        "the death event, then the dying state: {death:#?}"
    );

    let rest = ticks(&mut scene, 25);
    let gone: Vec<&Seen> = rest
        .iter()
        .filter(|seen| matches!(seen, Seen::Signal { .. } | Seen::Despawn { .. }))
        .collect();
    assert_eq!(
        gone,
        [
            &Seen::Signal {
                id: FIRST_ID,
                signal: MobSignal::Removed,
            },
            &Seen::Despawn { id: FIRST_ID },
        ],
        "the smoke, then the removal, once"
    );
    assert!(
        scene.app.world().get_entity(pig).is_err(),
        "the pig left the world"
    );
}

#[test]
fn a_stationary_mob_publishes_only_swing_signal_and_respects_cooldown() {
    let mut scene = scene();
    let pig = summon(&mut scene, DVec3::new(4.5, f64::from(FLOOR), 4.5));
    ticks(&mut scene, 5);

    let stab = MobSwingSpec::new(Hand::Off, SwingAnimationKind::Stab, 6);
    assert!(swing(&mut scene, pig, stab), "first swing starts");
    assert!(
        !swing(&mut scene, pig, stab),
        "SWG-STA-004: second swing in same tick is suppressed"
    );
    assert_eq!(
        tick(&mut scene),
        [Seen::Signal {
            id: FIRST_ID,
            signal: MobSignal::Swing {
                hand: Hand::Off,
                animation: SwingAnimationKind::Stab,
                duration: 6,
            },
        }],
        "a stationary mob swing publishes only the signal without an extraneous move"
    );

    for _ in 0..3 {
        assert!(!swing(&mut scene, pig, stab), "cooldown blocks T+1..T+3");
        assert_eq!(tick(&mut scene), [], "blocked swing publishes nothing");
    }

    let whack = MobSwingSpec::new(Hand::Main, SwingAnimationKind::Whack, 6);
    assert!(swing(&mut scene, pig, whack), "D=6 unlocks at T+4");
    assert_eq!(
        tick(&mut scene),
        [Seen::Signal {
            id: FIRST_ID,
            signal: MobSignal::Swing {
                hand: Hand::Main,
                animation: SwingAnimationKind::Whack,
                duration: 6,
            },
        }],
        "unlocked swing emits one signal"
    );
}
