use std::path::Path;
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, PacketPayload};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::persisted_registry::{PersistedRegistry, PersistedSnapshot};
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeInstances, InstanceEdit, InstanceModifier, ModifierKind, ModifierOperation,
};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBundleDelimiter;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundSetEntityData, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol_game::serverbound::{PlayerCommandAction, ServerboundPlayerCommand};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_save::{PlayerDat, read_player_dat};
use mcrs_minecraft_server::{
    VanillaCreativeRangePlugin, VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin,
};

use crate::host_app::{self, Arrival, PlayServer, label_of, replicated, sent_to};
use crate::world_server::{
    Holder, OVERWORLD, WorldServer, cell_worlds, holder, lockstep_tick, wait_for_generation, walk,
};

#[derive(Debug, PartialEq)]
struct Entry {
    attribute: Attribute,
    base: f64,
    modifiers: Vec<(String, f64, ModifierOperation)>,
}

#[derive(Debug, PartialEq)]
struct OwnUpdate {
    entity_id: i32,
    entries: Vec<Entry>,
}

fn own_update(packet: &PacketPayload) -> Option<OwnUpdate> {
    let PacketPayload::OwnEntityState(bytes) = packet else {
        return None;
    };
    let mut rest: &[u8] = bytes;
    let id = VarInt::decode(&mut rest).unwrap();
    (id.0 == ClientboundUpdateAttributes::ID).then(|| decode_update(rest))
}

fn decode_update(mut rest: &[u8]) -> OwnUpdate {
    let update = ClientboundUpdateAttributes::decode(&mut rest).unwrap();
    assert!(rest.is_empty(), "the packet has no trailing bytes");
    OwnUpdate {
        entity_id: update.entity_id.0,
        entries: update
            .attributes
            .iter()
            .map(|entry| Entry {
                attribute: entry
                    .attribute
                    .resolve()
                    .expect("the server sends known ids"),
                base: entry.base,
                modifiers: entry
                    .modifiers
                    .iter()
                    .map(|modifier| (modifier.id.to_string(), modifier.amount, modifier.operation))
                    .collect(),
            })
            .collect(),
    }
}

fn own_updates(packets: &[PacketPayload]) -> Vec<OwnUpdate> {
    packets.iter().filter_map(own_update).collect()
}

#[test]
fn atr_net_003_the_own_player_gets_each_change_of_its_sprint_modifier() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    let (anchor, _) = server.join(Uuid::from_u128(1), "Sprinter");
    let entity_id = player_network_id(server.session_of(anchor).unwrap());
    let sprint = |server: &mut PlayServer, action| {
        own_updates(&server.send(
            anchor,
            &ServerboundPlayerCommand {
                entity_id: VarInt(entity_id),
                action,
                data: VarInt(0),
            },
        ))
    };

    let started = sprint(&mut server, PlayerCommandAction::StartSprinting);
    let stopped = sprint(&mut server, PlayerCommandAction::StopSprinting);
    let idle = own_updates(&server.ticks(anchor, 2));

    let speed = |modifiers| OwnUpdate {
        entity_id,
        entries: vec![Entry {
            attribute: Attribute::MovementSpeed,
            base: f64::from(0.1_f32),
            modifiers,
        }],
    };
    assert_eq!(
        started,
        [speed(vec![(
            "minecraft:sprinting".to_owned(),
            f64::from(0.3_f32),
            ModifierOperation::AddMultipliedTotal,
        )])]
    );
    assert_eq!(stopped, [speed(Vec::new())]);
    assert_eq!(idle, []);
}

#[test]
fn a_sprint_stop_without_a_sprint_sends_nothing_and_changes_nothing() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    let (anchor, _) = server.join(Uuid::from_u128(1), "Walker");
    let player = server.player(anchor);
    server.ticks(anchor, 2);
    let last_changed = |server: &mut PlayServer| {
        server
            .world()
            .entity(player)
            .get_ref::<AttributeInstances>()
            .unwrap()
            .last_changed()
    };
    let before = last_changed(&mut server);

    server
        .world()
        .get_mut::<PlayerSprinting>(player)
        .unwrap()
        .set_changed();
    let packets = server.ticks(anchor, 2);

    assert_eq!(own_updates(&packets), []);
    assert_eq!(last_changed(&mut server), before);
}

#[test]
fn two_instances_changed_in_one_tick_go_out_in_one_packet_in_id_order() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaCreativeRangePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    server
        .world()
        .insert_resource(DefaultGameMode(GameMode::Survival));
    let (anchor, _) = server.join(Uuid::from_u128(2), "Builder");
    let player = server.player(anchor);
    server.ticks(anchor, 2);

    server
        .world()
        .get_mut::<PlayerGameMode>(player)
        .unwrap()
        .set_if_neq(PlayerGameMode(GameMode::Creative));
    let updates = own_updates(&server.ticks(anchor, 2));

    assert_eq!(
        updates,
        [OwnUpdate {
            entity_id: player_network_id(server.session_of(anchor).unwrap()),
            entries: creative_ranges().into(),
        }]
    );
}

