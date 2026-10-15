use crate::mock_connection;
use bevy_app::{App, Update};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bytes::BytesMut;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::VANILLA_PACK;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, Instant, ServerSideConnection};
use mcrs_minecraft_protocol::decode::PacketDecoder;
use mcrs_minecraft_protocol::packets::configuration::ClientboundRegistryData;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundSelectKnownPacks;
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::{Decode, Encode, Packet};
use mcrs_minecraft_registry::RegistryLookup;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_server::MinecraftServerPlugin;
use mcrs_minecraft_server::configuration::{
    KnownPackOffer, known_pack_offer, offered_pack, on_known_packs_response, registry_data,
    start_configuration,
};
use mcrs_minecraft_world::registry_load::{KnownPacks, test_known_packs, test_registries};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn served(root: &Path) -> (RegistrySet, KnownPacks) {
    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugin {
        asset_path: Some(root.to_str().unwrap().to_owned()),
        announce_on_lan: false,
        ..MinecraftServerPlugin::embedded()
    });
    app.finish();
    app.cleanup();
    let world = app.world();
    (
        world.resource::<RegistrySet>().clone(),
        world.resource::<KnownPacks>().clone(),
    )
}

#[cfg(unix)]
struct RemovedOnDrop(PathBuf);

#[cfg(unix)]
impl Drop for RemovedOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn assets_with_pack(pack: &str, files: &[(&str, &str)]) -> RemovedOnDrop {
    let assets = Path::new(ASSETS).canonicalize().unwrap();
    let root =
        RemovedOnDrop(std::env::temp_dir().join(format!("mcrs_{pack}_{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&root.0);
    let packs = root.0.join("mcrs/datapacks");
    std::fs::create_dir_all(&packs).unwrap();
    std::os::unix::fs::symlink(assets.join("minecraft"), root.0.join("minecraft")).unwrap();
    for entry in std::fs::read_dir(assets.join("mcrs")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "datapacks" {
            std::os::unix::fs::symlink(entry.path(), root.0.join("mcrs").join(entry.file_name()))
                .unwrap();
        }
    }
    for entry in std::fs::read_dir(assets.join("mcrs/datapacks")).unwrap() {
        let entry = entry.unwrap();
        std::os::unix::fs::symlink(entry.path(), packs.join(entry.file_name())).unwrap();
    }
    let metadata = std::fs::read(assets.join("mcrs/datapacks/beta/pack.mcmeta")).unwrap();
    let files = files.iter().map(|(path, text)| (*path, text.as_bytes()));
    for (path, bytes) in std::iter::once(("pack.mcmeta", &metadata[..])).chain(files) {
        let file = packs.join(pack).join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
    root
}

fn sent_entry<'a>(
    packets: &'a [ClientboundRegistryData<'_>],
    registry: &str,
    entry: &str,
) -> &'a mcrs_minecraft_protocol::registry::Entry<'a> {
    packets
        .iter()
        .find(|packet| packet.registry.as_str() == registry)
        .and_then(|packet| packet.entries.iter().find(|sent| sent.id.as_str() == entry))
        .unwrap_or_else(|| panic!("{registry}/{entry} is not sent"))
}

#[cfg(unix)]
#[test]
fn a_pack_replacing_a_shipped_damage_type_is_what_the_server_uses_and_sends() {
    let root = assets_with_pack(
        "zz_override",
        &[(
            "minecraft/damage_type/arrow.json",
            r#"{"exhaustion":0.75,"message_id":"arrow","scaling":"when_caused_by_living_non_player"}"#,
        )],
    );
    let (set, known) = served(&root.0);

    let registry = "minecraft:damage_type";
    let table = set.table(registry).unwrap();
    let arrow = usize::from(table.number("minecraft:arrow").unwrap());
    assert_eq!(
        set.column::<DamageType>(registry).unwrap()[arrow].exhaustion,
        0.75
    );
    assert_eq!(set.pack_of(registry, arrow), Some("file/zz_override"));

    let offered = known_pack_offer(true, &known);
    let accepted: Vec<KnownPack> = offered.iter().map(offered_pack).collect();
    let packets = registry_data(&set, &offered, &accepted);
    let sent = sent_entry(&packets, registry, "minecraft:arrow")
        .data
        .as_deref()
        .expect("a client that knows the vanilla pack still receives the replaced entry");
    assert_eq!(
        sent.extract_compound()
            .and_then(|arrow| arrow.get("exhaustion")),
        Some(&NbtTag::Float(0.75))
    );
    assert!(
        sent_entry(&packets, registry, "minecraft:cactus")
            .data
            .is_none(),
        "an entry the pack leaves alone still comes from the vanilla pack"
    );
}

pub(crate) struct ShippedPack {
    pub(crate) id: String,
    pub(crate) namespaces: Vec<PathBuf>,
}

pub(crate) fn shipped_packs() -> Vec<ShippedPack> {
    let assets = Path::new(ASSETS);
    let mut packs = vec![ShippedPack {
        id: VANILLA_PACK.to_owned(),
        namespaces: vec![assets.join("minecraft")],
    }];
    let mut local: Vec<_> = std::fs::read_dir(assets.join("mcrs/datapacks"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    local.sort();
    for root in local {
        let mut namespaces: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        namespaces.sort();
        packs.push(ShippedPack {
            id: format!("file/{}", root.file_name().unwrap().to_str().unwrap()),
            namespaces,
        });
    }
    packs
}

pub(crate) fn json_files(directory: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let relative = path.strip_prefix(directory).unwrap();
                found.push(relative.to_str().unwrap().replace('\\', "/"));
            }
        }
    }
    found
}

fn shipped_entries(registry: &str) -> Vec<(String, String)> {
    let directory = registry.strip_prefix("minecraft:").unwrap();
    let mut entries = BTreeMap::new();
    if registry == "minecraft:worldgen/biome" {
        for name in mcrs_minecraft_worldgen_builtin::biome_names() {
            let key = (format!("{}.json", name.path()), name.namespace().to_owned());
            entries.insert(key, VANILLA_PACK.to_owned());
        }
    }
    for pack in shipped_packs() {
        for namespace in &pack.namespaces {
            for file in json_files(&namespace.join(directory)) {
                let namespace = namespace.file_name().unwrap().to_str().unwrap().to_owned();
                entries.insert((file, namespace), pack.id.clone());
            }
        }
    }
    entries
        .into_iter()
        .map(|((file, namespace), pack)| {
            let path = file.strip_suffix(".json").unwrap();
            (format!("{namespace}:{path}"), pack)
        })
        .collect()
}

#[test]
fn the_registry_packets_list_the_entries_the_packs_ship() {
    let set = test_registries();
    let core = known_pack_offer(true, test_known_packs());
    for offered in [Vec::new(), core] {
        let accepted: Vec<KnownPack> = offered.iter().map(offered_pack).collect();
        for packet in registry_data(set, &offered, &accepted) {
            let registry = packet.registry.as_str();
            let shipped = shipped_entries(registry);
            let sent: Vec<&str> = packet
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect();
            let expected: Vec<&str> = shipped.iter().map(|(name, _)| name.as_str()).collect();
            assert_eq!(sent, expected, "{registry} with offer {offered:?}");
            for (entry, (name, pack)) in packet.entries.iter().zip(&shipped) {
                assert_eq!(
                    entry.data.is_none(),
                    !accepted.is_empty() && pack == VANILLA_PACK,
                    "{registry}/{name} from {pack} with offer {offered:?}"
                );
            }
        }
    }
}

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

const OTHER_PACK: KnownPack<'static> = KnownPack {
    namespace: "example",
    id: "extra",
    version: "1",
};

