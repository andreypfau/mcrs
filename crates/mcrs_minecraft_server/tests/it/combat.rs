//! A player takes damage, dies and respawns through the dimension combat runtime.

use bevy_ecs::prelude::*;
use mcrs_minecraft_combat::{DamageRequest, DamageSource, Hit, PlayerVitals as Health};
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{ClientboundDamageEvent, ClientboundEntityEvent};
use mcrs_minecraft_protocol_game::serverbound::{
    ClientCommandAction, ServerboundAttack, ServerboundChatCommand, ServerboundClientCommand,
};
use mcrs_minecraft_server::{VanillaCombatPlugin, VanillaEntityPhysicsPlugin};

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer};

const PLAYER: Uuid = Uuid::from_u128(7);
const OTHER: Uuid = Uuid::from_u128(8);

fn start() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.add_plugins(VanillaCombatPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    PlayServer::start(app, &[(PLAYER, 4)], 0)
}

fn health_of(server: &mut PlayServer, anchor: Entity) -> f32 {
    let player = server.player(anchor);
    server
        .world()
        .get::<Health>(player)
        .expect("the player is combat attached")
        .health
}

fn hurt(server: &mut PlayServer, anchor: Entity, amount: f32) {
    let player = server.player(anchor);
    server.world().write_message(DamageRequest::player(
        player,
        Hit::from_source(amount, DamageSource::new(10)),
    ));
}

#[test]
fn a_damaged_player_sees_the_health_and_the_attributes() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, joined) = server.join(PLAYER, "fighter");
    command(&mut server, anchor, "gamemode survival");

    let initial = joined.iter().find_map(|packet| match packet {
        PacketPayload::SetHealth(health) => Some(health.clone()),
        _ => None,
    });
    assert_eq!(initial.map(|health| health.health), Some(20.0));
    let packets = server.ticks(anchor, 1);
    let attributes = joined
        .iter()
        .chain(packets.iter())
        .find_map(|packet| match packet {
            PacketPayload::UpdateAttributes(attributes) => Some(attributes),
            _ => None,
        })
        .expect("the join sends the player attributes");
    assert!(
        attributes.attributes.iter().any(|attribute| {
            attribute.attribute == mcrs_minecraft_entity::attribute_keys::Attribute::MaxHealth
                && attribute.base == 20.0
        }),
        "the client sees the maximum health"
    );
    assert!(
        attributes.attributes.iter().any(|attribute| {
            attribute.attribute == mcrs_minecraft_entity::attribute_keys::Attribute::Armor
                && attribute.base == 0.0
        }),
        "the client sees the armor"
    );

    hurt(&mut server, anchor, 5.0);
    let packets = server.ticks(anchor, 2);
    assert_eq!(health_of(&mut server, anchor), 15.0);
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, PacketPayload::DamageEvent(_))),
        "the hit sends its damage event"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            PacketPayload::SetHealth(health) if health.health == 15.0
        )),
        "the client sees the new health"
    );
}

#[test]
fn a_dead_player_sees_the_death_animation_and_the_death_screen() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "slain");
    command(&mut server, anchor, "gamemode survival");
    hurt(&mut server, anchor, 100.0);
    let packets = server.ticks(anchor, 2);
    assert_eq!(health_of(&mut server, anchor), 0.0);
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            PacketPayload::EntityEvent(event)
                if event.entity_status == mcrs_minecraft_protocol_game::clientbound::ClientboundEntityEvent::DIED
        )),
        "the death animation reaches the client"
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, PacketPayload::PlayerCombatKill(_))),
        "the death screen reaches the client"
    );
}

#[test]
fn dmg_net_012_other_players_see_the_damage_and_the_death_animation() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "fighter");
    let (watcher, _) = server.join(OTHER, "watcher");
    command(&mut server, anchor, "gamemode survival");
    let target = player_network_id(server.session_of(anchor).expect("the target has a session"));

    hurt(&mut server, anchor, 5.0);
    let packets = server.ticks(watcher, 2);
    let damage = host_app::replicated(&packets)
        .into_iter()
        .find(|(id, _)| *id == ClientboundDamageEvent::ID)
        .expect("the other player gets the damage event");
    let mut body = damage.1.as_slice();
    let damage = ClientboundDamageEvent::decode(&mut body).expect("the damage event decodes");
    assert_eq!(
        damage.entity_id.0, target,
        "the event names the hurt player"
    );

    hurt(&mut server, anchor, 100.0);
    let packets = server.ticks(watcher, 2);
    let death = host_app::replicated(&packets)
        .into_iter()
        .find(|(id, _)| *id == ClientboundEntityEvent::ID)
        .expect("the other player gets the death animation");
    let mut body = death.1.as_slice();
    let death = ClientboundEntityEvent::decode(&mut body).expect("the death animation decodes");
    assert_eq!(
        death.entity_id, target,
        "the animation names the dead player"
    );
    assert_eq!(death.entity_status, ClientboundEntityEvent::DIED);
}

