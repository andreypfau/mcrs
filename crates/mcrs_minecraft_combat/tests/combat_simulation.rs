use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use mcrs_minecraft_combat::{
    ActiveEffect, ActiveEffects, Armor, ArmorToughness, CombatPlugin, CombatPosition,
    DamageCooldown, DamageDealt, DamageRequest, DeathEvent, FoodData, Health, MobEffect,
};
use mcrs_minecraft_damage::{DamageSource, Hit};
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_protocol_game::Difficulty;

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
            Health::new(20.0),
            Armor(10.0),
            ArmorToughness(4.0),
            DamageCooldown::default(),
            CombatPosition::from_coords(0.0, 64.0, 0.0),
        ))
        .id();

    // Send damage request of 10.0
    let mut damage_writer = app.world_mut().resource_mut::<Messages<DamageRequest>>();
    damage_writer.write(DamageRequest::new(
        entity,
        Hit::from_source(10.0, DamageSource::new(0)),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    // With armor 10 and toughness 4, damage is significantly reduced (below 10.0)
    assert!(health.current < 20.0);
    assert!(health.current > 10.0);

    let dealt_messages = app.world().resource::<Messages<DamageDealt>>();
    assert_eq!(dealt_messages.len(), 1);

    // Immediate second hit of smaller amount is rejected due to 10-tick invulnerability
    let mut damage_writer = app.world_mut().resource_mut::<Messages<DamageRequest>>();
    damage_writer.write(DamageRequest::new(
        entity,
        Hit::from_source(2.0, DamageSource::new(0)),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let dealt_messages = app.world().resource::<Messages<DamageDealt>>();
    // Should still only have the previous message or rejected outcome
    assert_eq!(dealt_messages.len(), 1);

    // Fatal blow of 1000.0 damage
    let mut damage_writer = app.world_mut().resource_mut::<Messages<DamageRequest>>();
    damage_writer.write(DamageRequest::new(
        entity,
        Hit::from_source(1000.0, DamageSource::new(0)),
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    assert_eq!(health.current, 0.0);
    assert!(health.is_dead());

    let death_messages = app.world().resource::<Messages<DeathEvent>>();
    assert_eq!(death_messages.len(), 1);
}

#[test]
fn integration_hunger_exhaustion_and_healing() {
    let mut food = FoodData::new(20, 5.0);
    let mut health = Health::new(20.0);
    health.current = 10.0;

    // Fast regeneration with full food and saturation: heals every 10 ticks
    for _ in 0..9 {
        let (heal, _) = food.tick(health.current, health.max, true, Difficulty::Normal);
        health.heal(heal);
        assert_eq!(health.current, 10.0);
    }
    // 10th tick heals
    let (heal, _) = food.tick(health.current, health.max, true, Difficulty::Normal);
    health.heal(heal);
    assert!(health.current > 10.0);
}

#[test]
fn integration_status_effects_application_and_ticking() {
    let mut app = App::new();
    app.add_plugins(CombatPlugin);
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<ChangeDimensionRequest>();
    app.init_resource::<GameTime>();

    let mut effects = ActiveEffects::new();
    effects.add(ActiveEffect::new(MobEffect::Regeneration, 0, 50));
    effects.add(ActiveEffect::new(MobEffect::Poison, 0, 25));

    let mut health = Health::new(20.0);
    health.current = 15.0;

    let entity = app.world_mut().spawn((effects, health)).id();

    app.world_mut().run_schedule(FixedUpdate);

    let health = app.world().get::<Health>(entity).unwrap();
    // At duration 50, regeneration tick triggered healing 1.0 -> 16.0
    assert_eq!(health.current, 16.0);
}
