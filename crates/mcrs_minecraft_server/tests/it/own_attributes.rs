use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBundleDelimiter;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol_game::serverbound::{PlayerCommandAction, ServerboundPlayerCommand};
use mcrs_minecraft_server::{
    VanillaCreativeRangePlugin, VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin,
};

use crate::host_app::{self, PlayServer, replicated, sent_to};

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
    assert_eq!(
        id.0,
        ClientboundUpdateAttributes::ID,
        "only attributes are sent"
    );
    Some(decode_update(rest))
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
                attribute: entry.attribute,
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

/// ATR-NET-003 and MVC-PKT-031: the own player gets the entry of `movement_speed` when its sprint
/// starts and when it stops, and nothing in a tick without a change.
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
            base: 0.10000000149011612,
            modifiers,
        }],
    };
    assert_eq!(
        started,
        [speed(vec![(
            "minecraft:sprinting".to_owned(),
            0.30000001192092896,
            ModifierOperation::AddMultipliedTotal,
        )])]
    );
    assert_eq!(stopped, [speed(Vec::new())]);
    assert_eq!(idle, []);
}

/// The `update_attributes` of the muxer in `packets`.
fn tracked_updates(packets: &[PacketPayload]) -> Vec<OwnUpdate> {
    replicated(packets)
        .into_iter()
        .filter(|(id, _)| *id == ClientboundUpdateAttributes::ID)
        .map(|(_, body)| decode_update(&body))
        .collect()
}

/// ATR-NET-003: a player that tracks a sprinter gets the entry of its `movement_speed` once, and
/// the sprinter gets none about itself from the muxer.
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
                base: 0.10000000149011612,
                modifiers: vec![(
                    "minecraft:sprinting".to_owned(),
                    0.30000001192092896,
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

/// TRK-DATA-004 and TRK-SPN-001: the spawn bundle of a creative player holds its two range
/// instances, right after its `add_entity`.
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
    let ids: Vec<i32> = packets[add + 1..add + 3]
        .iter()
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(
        ids,
        [
            ClientboundUpdateAttributes::ID,
            ClientboundBundleDelimiter::ID
        ],
        "the attributes close the bundle"
    );
    let mut update = decode_update(&packets[add + 1].1);
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

/// ATR-NET-005: a player that joins in creative mode gets its two range modifiers after the
/// `login`.
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