#[test]
fn a_respawn_restores_the_health_and_clears_the_death() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "returning");
    command(&mut server, anchor, "gamemode survival");
    hurt(&mut server, anchor, 100.0);
    server.ticks(anchor, 2);
    let packets = server.send(
        anchor,
        &ServerboundClientCommand {
            action: ClientCommandAction::PerformRespawn,
        },
    );
    assert_eq!(health_of(&mut server, anchor), 20.0);
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, PacketPayload::Respawn(_))),
        "the respawn packet reaches the client"
    );
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, PacketPayload::PlayerPosition(_))),
        "the client is put at the spawn"
    );
    let player = server.player(anchor);
    assert!(
        server
            .world()
            .get::<mcrs_minecraft_combat::Dying>(player)
            .is_none(),
        "the respawn clears the death state"
    );
}

#[test]
fn two_respawn_requests_in_one_tick_respawn_the_player_once() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "returning");
    survival(&mut server, anchor);
    hurt(&mut server, anchor, 100.0);
    server.ticks(anchor, 2);
    let request = ServerboundClientCommand {
        action: ClientCommandAction::PerformRespawn,
    };
    server.queue(anchor, &request);
    server.queue(anchor, &request);
    let packets = server.ticks(anchor, 2);
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, PacketPayload::Respawn(_)))
            .count(),
        1,
        "the second request of the same tick does not respawn the player again"
    );
    assert_eq!(health_of(&mut server, anchor), 20.0);
}

/// PLD-DRP-001 and PLD-DRP-004: a dead survival player scatters the inventory, including the armour slots.
#[test]
fn dmg_death_002_death_drops_the_inventory() {
    use mcrs_minecraft_engine_item::HolderCommandsExt;
    use mcrs_minecraft_item_stack::{Count, ItemStack, Kind};

    let mut server = start();
    load_stone_column(&mut server);
    if !server
        .world()
        .contains_resource::<mcrs_minecraft_item_table::ItemTable>()
    {
        server
            .world()
            .insert_resource(mcrs_minecraft_item_table::ItemTable::leak(vec![
                Vec::new(),
                Vec::new(),
            ]));
    }
    let (anchor, _) = server.join(PLAYER, "looter");
    survival(&mut server, anchor);
    let player = server.player(anchor);
    let count = Count::new(1).expect("one item");
    let mut values = vec![None; 47];
    values[0] = Some(ItemStack::new(Kind(1), count));
    values[36] = Some(ItemStack::new(Kind(1), count));
    values[46] = Some(ItemStack::new(Kind(1), count));
    server
        .world()
        .commands()
        .entity(player)
        .load_inventory(values);
    server.ticks(anchor, 1);
    assert_eq!(
        server
            .world()
            .get::<mcrs_minecraft_engine_item::Slots>(player)
            .expect("the player holds slots")
            .occupied()
            .count(),
        3
    );

    server
        .world()
        .write_message(mcrs_minecraft_combat::DamageRequest::player(
            player,
            mcrs_minecraft_combat::Hit::from_source(
                1000.0,
                mcrs_minecraft_combat::DamageSource::new(10),
            ),
        ));
    server.ticks(anchor, 4);
    let dropped = server
        .world()
        .query::<&mcrs_minecraft_engine_item::Dropped>()
        .iter(server.world())
        .count();
    assert_eq!(
        dropped, 3,
        "the inventory, the armour and the carried stack leave the dead player"
    );
    assert_eq!(
        server
            .world()
            .get::<mcrs_minecraft_engine_item::Slots>(player)
            .expect("the player still holds slots")
            .occupied()
            .count(),
        0
    );
}

