use crate::mock_connection;
use bevy_app::{App, Update};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bytes::BytesMut;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::VANILLA_PACK;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, Instant, ServerSideConnection};
use mcrs_minecraft_protocol::decode::PacketDecoder;
use mcrs_minecraft_protocol::packets::configuration::ClientboundRegistryData;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundSelectKnownPacks;
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::{Decode, Encode, Packet};
use mcrs_minecraft_registry::RegistryLookup;
use mcrs_minecraft_server::configuration::{
    KnownPackOffer, known_pack_offer, on_known_packs_response, registry_data, start_configuration,
};
use mcrs_minecraft_world::registries::test_registries;

#[test]
fn the_registry_packets_carry_the_synced_registries_in_declared_order() {
    let set = test_registries();
    let packets = registry_data(set, &[], &[]);

    let sent: Vec<&str> = packets
        .iter()
        .map(|packet| packet.registry.as_str())
        .collect();
    let synced: Vec<&str> = set
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .collect();
    assert_eq!(sent.len(), 32);
    assert_eq!(sent, synced);
    assert!(!sent.contains(&"minecraft:environment_attribute"));

    for packet in &packets {
        let table = set.table(packet.registry.as_str()).unwrap();
        let names: Vec<&str> = packet
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let in_id_order: Vec<&str> = table.names().iter().map(|name| name.as_str()).collect();
        assert_eq!(names, in_id_order, "entries of {}", packet.registry);
        assert!(
            packet.entries.iter().all(|entry| entry.data.is_some()),
            "an entry of {} is sent without data",
            packet.registry
        );
    }

    let mut bytes = 0;
    for packet in &packets {
        let mut buffer = Vec::new();
        packet.encode(&mut buffer).unwrap();
        bytes += buffer.len();
    }
    println!(
        "the {} registry packets encode to {bytes} bytes",
        packets.len()
    );
}

#[test]
fn an_entry_of_a_known_pack_is_sent_without_its_data() {
    let set = test_registries();
    let offered = known_pack_offer(true);
    let packets = registry_data(set, &offered, &offered);

    let mut from_other_packs = 0;
    for packet in &packets {
        let registry = packet.registry.as_str();
        for (id, entry) in packet.entries.iter().enumerate() {
            let vanilla = set.pack_of(registry, id) == Some(VANILLA_PACK);
            assert_eq!(
                entry.data.is_none(),
                vanilla,
                "{registry}/{}: data is sent exactly when the entry is not from the vanilla pack",
                entry.id
            );
            from_other_packs += usize::from(!vanilla);
        }
    }
    assert!(
        from_other_packs > 0,
        "no entry comes from another pack, so the test cannot tell the skip from sending nothing"
    );
}

#[test]
fn every_entry_carries_data_unless_the_client_accepts_exactly_the_offer() {
    let set = test_registries();
    let core = |version| KnownPack {
        namespace: "minecraft",
        id: "core",
        version,
    };
    let other = KnownPack {
        namespace: "example",
        id: "extra",
        version: "1",
    };
    let cases: [(bool, Vec<KnownPack>); 4] = [
        (false, vec![core(VERSION.id.as_str())]),
        (true, vec![core("0")]),
        (true, Vec::new()),
        (true, vec![core(VERSION.id.as_str()), other]),
    ];
    for (offer, accepted) in cases {
        let packets = registry_data(set, &known_pack_offer(offer), &accepted);
        assert!(
            packets
                .iter()
                .flat_map(|packet| &packet.entries)
                .all(|entry| entry.data.is_some()),
            "offer {offer}, accepted {accepted:?}: an entry is sent without data"
        );
    }
}

#[test]
fn the_bridge_numbers_world_entries_as_the_registry_packets_do() {
    let set = test_registries();
    let packets = registry_data(set, &[], &[]);
    assert!(!packets.is_empty());

    for packet in &packets {
        let path = packet.registry.path();
        for (position, entry) in packet.entries.iter().enumerate() {
            let name = ResourceLocation::read(entry.id.as_str()).unwrap();
            let number = u16::try_from(position).unwrap();
            assert_eq!(
                RegistryLookup::id(set, path, &name),
                Some(number),
                "{path}/{name}"
            );
            assert_eq!(
                RegistryLookup::name(set, path, number),
                Some(&name),
                "{path}/{number}"
            );
        }
    }
}

#[test]
fn the_known_pack_offer_follows_the_plugin() {
    let offered = known_pack_offer(true);
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].namespace, "minecraft");
    assert_eq!(offered[0].id, "core");
    assert_eq!(offered[0].version, VERSION.id);
    assert!(known_pack_offer(false).is_empty());
}

#[test]
fn a_known_pack_reply_is_compared_with_the_offer_that_connection_received() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .insert_resource(test_registries().clone())
        .insert_resource(KnownPackOffer(true))
        .add_systems(Update, start_configuration())
        .add_observer(on_known_packs_response);
    let (raw, mut outgoing) = mock_connection::make_mock_raw_connection();
    let connection = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Configuration,
        ))
        .id();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.update();

    app.world_mut().insert_resource(KnownPackOffer(false));
    let mut reply = Vec::new();
    ServerboundSelectKnownPacks {
        known_packs: known_pack_offer(true),
    }
    .encode(&mut reply)
    .unwrap();
    app.world_mut().trigger(ReceivedPacketEvent {
        entity: connection,
        id: ServerboundSelectKnownPacks::ID,
        data: reply.into(),
        timestamp: Instant::now(),
    });
    app.world_mut().flush();

    app.world_mut()
        .get_mut::<ServerSideConnection>(connection)
        .unwrap()
        .raw
        .flush()
        .unwrap();
    let mut decoder = PacketDecoder::new();
    while let Ok(blob) = outgoing.try_recv() {
        decoder.queue_bytes(BytesMut::from(&blob[..]));
    }
    let without_data: usize = std::iter::from_fn(|| decoder.try_next_packet().unwrap())
        .filter(|frame| frame.id == ClientboundRegistryData::ID)
        .map(|frame| {
            let packet = ClientboundRegistryData::decode(&mut &frame.body[..]).unwrap();
            packet
                .entries
                .iter()
                .filter(|entry| entry.data.is_none())
                .count()
        })
        .sum();
    assert!(
        without_data > 0,
        "the reply matched the offer the connection received, so vanilla entries go without data"
    );
}
