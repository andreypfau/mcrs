//! A player takes damage, dies and respawns through the dimension combat runtime.

use bevy_ecs::prelude::*;
use mcrs_minecraft_combat::{DamageRequest, DamageSource, Health, Hit};
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::uuid::Uuid;
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
        .current
}

fn hurt(server: &mut PlayServer, anchor: Entity, amount: f32) {
    let player = server.player(anchor);
    server.world().write_message(DamageRequest::new(
        player,
        Hit::from_source(amount, DamageSource::new(10)),
    ));
}

#[test]
fn a_damaged_player_sees_health_dies_and_respawns() {
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

    server.ticks(anchor, 25);
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
            .get::<mcrs_minecraft_combat::DeathState>(player)
            .is_none(),
        "the respawn clears the death state"
    );
}

/// DMG-DEATH-002: a dead survival player scatters the inventory, including the armour slots.
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
        .write_message(mcrs_minecraft_combat::DamageRequest::new(
            player,
            mcrs_minecraft_combat::Hit::from_source(
                1000.0,
                mcrs_minecraft_combat::DamageSource::new(
                    mcrs_minecraft_combat::damage_types::GENERIC,
                ),
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

/// DMG-ENV-003: a player five blocks past the border and its safe zone takes the border damage.
#[test]
fn dmg_env_003_a_player_outside_the_world_border_takes_damage() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "wanderer");
    survival(&mut server, anchor);
    let player = server.player(anchor);
    {
        let mut transform = server
            .world()
            .get_mut::<mcrs_minecraft_engine_world::entity::physics::Transform>(player)
            .expect("the player has a transform");
        transform.translation.x = 10.0;
    }
    server
        .world()
        .insert_resource(mcrs_minecraft_combat::WorldBorderState {
            center_x: 0.0,
            center_z: 0.0,
            size: 10.0,
            safe_zone: 0.0,
            damage_per_block: 0.2,
        });
    server.ticks(anchor, 10);
    let health = health_of(&mut server, anchor);
    assert_eq!(
        health, 19.0,
        "five blocks past the border deal one point of damage"
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
        .map(|(at, state)| (at.id.0, state.living.expect("the summon is living").health))
        .expect("the summon has a row")
}

fn mob_health(server: &mut PlayServer, id: u64) -> f32 {
    let rows = server.world().resource::<EntityRows>();
    rows.living_rows()
        .find(|(at, _)| at.id.0 == id)
        .and_then(|(_, state)| state.living)
        .expect("the mob is alive")
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
    assert!(
        mob_health(&mut server, near) < near_before,
        "the pig in reach loses health"
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
    assert!(
        health_of(&mut server, target) < before,
        "the attacked player loses health"
    );
    assert_eq!(health_of(&mut server, attacker), 20.0);
}

/// DMG-ENV-002: a mob that stands in fire catches fire, is drawn on fire, and loses health.
#[test]
fn dmg_env_002_a_mob_in_fire_burns_and_loses_health() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "fire");
    command(
        &mut server,
        anchor,
        "summon minecraft:pig 12 64 8 {NoAI:1b,PersistenceRequired:1b}",
    );
    server.ticks(anchor, 2);
    let (id, before) = summoned_mob(&mut server, 12.5);
    let fire = crate::support::standalone_corpus()
        .0
        .default_state("minecraft:fire")
        .expect("fire is a block");
    {
        use bevy_ecs::system::SystemState;
        use mcrs_minecraft_engine_world::world::dimension::Dimension;
        use mcrs_minecraft_engine_world::world::storage::view::WorldViewMut;
        use mcrs_minecraft_volume::BlockVolumeMut;
        let world = server.world();
        let dim = world
            .query_filtered::<Entity, With<Dimension>>()
            .single(world)
            .expect("the dimension");
        let mut state: SystemState<WorldViewMut> = SystemState::new(world);
        let mut view = state.get_mut(world).expect("valid params");
        view.get_mut(dim)
            .expect("the dimension")
            .set_block(mcrs_voxel::BlockPos::new(12, 64, 8), fire)
            .expect("the fire is set");
        state.apply(world);
    }
    server.ticks(anchor, 45);
    let (after, position, on_fire, fire_ticks, air) = {
        let rows = server.world().resource::<EntityRows>();
        let (_, state) = rows
            .living_rows()
            .find(|(at, _)| at.id.0 == id)
            .expect("the pig still lives");
        let living = state.living.expect("living");
        (
            living.vitals.health,
            state.body.position,
            living.on_fire(),
            living.vitals.fire_ticks,
            living.vitals.air,
        )
    };
    assert!(
        after < before,
        "the burning pig loses health: pos {position:?} fire {on_fire} ticks {fire_ticks} air {air} health {after}",
    );
    assert!(on_fire || fire_ticks > 0, "the pig burns");
}

/// DMG-ENV-001: a mob under water loses its air and then its health.
#[test]
fn dmg_env_001_a_mob_under_water_drowns() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "mob-diver");
    command(
        &mut server,
        anchor,
        "summon minecraft:pig 12 64 8 {NoAI:1b,PersistenceRequired:1b}",
    );
    server.ticks(anchor, 2);
    let (id, before) = summoned_mob(&mut server, 12.5);
    place(&mut server, &[(12, 64, 8), (12, 65, 8)], "minecraft:water");
    server.ticks(anchor, 340);
    assert!(
        mob_health(&mut server, id) < before,
        "the submerged pig drowns"
    );
}