fn tracked_updates(packets: &[PacketPayload]) -> Vec<OwnUpdate> {
    replicated(packets)
        .into_iter()
        .filter(|(id, _)| *id == ClientboundUpdateAttributes::ID)
        .map(|(_, body)| decode_update(&body))
        .collect()
}

#[test]
fn atr_net_003_a_tracking_player_gets_the_sprint_modifier_of_the_player_it_tracks() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    let sprinter = server.admit(server.dim, Uuid::from_u128(1), "Sprinter");
    let watcher = server.admit(server.dim, Uuid::from_u128(2), "Watcher");
    server.ticks_all(3);
    let sprinter_session = server.session_of(sprinter).unwrap();
    let watcher_session = server.session_of(watcher).unwrap();
    let entity_id = player_network_id(sprinter_session);

    server.queue(
        sprinter,
        &ServerboundPlayerCommand {
            entity_id: VarInt(entity_id),
            action: PlayerCommandAction::StartSprinting,
            data: VarInt(0),
        },
    );
    let started = server.ticks_all(2);
    let idle = server.ticks(watcher, 2);

    assert_eq!(
        tracked_updates(&sent_to(watcher_session, started.clone())),
        [OwnUpdate {
            entity_id,
            entries: vec![Entry {
                attribute: Attribute::MovementSpeed,
                base: f64::from(0.1_f32),
                modifiers: vec![(
                    "minecraft:sprinting".to_owned(),
                    f64::from(0.3_f32),
                    ModifierOperation::AddMultipliedTotal,
                )],
            }],
        }]
    );
    assert_eq!(
        tracked_updates(&sent_to(sprinter_session, started)),
        [],
        "the sprinter gets its own entries only as its own entity state"
    );
    assert_eq!(
        tracked_updates(&idle),
        [],
        "an unchanged instance sends nothing"
    );
}

#[test]
fn trk_data_004_the_spawn_bundle_of_a_creative_player_holds_its_range_modifiers() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaCreativeRangePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    server
        .world()
        .insert_resource(DefaultGameMode(GameMode::Creative));
    let builder = server.admit(server.dim, Uuid::from_u128(3), "Builder");
    server.ticks_all(3);
    let watcher = server.admit(server.dim, Uuid::from_u128(4), "Watcher");
    let packets = replicated(&server.ticks(watcher, 3));
    let entity_id = player_network_id(server.session_of(builder).unwrap());

    let add = packets
        .iter()
        .position(|(id, body)| {
            *id == ClientboundAddEntity::ID
                && ClientboundAddEntity::decode(&mut body.as_slice())
                    .unwrap()
                    .id
                    .0
                    == entity_id
        })
        .expect("the watcher gets the spawn of the builder");
    let ids: Vec<i32> = packets[add + 1..add + 4]
        .iter()
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(
        ids,
        [
            ClientboundSetEntityData::ID,
            ClientboundUpdateAttributes::ID,
            ClientboundBundleDelimiter::ID
        ],
        "the attributes follow the entity data and close the bundle"
    );
    let mut update = decode_update(&packets[add + 2].1);
    update.entries.sort_by_key(|entry| entry.attribute);
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.entries, creative_ranges());
}

fn creative_ranges() -> [Entry; 2] {
    let range = |attribute, base, id: &str, amount| Entry {
        attribute,
        base,
        modifiers: vec![(id.to_owned(), amount, ModifierOperation::AddValue)],
    };
    let mut expected = [
        range(
            Attribute::BlockInteractionRange,
            4.5,
            "minecraft:creative_mode_block_range",
            0.5,
        ),
        range(
            Attribute::EntityInteractionRange,
            3.0,
            "minecraft:creative_mode_entity_range",
            2.0,
        ),
    ];
    expected.sort_by_key(|entry| entry.attribute);
    expected
}

#[test]
fn atr_net_005_a_creative_player_gets_its_range_modifiers_after_the_login() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaCreativeRangePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    server
        .world()
        .insert_resource(DefaultGameMode(GameMode::Creative));
    let anchor = server.admit(server.dim, Uuid::from_u128(2), "Builder");
    let packets = server.ticks(anchor, 4);

    let login = packets
        .iter()
        .position(|packet| matches!(packet, PacketPayload::PlayerLogin(_)))
        .expect("a login is sent");
    let update = packets
        .iter()
        .position(|packet| own_update(packet).is_some())
        .expect("an update of the attributes is sent");
    assert!(login < update, "the update follows the login: {packets:?}");
    let mut updates = own_updates(&packets);
    assert_eq!(updates.len(), 1, "{updates:?}");
    let entity_id = player_network_id(server.session_of(anchor).unwrap());
    assert_eq!(updates[0].entity_id, entity_id);
    updates[0].entries.sort_by_key(|entry| entry.attribute);
    assert_eq!(updates[0].entries, creative_ranges());
}

