use mcrs_minecraft_resource::ResourceLocationBuf;
use std::collections::BTreeMap;

use mcrs_minecraft_datapack::PackSource;
use mcrs_minecraft_game_registries::registry_load::{
    load_received, sent_by, test_packs, test_registries,
};
use mcrs_minecraft_registries::{NetworkSnapshot, Registries};

const REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

type Contents = BTreeMap<String, (Vec<String>, Vec<(String, Vec<u16>)>)>;

fn contents(set: &Registries, registries: &[String]) -> Contents {
    registries
        .iter()
        .map(|registry| {
            let key = ResourceLocationBuf::read(registry).unwrap();
            let names = set
                .names_of(&key)
                .map(|names| names.iter().map(ToString::to_string).collect())
                .unwrap_or_default();
            let tags = set
                .tags_of(key.as_location())
                .map(|tags| {
                    tags.map(|(name, members)| (name.to_string(), members.to_vec()))
                        .collect()
                })
                .unwrap_or_default();
            (registry.clone(), (names, tags))
        })
        .collect()
}

#[test]
fn a_client_holds_the_synced_registries_and_tags_the_server_holds() {
    let packs = test_packs();
    for (known, local) in [(true, Some(&packs as &dyn PackSource)), (false, None)] {
        let received = sent_by(test_registries(), known);
        let sent: Vec<String> = received
            .registries
            .iter()
            .map(|(key, _)| key.to_string())
            .chain(received.tags.iter().map(|(key, _)| key.to_string()))
            .collect();
        let (client, _) = load_received(REPORT, &received, local)
            .unwrap_or_else(|error| panic!("known {known}: {error}"));
        assert_eq!(
            contents(&client, &sent),
            contents(test_registries(), &sent),
            "known packs negotiated: {known}"
        );
    }
}

/// The client reads the vanilla entries of `left_out` from its own vanilla files and every other
/// entry from the data sent; it then holds what it holds when the server sends every entry with
/// its data.
fn files_stand_for_the_data_sent(left_out: impl Fn(&ResourceLocationBuf) -> bool) {
    let with_data = sent_by(test_registries(), false);
    let mut without = sent_by(test_registries(), true);
    for ((registry, entries), (_, full)) in without.registries.iter_mut().zip(&with_data.registries)
    {
        if !left_out(registry) {
            entries.clone_from(full);
        }
    }
    let resent = |client: &Registries| -> Vec<Vec<u8>> {
        let snapshot = NetworkSnapshot::new(client).unwrap();
        snapshot.registry_data(false).map(<[u8]>::to_vec).collect()
    };
    let packs = test_packs();
    let (from_files, _) = load_received(REPORT, &without, Some(&packs)).unwrap();
    let (from_data, _) = load_received(REPORT, &with_data, None).unwrap();
    let pairs = resent(&from_files).into_iter().zip(resent(&from_data));
    for ((registry, _), (from_files, from_data)) in with_data.registries.iter().zip(pairs) {
        assert!(from_files == from_data, "{registry}");
    }
}

#[test]
fn vanilla_entries_of_a_reduced_network_form_read_from_files_equal_the_data_sent() {
    files_stand_for_the_data_sent(|registry| {
        let reduced = [
            "minecraft:worldgen/biome",
            "minecraft:dimension_type",
            "minecraft:timeline",
            "minecraft:wolf_variant",
        ];
        reduced.contains(&registry.as_str())
    });
}

mod exhaustive {
    #[test]
    fn every_vanilla_entry_read_from_files_equals_the_data_sent() {
        super::files_stand_for_the_data_sent(|_| true);
    }
}
