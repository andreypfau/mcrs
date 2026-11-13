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
use mcrs_minecraft_engine::player_state::{
    Attribute as SavedAttribute, Effect as SavedEffect, PlayerFileState,
};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};

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
fn player_file_state_loads_and_saves_combat_state() {
    let mut app = App::new();
    app.add_plugins(CombatPlugin);
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<ChangeDimensionRequest>();
    app.init_resource::<GameTime>();

    let mut file = PlayerFileState {
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
            hidden_effect: None,
        }],
        attributes: vec![SavedAttribute {
            id: "minecraft:max_health".to_owned(),
            base: 30.0,
            modifiers: Vec::new(),
        }],
        ..PlayerFileState::default()
    };
    let entity = app
        .world_mut()
        .spawn((Player, Transform::default(), file.clone()))
        .id();

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

    file = app.world().get::<PlayerFileState>(entity).unwrap().clone();
    assert_eq!(file.health, 7.5);
    assert_eq!(file.food_level, 0);
    assert_eq!(file.xp_level, 5);
    assert_eq!(file.active_effects.len(), 1);
    assert_eq!(file.active_effects[0].id, "minecraft:speed");

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

    let file = app.world().get::<PlayerFileState>(entity).unwrap();
    assert_eq!(file.health, 3.0);
    assert_eq!(file.food_level, 4);
    assert_eq!(file.xp_level, 9);
    assert!(file.active_effects.is_empty());
    assert_eq!(
        file.attributes
            .iter()
            .find(|attribute| attribute.id == "minecraft:max_health")
            .unwrap()
            .base,
        24.0
    );
}