fn command(server: &mut PlayServer, anchor: Entity, line: &str) {
    server.send(
        anchor,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
}

fn summoned_mob(server: &mut PlayServer, x: f64) -> (u64, f32) {
    let rows = server.world().resource::<EntityRows>();
    rows.living_rows()
        .find(|(_, state)| (state.body.position.x - x).abs() < 1.0)
        .map(|(at, state)| {
            (
                at.id.0,
                state.living.expect("the summon is living").vitals.health,
            )
        })
        .expect("the summon has a row")
}

fn mob_health(server: &mut PlayServer, id: u64) -> f32 {
    let rows = server.world().resource::<EntityRows>();
    rows.living_rows()
        .find(|(at, _)| at.id.0 == id)
        .and_then(|(_, state)| state.living)
        .expect("the mob is alive")
        .vitals
        .health
}

/// DMG-SRC-002: a bare-hand player attack reaches a mob within the survival entity range, and
/// the same attack at a mob past that range changes no health.
#[test]
fn dmg_src_002_a_player_attack_reaches_a_mob_only_within_range() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "fighter");
    command(&mut server, anchor, "summon minecraft:pig 10 64 8");
    command(&mut server, anchor, "summon minecraft:pig 20 64 8");
    server.ticks(anchor, 2);

    let (near, near_before) = summoned_mob(&mut server, 10.5);
    let (far, far_before) = summoned_mob(&mut server, 20.5);
    server.send(
        anchor,
        &ServerboundAttack {
            entity_id: VarInt(i32::try_from(near).expect("the row id fits")),
        },
    );
    assert_eq!(
        mob_health(&mut server, near),
        near_before - 1.0,
        "the pig in reach loses one bare-hand hit"
    );
    server.send(
        anchor,
        &ServerboundAttack {
            entity_id: VarInt(i32::try_from(far).expect("the row id fits")),
        },
    );
    assert_eq!(
        mob_health(&mut server, far),
        far_before,
        "the pig past reach keeps its health"
    );
}

/// DMG-HIT-001: one player may send an attack packet every four ticks, whatever it hits.
#[test]
fn dmg_src_002_a_player_attack_rate_limits_the_packets() {
    let mut server = start();
    server.app.world_mut().insert_resource(GameTime(0));
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "fighter");
    command(&mut server, anchor, "summon minecraft:pig 10 64 8");
    command(&mut server, anchor, "summon minecraft:pig 11 64 8");
    server.ticks(anchor, 2);
    let (first, _) = summoned_mob(&mut server, 10.5);
    let (second, second_before) = summoned_mob(&mut server, 11.5);

    server.send(
        anchor,
        &ServerboundAttack {
            entity_id: VarInt(i32::try_from(first).expect("the row id fits")),
        },
    );
    server.send(
        anchor,
        &ServerboundAttack {
            entity_id: VarInt(i32::try_from(second).expect("the row id fits")),
        },
    );
    assert_eq!(
        mob_health(&mut server, second),
        second_before,
        "the second packet inside the interval is ignored"
    );

    server.app.world_mut().resource_mut::<GameTime>().0 += 4;
    server.send(
        anchor,
        &ServerboundAttack {
            entity_id: VarInt(i32::try_from(second).expect("the row id fits")),
        },
    );
    assert_eq!(
        mob_health(&mut server, second),
        second_before - 1.0,
        "the next packet after the interval lands"
    );
}

/// DMG-SRC-002: a player attack names another player as its target, and the target's health
/// falls.
#[test]
fn dmg_src_002_a_player_attack_hits_another_player() {
    let mut server = start();
    load_stone_column(&mut server);
    let (attacker, _) = server.join(PLAYER, "attacker");
    let (target, _) = server.join(OTHER, "target");
    let target_entity = server.player(target);
    server.world().entity_mut(target_entity).insert(
        mcrs_minecraft_engine::ability::PlayerGameMode(GameMode::Survival),
    );
    let owner = *server
        .world()
        .get::<mcrs_minecraft_engine_world::session::Owner>(target_entity)
        .expect("the target has an owner");
    let before = health_of(&mut server, target);
    server.send(
        attacker,
        &ServerboundAttack {
            entity_id: VarInt(player_network_id(owner.0)),
        },
    );
    assert_eq!(
        health_of(&mut server, target),
        before - 1.0,
        "the attacked player loses one bare-hand hit"
    );
    assert_eq!(health_of(&mut server, attacker), 20.0);
}

fn survival(server: &mut PlayServer, anchor: Entity) {
    command(server, anchor, "gamemode survival");
}

fn set_block(server: &mut PlayServer, at: mcrs_voxel::BlockPos, state: mcrs_voxel::BlockStateId) {
    use mcrs_minecraft_engine_world::voxel_update::BlockSetRequest;
    use mcrs_minecraft_engine_world::world::dimension::Dimension;
    use mcrs_minecraft_simulation::BlockUpdateFlags;
    let world = server.world();
    let dimension = world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .expect("one dimension");
    world.write_message(BlockSetRequest {
        dimension,
        pos: at,
        new_state: state,
        flags: BlockUpdateFlags::ALL,
        recursion_left: 1,
    });
}