/// DMG-ENV-003: powder snow freezes a mob after the warm-up.
#[test]
fn dmg_env_003_a_mob_in_powder_snow_freezes() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "mob-snow");
    command(
        &mut server,
        anchor,
        "summon minecraft:pig 12 64 8 {NoAI:1b,PersistenceRequired:1b}",
    );
    server.ticks(anchor, 2);
    let (id, before) = summoned_mob(&mut server, 12.5);
    place(
        &mut server,
        &[(12, 64, 8), (12, 65, 8)],
        "minecraft:powder_snow",
    );
    server.ticks(anchor, 190);
    assert!(
        mob_health(&mut server, id) < before,
        "the frozen pig loses health"
    );
}

/// DMG-ENV-004: a mob that touches a cactus loses health.
#[test]
fn dmg_env_004_a_mob_against_a_cactus_is_hurt() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "mob-cactus");
    command(
        &mut server,
        anchor,
        "summon minecraft:pig 12 64 8 {NoAI:1b,PersistenceRequired:1b}",
    );
    server.ticks(anchor, 2);
    let (id, before) = summoned_mob(&mut server, 12.5);
    place(&mut server, &[(12, 64, 8)], "minecraft:cactus");
    server.ticks(anchor, 35);
    assert!(
        mob_health(&mut server, id) < before,
        "the cactus hurts the pig"
    );
}

/// DMG-ENV-004: a mob with its eyes in a block suffocates.
#[test]
fn dmg_env_004_a_mob_with_its_eyes_in_a_block_suffocates() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "mob-wall");
    command(
        &mut server,
        anchor,
        "summon minecraft:pig 12 64 8 {NoAI:1b,PersistenceRequired:1b}",
    );
    server.ticks(anchor, 2);
    let (id, before) = summoned_mob(&mut server, 12.5);
    place(&mut server, &[(12, 65, 8)], "minecraft:stone");
    server.ticks(anchor, 35);
    assert!(
        mob_health(&mut server, id) < before,
        "the suffocating pig loses health"
    );
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

/// DMG-ENV-001: a survival player under water loses air and then health.
#[test]
fn dmg_env_001_a_player_under_water_drowns() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "diver");
    survival(&mut server, anchor);
    place(
        &mut server,
        &[(8, 64, 8), (8, 65, 8), (8, 64, 9), (9, 64, 8)],
        "minecraft:water",
    );
    server.ticks(anchor, 330);
    let health = health_of(&mut server, anchor);
    assert!(health < 20.0, "the diver loses health, got {health}");
}

