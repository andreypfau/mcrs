use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use glam::DVec3;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_entity_motion_replication::NetworkIds;
use mcrs_minecraft_engine_mob_attributes::AttributeEdit;
use mcrs_minecraft_engine_mob_attributes::MobAttributesPlugin;
use mcrs_minecraft_engine_physics::FLOOR;
use mcrs_minecraft_engine_physics::MobExtras;
use mcrs_minecraft_engine_physics::MobIndex;
use mcrs_minecraft_engine_physics::MobSpawnRequest;
use mcrs_minecraft_engine_physics::SpawnSetup;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::InstanceModifier;
use mcrs_minecraft_entity_attribute::ModifierKind;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::ReplicationData;
use mcrs_minecraft_replication::SpawnLook;

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

fn hurt(scene: &mut TestScene, pig: Entity, amount: f32) {
    let tick = scene.tick;
    let mut pending = scene.app.world_mut().resource_mut::<PendingHits>();
    let sequence = u32::try_from(pending.len()).expect("a short queue");
    pending.push(DamageRequest::new(
        pig,
        Hit::new(amount, DamageSource::of(7)),
        WriterKey::Command { sequence: 0 },
        tick,
        sequence,
    ));
}

fn outcome(scene: &TestScene, pig: Entity) -> Option<HitOutcome> {
    (scene.app.world().resource::<Messages<Damaged>>())
        .iter_current_update_messages()
        .filter(|damaged| damaged.target == pig)
        .map(|damaged| damaged.report.outcome)
        .last()
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

    hurt(&mut scene, pig, 1.0);
    let hurt_tick = tick(&mut scene);
    assert_eq!(outcome(&scene, pig), Some(HitOutcome::Full));
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

fn summon_pig(scene: &mut TestScene) -> Entity {
    summon(scene, DVec3::new(4.5, f64::from(FLOOR), 4.5))
}

fn drop_item(scene: &mut TestScene) -> Entity {
    let body = Body {
        position: DVec3::new(4.5, f64::from(FLOOR), 4.5),
        ..Body::default()
    };
    let item = scene
        .app
        .world_mut()
        .spawn((
            MotionState(EntityState::new(EntityUuid::from_u128(7), body, None)),
            MotionKind(MotionRow::item()),
            Appearance {
                kind: &EntityTypeData::ITEM,
                look: SpawnLook::Mob(MobLooks::default()),
            },
        ))
        .id();
    tick(scene);
    item
}

fn join_player(scene: &mut TestScene) -> Entity {
    scene.app.world_mut().spawn(Owner(PlayerSession(3))).id()
}

#[test]
fn a_network_id_names_its_entity_until_the_entity_despawns() {
    let cases: [(&str, fn(&mut TestScene) -> Entity, i32); 3] = [
        ("mob", summon_pig, 2),
        ("item", drop_item, 2),
        ("player", join_player, 7),
    ];
    for (name, spawn, id) in cases {
        let mut scene = scene();
        let entity = spawn(&mut scene);
        let world = scene.app.world_mut();
        assert_eq!(
            world.resource::<NetworkIds>().get(id),
            Some(entity),
            "{name}"
        );
        world.despawn(entity);
        assert_eq!(world.resource::<NetworkIds>().get(id), None, "{name}");
    }
}