fn held(app: &mut App, label: Entity, session: PlayerSession) -> (&mut World, Entity) {
    let Holder { cell, player } = holder(app, label, session).expect("the player is in a cell");
    (cell_worlds(app, label).swap_remove(cell), player)
}

fn set_sprinting(app: &mut App, label: Entity, session: PlayerSession, sprinting: bool) {
    let (world, player) = held(app, label, session);
    world
        .get_mut::<PlayerSprinting>(player)
        .unwrap()
        .set_if_neq(PlayerSprinting(sprinting));
}

#[test]
fn a_sprinter_that_crosses_a_cell_border_keeps_its_sprint_modifier_until_it_stops() {
    let mut app = WorldServer {
        cells: 2,
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let mut join = |name, x| {
        let arrival = Arrival {
            uuid: Uuid::from_u128(u128::from(name == "Watcher") + 1),
            name,
            position: DVec3::new(x, 100.0, 8.5),
            view_distance: 2,
        };
        let anchor = host_app::admit(&mut app, label, &arrival);
        host_app::session_of(&app, anchor).unwrap()
    };
    let sprinter = join("Sprinter", -8.5);
    let watcher = join("Watcher", 8.5);
    for _ in 0..30 {
        lockstep_tick(&mut app, label);
    }
    set_sprinting(&mut app, label, sprinter, true);
    lockstep_tick(&mut app, label);
    let west = holder(&mut app, label, sprinter).unwrap().cell;

    walk(&mut app, label, sprinter, DVec3::new(16.0, 0.0, 0.0));
    let mut crossed = Vec::new();
    for _ in 0..10 {
        crossed.extend(lockstep_tick(&mut app, label));
    }
    assert_ne!(holder(&mut app, label, sprinter).unwrap().cell, west);
    assert_eq!(
        own_updates(&sent_to(sprinter, crossed)),
        [],
        "a cell crossing tells the client nothing"
    );
    let (world, player) = held(&mut app, label, sprinter);
    let speed = world
        .get::<AttributeInstances>(player)
        .unwrap()
        .value(Attribute::MovementSpeed)
        .unwrap();
    assert_eq!(
        speed.to_bits(),
        (f64::from(0.1_f32) * (1.0 + f64::from(0.3_f32))).to_bits()
    );

    set_sprinting(&mut app, label, sprinter, false);
    let mut stopped = Vec::new();
    for _ in 0..2 {
        stopped.extend(lockstep_tick(&mut app, label));
    }

    let entity_id = player_network_id(sprinter);
    let updates: Vec<OwnUpdate> = tracked_updates(&sent_to(watcher, stopped))
        .into_iter()
        .filter(|update| update.entity_id == entity_id)
        .collect();
    assert_eq!(
        updates,
        [OwnUpdate {
            entity_id,
            entries: vec![Entry {
                attribute: Attribute::MovementSpeed,
                base: f64::from(0.1_f32),
                modifiers: Vec::new(),
            }],
        }]
    );
}

#[test]
fn a_modifier_of_a_player_holds_after_a_change_of_dimension() {
    let mut app = host_app::make_host_app();
    app.add_plugins(VanillaEntityAttributesPlugin);
    let mut server = PlayServer::start_in(
        app,
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        &[],
        0,
    );
    let (anchor, _) = server.join(Uuid::from_u128(5), "Walker");
    let overworld = server.dim;
    let end = label_of(&server.app, "minecraft:the_end");
    let player = server.player(anchor);
    let boost = || {
        InstanceModifier::new(
            ResourceLocationBuf::read("mcrs:boost").unwrap(),
            0.25,
            ModifierOperation::AddMultipliedBase,
            ModifierKind::Permanent,
        )
    };
    let _ = server
        .world()
        .get_mut::<AttributeInstances>(player)
        .unwrap()
        .apply(Attribute::MovementSpeed, &InstanceEdit::Put(boost()));
    server.ticks(anchor, 1);
    server
        .world_in(overworld)
        .resource_mut::<Messages<ChangeDimensionRequest>>()
        .write(ChangeDimensionRequest {
            player,
            to: ResourceKeyBuf::from_location(
                ResourceLocationBuf::read("minecraft:the_end").unwrap(),
            ),
            transform: Transform::from_translation(DVec3::new(100.5, 49.0, 0.5)),
        });
    let packets = server.ticks(anchor, 3);

    let arrived = server.player_in(end, anchor).expect("the player arrived");
    let instance = server
        .world_in(end)
        .get::<AttributeInstances>(arrived)
        .unwrap()
        .set()
        .instance(Attribute::MovementSpeed)
        .cloned()
        .expect("the instance holds its modifier");
    assert_eq!(instance.modifiers(), [boost()]);
    let respawn = packets
        .iter()
        .position(|packet| matches!(packet, PacketPayload::Respawn(_)))
        .expect("a respawn is sent");
    let update = packets
        .iter()
        .position(|packet| own_update(packet).is_some())
        .expect("an update of the attributes is sent");
    assert!(
        respawn < update,
        "the update follows the respawn: {packets:?}"
    );
    assert_eq!(
        own_updates(&packets[update..]),
        [OwnUpdate {
            entity_id: player_network_id(server.session_of(anchor).unwrap()),
            entries: vec![Entry {
                attribute: Attribute::MovementSpeed,
                base: f64::from(0.1_f32),
                modifiers: vec![(
                    "mcrs:boost".to_owned(),
                    0.25,
                    ModifierOperation::AddMultipliedBase,
                )],
            }],
        }]
    );
}

fn leave_and_read_file(server: &mut PlayServer, anchor: Entity, uuid: Uuid) -> PlayerDat {
    let save_dir = std::env::temp_dir().join(format!("mcrs-attributes-leave-{uuid}"));
    std::fs::create_dir_all(&save_dir).unwrap();
    server.world().insert_resource(WorldSave(save_dir.clone()));
    let session = server.session_of(anchor).unwrap();
    let dim = server.dim;
    server.submit_to(dim, ToDim::Despawn(InboundPlayerDespawn { session }));
    let deadline = Instant::now() + Duration::from_secs(30);
    let dat = loop {
        server.ticks_all(1);
        if let Some(dat) = read_player_dat(&save_dir, uuid).unwrap() {
            break dat;
        }
        assert!(
            Instant::now() < deadline,
            "the leave never wrote the player file"
        );
    };
    let _ = std::fs::remove_dir_all(save_dir);
    dat
}

#[test]
fn a_new_player_that_leaves_has_an_empty_attribute_list_in_its_file() {
    let mut app = host_app::make_host_app();
    app.add_plugins(VanillaEntityAttributesPlugin);
    let mut server = PlayServer::start_in(app, &[(OVERWORLD, OVERWORLD)], &[], 0);
    let uuid = Uuid::new_v4();
    let (anchor, _) = server.join(uuid, "Fresh");
    server.ticks(anchor, 1);

    let dat = leave_and_read_file(&mut server, anchor, uuid);
    let attributes = dat.get("attributes").and_then(NbtTag::extract_list);
    assert_eq!(attributes.map(<[NbtTag]>::len), Some(0));
}

#[test]
fn a_player_that_leaves_and_joins_again_keeps_its_base_and_permanent_modifier() {
    let mut app = host_app::make_host_app();
    app.add_plugins(VanillaEntityAttributesPlugin);
    let mut server = PlayServer::start_in(app, &[(OVERWORLD, OVERWORLD)], &[], 0);
    let uuid = Uuid::new_v4();
    let (anchor, _) = server.join(uuid, "Keeper");
    let player = server.player(anchor);
    let lucky = || {
        InstanceModifier::new(
            ResourceLocationBuf::read("test:a").unwrap(),
            1.0,
            ModifierOperation::AddValue,
            ModifierKind::Permanent,
        )
    };
    let edited = {
        let mut instances = server
            .world()
            .get_mut::<AttributeInstances>(player)
            .unwrap();
        let _ = instances.apply(Attribute::MaxHealth, &InstanceEdit::SetBase(30.0));
        let _ = instances.apply(Attribute::Luck, &InstanceEdit::Put(lucky()));
        instances.clone()
    };
    server.ticks(anchor, 1);

    let dat = leave_and_read_file(&mut server, anchor, uuid);
    let registry = server.app.world().resource::<PersistedRegistry>();
    let file = PersistedSnapshot::from_dat(&dat, registry, Path::new("player.dat")).unwrap();
    let arrival = Arrival {
        uuid,
        name: "Keeper",
        position: DVec3::new(8.5, 64.0, 8.5),
        view_distance: 2,
    };
    let anchor = host_app::admit_with_file(&mut server.app, server.dim, &arrival, &file);
    server.ticks(anchor, 3);
    let player = server.player(anchor);
    assert_eq!(
        server.world().get::<AttributeInstances>(player),
        Some(&edited)
    );
}