/// DMG-ENV-002: a survival player in fire catches fire and loses health.
#[test]
fn dmg_env_002_a_player_in_fire_burns() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "burner");
    survival(&mut server, anchor);
    place(&mut server, &[(8, 64, 8)], "minecraft:fire");
    server.ticks(anchor, 25);
    let health = health_of(&mut server, anchor);
    assert!(
        health < 20.0,
        "the burning player loses health, got {health}"
    );
}

/// DMG-ENV-002: lava ignites a player and deals its own damage.
#[test]
fn dmg_env_002_a_player_in_lava_burns() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "lava");
    survival(&mut server, anchor);
    place(&mut server, &[(8, 64, 8), (8, 65, 8)], "minecraft:lava");
    server.ticks(anchor, 25);
    let health = health_of(&mut server, anchor);
    assert!(health < 20.0, "the lava player loses health, got {health}");
}

/// DMG-ENV-003: powder snow freezes a player after the warm-up.
#[test]
fn dmg_env_003_a_player_in_powder_snow_freezes() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "snow");
    survival(&mut server, anchor);
    place(
        &mut server,
        &[(8, 64, 8), (8, 65, 8)],
        "minecraft:powder_snow",
    );
    server.ticks(anchor, 190);
    let health = health_of(&mut server, anchor);
    assert!(
        health < 20.0,
        "the frozen player loses health, got {health}"
    );
}

/// DMG-ENV-004: a player with its eyes in a block suffocates.
#[test]
fn dmg_env_004_a_player_with_its_eyes_in_a_block_suffocates() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "wall");
    survival(&mut server, anchor);
    place(&mut server, &[(8, 65, 8)], "minecraft:stone");
    server.ticks(anchor, 35);
    let health = health_of(&mut server, anchor);
    assert!(
        health < 20.0,
        "the suffocating player loses health, got {health}"
    );
}

/// DMG-ENV-004: a player against a cactus loses health.
#[test]
fn dmg_env_004_a_player_against_a_cactus_is_hurt() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "cactus");
    survival(&mut server, anchor);
    place(&mut server, &[(9, 64, 8)], "minecraft:cactus");
    let player = server.player(anchor);
    server
        .world()
        .get_mut::<mcrs_minecraft_engine_world::entity::physics::Transform>(player)
        .expect("the player has a transform")
        .translation
        .x = 8.75;
    server.ticks(anchor, 35);
    let health = health_of(&mut server, anchor);
    assert!(health < 20.0, "the cactus hurts the player, got {health}");
}

/// DMG-ENV-005: a player below the void floor takes damage regardless of the game mode.
#[test]
fn dmg_env_005_a_player_below_the_world_loses_health() {
    let mut server = start();
    load_stone_column(&mut server);
    let (anchor, _) = server.join(PLAYER, "void");
    let player = server.player(anchor);
    let min_y = {
        let world = server.world();
        world
            .query::<&mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig>()
            .single(world)
            .expect("one dimension")
            .min_y
    };
    server
        .world()
        .get_mut::<mcrs_minecraft_engine_world::entity::physics::Transform>(player)
        .expect("the player has a transform")
        .translation
        .y = f64::from(min_y) - 65.0;
    server.ticks(anchor, 3);
    let health = health_of(&mut server, anchor);
    assert!(health < 20.0, "the void player loses health, got {health}");
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
        .get_mut::<mcrs_minecraft_combat::ActiveEffects>(player)
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
                if effect.effect.0 == mcrs_minecraft_combat::MobEffect::Speed.id() as i32
                    && (1..=100).contains(&effect.duration.0)
        )),
        "the effect reaches the client with its duration"
    );
    server
        .world()
        .get_mut::<mcrs_minecraft_combat::ActiveEffects>(player)
        .expect("the player has effects")
        .remove(mcrs_minecraft_combat::MobEffect::Speed);
    let removed = server.ticks_all(1);
    assert!(
        removed.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::RemoveMobEffect(effect)
                if effect.effect.0 == mcrs_minecraft_combat::MobEffect::Speed.id() as i32
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
        .add_experience(10);
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
