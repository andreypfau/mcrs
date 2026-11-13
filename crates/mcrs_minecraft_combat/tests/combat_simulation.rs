use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use mcrs_minecraft_combat::{
    ActiveEffect, Armor, ArmorToughness, BaseMaxHealth, CombatPlugin, DamageDealt, DamageRequest,
    DamageTarget, DeathEvent, MobEffect, PlayerEffects, PlayerFood as FoodData, PlayerHitRecord,
    PlayerVitals as Health,
};
use mcrs_minecraft_damage::{DamageSource, Hit};
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::persisted_registry::{PersistedRegistry, PersistedSnapshot};
use mcrs_minecraft_engine::player_state::{
    Attribute as SavedAttribute, Effect as SavedEffect, EffectData, PlayerFileState,
};
use mcrs_minecraft_engine::{PlayerPersistencePlugin, apply_join_snapshot};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_nbt::tag_serializer::to_nbt_tag;
use mcrs_minecraft_save::PlayerDat;
use std::path::Path;

#[test]
fn integration_combat_damage_flow_and_death() {
    let mut app = App::new();
    app.add_plugins(CombatPlugin);
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<ChangeDimensionRequest>();
    app.init_resource::<GameTime>();

    // Entity with 20 HP, 10 Armor, 4 Toughness
    let entity = app
        .world_mut()
        .spawn((
            Health::new(20.0, 20.0),
            Armor {
                armor: 10.0,
                toughness: 4.0,
            },
            ArmorToughness(4.0),
            PlayerHitRecord::default(),
        ))
        .id();

    // Send damage request of 10.0
    let mut damage_writer = app.world_mut().resource_mut::<Messages<DamageRequest>>();
    damage_writer.write(DamageRequest::player(
        entity,
        Hit::from_source(10.0, DamageSource::new(0)),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    assert_eq!(health.health, 12.666_666);

    let dealt_messages = app.world().resource::<Messages<DamageDealt>>();
    assert_eq!(dealt_messages.len(), 1);

    // Immediate second hit of smaller amount is rejected due to 10-tick invulnerability
    let mut damage_writer = app.world_mut().resource_mut::<Messages<DamageRequest>>();
    damage_writer.write(DamageRequest::player(
        entity,
        Hit::from_source(2.0, DamageSource::new(0)),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let dealt_messages = app.world().resource::<Messages<DamageDealt>>();
    // Should still only have the previous message or rejected outcome
    assert_eq!(dealt_messages.len(), 1);

    // Fatal blow of 1000.0 damage
    let mut damage_writer = app.world_mut().resource_mut::<Messages<DamageRequest>>();
    damage_writer.write(DamageRequest::player(
        entity,
        Hit::from_source(1000.0, DamageSource::new(0)),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    assert_eq!(health.health, 0.0);
    assert!(!health.is_alive());

    let death_messages = app.world().resource::<Messages<DeathEvent>>();
    assert_eq!(death_messages.len(), 1);
}

#[test]
fn integration_hunger_exhaustion_and_healing() {
    let mut app = App::new();
    app.add_plugins(CombatPlugin);
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<ChangeDimensionRequest>();
    app.init_resource::<GameTime>();

    let mut health = Health::new(20.0, 20.0);
    health.health = 10.0;
    let entity = app
        .world_mut()
        .spawn((Player, health, FoodData::default()))
        .id();

    for age in 0..9 {
        app.world_mut().resource_mut::<GameTime>().0 = age;
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(app.world().get::<Health>(entity).unwrap().health, 10.0);
    }
    app.world_mut().resource_mut::<GameTime>().0 = 9;
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(
        app.world().get::<Health>(entity).unwrap().health,
        10.833_333
    );
}

#[test]
fn integration_status_effects_application_and_ticking() {
    let mut app = App::new();
    app.add_plugins(CombatPlugin);
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<ChangeDimensionRequest>();
    app.init_resource::<GameTime>();

    let mut effects = PlayerEffects::new();
    effects.add(ActiveEffect::new(MobEffect::Regeneration, 0, 50));
    effects.add(ActiveEffect::new(MobEffect::Poison, 0, 25));

    let mut health = Health::new(20.0, 20.0);
    health.health = 15.0;

    let entity = app.world_mut().spawn((effects, health)).id();

    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    // At duration 50, regeneration tick triggered healing 1.0 -> 16.0
    assert_eq!(health.health, 16.0);
    let _ = DamageTarget::Player(entity);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test covers the whole player file round trip"
)]
fn player_file_state_loads_and_saves_combat_state() {
    let mut app = App::new();
    app.add_plugins((PlayerPersistencePlugin, CombatPlugin));
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<ChangeDimensionRequest>();
    app.init_resource::<GameTime>();

    let file = PlayerFileState {
        health: 7.5,
        absorption_amount: 2.0,
        hurt_time: 3,
        fire: 20,
        air: 100,
        ticks_frozen: 4,
        fall_distance: 2.5,
        on_ground: false,
        food_level: 0,
        food_tick_timer: 7,
        food_saturation_level: 1.5,
        food_exhaustion_level: 0.25,
        xp_level: 5,
        xp_total: 100,
        xp_progress: 0.25,
        score: 3,
        active_effects: vec![SavedEffect {
            id: "minecraft:speed".to_owned(),
            amplifier: 1,
            duration: 100,
            ambient: true,
            show_particles: false,
            show_icon: Some(true),
            hidden_effect: Some(Box::new(EffectData {
                amplifier: 0,
                duration: 60,
                ambient: false,
                show_particles: true,
                show_icon: Some(true),
                hidden_effect: None,
            })),
        }],
        attributes: vec![SavedAttribute {
            id: "minecraft:max_health".to_owned(),
            base: 30.0,
            modifiers: Vec::new(),
        }],
        ..PlayerFileState::default()
    };
    let root = match to_nbt_tag(&file).unwrap() {
        mcrs_minecraft_nbt::tag::NbtTag::Compound(root) => root,
        _ => panic!("a player file is a compound"),
    };
    let dat = PlayerDat::new(mcrs_minecraft_core::VERSION.world_version, root);
    let snapshot = {
        let registry = app.world().resource::<PersistedRegistry>();
        PersistedSnapshot::from_dat(&dat, registry, Path::new("test.dat")).unwrap()
    };
    let entity = app.world_mut().spawn((Player, Transform::default())).id();
    apply_join_snapshot(app.world_mut(), entity, &snapshot);

    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    assert_eq!(health.health, 7.5);
    assert_eq!(health.max_health, 30.0);
    assert_eq!(health.absorption, 2.0);
    assert!(health.hurt_time > 0, "the hurt time survives the load");
    assert_eq!(health.fire_ticks, 20);
    assert_eq!(health.air, 100);
    assert_eq!(health.frozen_ticks, 4);
    assert_eq!(app.world().get::<BaseMaxHealth>(entity).unwrap().0, 30.0);
    let food = app.world().get::<FoodData>(entity).unwrap();
    assert_eq!(food.food_level, 0);
    assert_eq!(food.tick_timer, 9);
    let experience = app
        .world()
        .get::<mcrs_minecraft_combat::PlayerExperience>(entity)
        .unwrap();
    assert_eq!(experience.level, 5);
    assert_eq!(experience.total, 100);
    assert_eq!(experience.score, 3);
    assert!(
        app.world()
            .get::<PlayerEffects>(entity)
            .unwrap()
            .has(MobEffect::Speed)
    );

    let saved = capture(&mut app, entity).unwrap();
    assert_eq!(saved.get("Health").unwrap().extract_float().unwrap(), 7.5);
    assert_eq!(saved.get("foodLevel").unwrap().extract_int().unwrap(), 0);
    assert_eq!(saved.get("XpLevel").unwrap().extract_int().unwrap(), 5);
    let effects = saved.get("active_effects").unwrap().extract_list().unwrap();
    assert_eq!(effects.len(), 1);
    let effect = effects[0].extract_compound().unwrap();
    assert_eq!(
        effect.get("id").unwrap().extract_string().unwrap(),
        "minecraft:speed"
    );
    let hidden = effect
        .get("hidden_effect")
        .unwrap()
        .extract_compound()
        .unwrap();
    assert_eq!(hidden.get("amplifier").unwrap().extract_byte().unwrap(), 0);
    assert_eq!(hidden.get("duration").unwrap().extract_int().unwrap(), 58);

    app.world_mut().get_mut::<Health>(entity).unwrap().health = 3.0;
    app.world_mut()
        .get_mut::<FoodData>(entity)
        .unwrap()
        .food_level = 4;
    app.world_mut()
        .get_mut::<mcrs_minecraft_combat::PlayerExperience>(entity)
        .unwrap()
        .level = 9;
    app.world_mut().get_mut::<BaseMaxHealth>(entity).unwrap().0 = 24.0;
    app.world_mut()
        .get_mut::<PlayerEffects>(entity)
        .unwrap()
        .remove(MobEffect::Speed);

    app.world_mut().run_schedule(FixedUpdate);

    let saved = capture(&mut app, entity).unwrap();
    assert_eq!(saved.get("Health").unwrap().extract_float().unwrap(), 3.0);
    assert_eq!(saved.get("foodLevel").unwrap().extract_int().unwrap(), 4);
    assert_eq!(saved.get("XpLevel").unwrap().extract_int().unwrap(), 9);
    assert!(saved.get("active_effects").is_none());
    let attributes = saved.get("attributes").unwrap().extract_list().unwrap();
    let max_health = attributes
        .iter()
        .filter_map(|attribute| attribute.extract_compound())
        .find(|attribute| {
            attribute
                .get("id")
                .and_then(mcrs_minecraft_nbt::tag::NbtTag::extract_string)
                == Some("minecraft:max_health")
        })
        .unwrap();
    assert_eq!(
        max_health.get("base").unwrap().extract_double().unwrap(),
        24.0
    );
}

fn capture(app: &mut App, entity: bevy_ecs::entity::Entity) -> Option<PersistedSnapshot> {
    let registry = app.world_mut().remove_resource::<PersistedRegistry>()?;
    let snapshot = registry.capture_file(app.world_mut(), entity).ok();
    app.world_mut().insert_resource(registry);
    snapshot
}