fn place(server: &mut PlayServer, positions: &[(i32, i32, i32)], name: &str) {
    let state = crate::support::standalone_corpus()
        .0
        .default_state(name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    for &(x, y, z) in positions {
        set_block(server, mcrs_voxel::BlockPos::new(x, y, z), state);
    }
}

/// ENV-FIR-004: a survival player in fire catches fire and loses health.
#[test]
fn dmg_env_002_a_player_in_fire_burns() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "burner");
    survival(&mut server, anchor);
    place(&mut server, &[(8, 64, 8)], "minecraft:fire");
    let packets = server.ticks(anchor, 25);
    let health = packets.iter().find_map(|packet| match packet {
        PacketPayload::SetHealth(health) if health.health == 18.5 => Some(health.health),
        _ => None,
    });
    assert_eq!(
        health,
        Some(18.5),
        "the burning player loses 1.5 and the client sees it"
    );
}

/// ENV-AIR-002: only the eye in water starts drowning, so waist-deep water keeps the air.
#[test]
fn dmg_env_001_a_player_waist_deep_in_water_keeps_air() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "wading");
    survival(&mut server, anchor);
    let player = server.player(anchor);
    let position = server
        .world()
        .get::<mcrs_minecraft_engine_world::entity::physics::Transform>(player)
        .expect("the player has a position")
        .translation;
    place(
        &mut server,
        &[(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        )],
        "minecraft:water",
    );
    server.ticks(anchor, 2);
    let health = server
        .world()
        .get::<mcrs_minecraft_combat::PlayerVitals>(player)
        .expect("the player has air");
    assert_eq!(
        health.0.air, 300,
        "water at the feet but not at the eye does not start drowning"
    );
}

#[test]
fn dmg_net_009_equipment_changes_reach_the_other_players() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "armoured");
    let (_, _) = server.join(OTHER, "watcher");
    if !server
        .world()
        .contains_resource::<mcrs_minecraft_item_table::ItemTable>()
    {
        server
            .world()
            .insert_resource(mcrs_minecraft_item_table::ItemTable::leak(vec![
                Vec::new(),
                Vec::new(),
            ]));
    }
    let player = server.player(anchor);
    let mut values = vec![None; 41];
    values[39] = Some(mcrs_minecraft_item_stack::ItemStack::new(
        mcrs_minecraft_item_stack::Kind(1),
        mcrs_minecraft_item_stack::Count::new(1).expect("one item"),
    ));
    {
        use mcrs_minecraft_engine_item::HolderCommandsExt;
        server
            .world()
            .commands()
            .entity(player)
            .load_inventory(values);
    }
    let packets = server.ticks_all(2);
    assert!(
        packets.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::SetEquipment(equipment)
                if equipment.slots.iter().any(|(slot, stack)| {
                    *slot == mcrs_minecraft_protocol_game::entity::EquipmentSlot::Head
                        && stack.id.number() == 1
                })
        )),
        "the head slot reaches the other player"
    );
}

#[test]
fn dmg_net_010_an_effect_is_applied_and_removed_on_the_wire() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "effect");
    let player = server.player(anchor);
    server
        .world()
        .get_mut::<mcrs_minecraft_combat::PlayerEffects>(player)
        .expect("the player has effects")
        .add(mcrs_minecraft_combat::ActiveEffect::new(
            mcrs_minecraft_combat::MobEffect::Speed,
            0,
            100,
        ));
    let applied = server.ticks_all(1);
    assert!(
        applied.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::UpdateMobEffect(effect)
                if effect.effect.0 == mcrs_minecraft_combat::MobEffect::Speed.id().number() as i32
                    && (1..=100).contains(&effect.duration.0)
        )),
        "the effect reaches the client with its duration"
    );
    server
        .world()
        .get_mut::<mcrs_minecraft_combat::PlayerEffects>(player)
        .expect("the player has effects")
        .remove(mcrs_minecraft_combat::MobEffect::Speed);
    let removed = server.ticks_all(1);
    assert!(
        removed.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::RemoveMobEffect(effect)
                if effect.effect.0 == mcrs_minecraft_combat::MobEffect::Speed.id().number() as i32
        )),
        "the removed effect reaches the client"
    );
}

#[test]
fn dmg_net_011_experience_points_change_the_level_on_the_wire() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "experience");
    let player = server.player(anchor);
    server
        .world()
        .get_mut::<mcrs_minecraft_combat::PlayerExperience>(player)
        .expect("the player has experience")
        .add_points(10);
    let packets = server.ticks_all(1);
    assert!(
        packets.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::SetExperience(experience)
                if experience.experience_level.0 == 1
                    && experience.total_experience.0 == 10
                    && (experience.experience_progress - 1.0 / 3.0).abs() < 1.0e-5
        )),
        "the client sees the level and the progress"
    );
}