#[test]
fn an_entry_of_an_accepted_pack_is_sent_without_its_data() {
    let set = test_registries();
    let offered = known_pack_offer(true, test_known_packs());
    let exactly: Vec<KnownPack> = offered.iter().map(offered_pack).collect();
    let with_another = exactly.iter().cloned().chain([OTHER_PACK]).collect();

    for accepted in [exactly, with_another] {
        let packets = registry_data(set, &offered, &accepted);
        let mut from_other_packs = 0;
        for packet in &packets {
            let registry = packet.registry.as_str();
            for (id, entry) in packet.entries.iter().enumerate() {
                let vanilla = set.pack_of(registry, id) == Some(VANILLA_PACK);
                assert_eq!(
                    entry.data.is_none(),
                    vanilla,
                    "{registry}/{}: data is sent exactly when the entry is not from the vanilla pack, accepted {accepted:?}",
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
}

#[test]
fn every_entry_carries_data_unless_the_client_accepts_an_offered_pack() {
    let set = test_registries();
    let core = |version| KnownPack {
        namespace: "minecraft",
        id: "core",
        version,
    };
    let cases: [(bool, Vec<KnownPack>); 4] = [
        (false, vec![core(VERSION.id.as_str())]),
        (true, vec![core("0")]),
        (true, Vec::new()),
        (true, vec![OTHER_PACK]),
    ];
    for (offer, accepted) in cases {
        let packets = registry_data(set, &known_pack_offer(offer, test_known_packs()), &accepted);
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
fn a_known_pack_reply_is_compared_with_the_offer_that_connection_received() {
    let offered = known_pack_offer(true, test_known_packs());
    let offered: Vec<KnownPack> = offered.iter().map(offered_pack).collect();
    assert_eq!(
        offered,
        [KnownPack {
            namespace: "minecraft",
            id: "core",
            version: VERSION.id.as_str(),
        }]
    );
    assert!(known_pack_offer(false, test_known_packs()).is_empty());

    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .insert_resource(test_registries().clone())
        .insert_resource(test_known_packs().clone())
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
    let offered = known_pack_offer(true, test_known_packs());
    ServerboundSelectKnownPacks {
        known_packs: offered.iter().map(offered_pack).collect(),
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
