use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_protocol::item::ctx::WireCtx;
use mcrs_minecraft_protocol::item::{DecodeCtx, EncodeCtx};
use mcrs_minecraft_protocol::{Encode, VarInt};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::{Ref, Resolved};
use mcrs_minecraft_sound::SoundEvent;
use mcrs_minecraft_world::registry_load::{sent_by, test_registries};

use crate::common::{declared_registries, loaded_names};

fn crate_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read(relative: &str) -> String {
    let path = crate_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn the_running_app_numbers_world_registries_as_the_loader_does(app: &App) {
    let set = app.world().resource::<CurrentRegistries>();
    let world_registries = declared_registries(false);

    let mut snapshots = 0;
    for (registry, entries) in sent_by(set, false)
        .0
        .into_iter()
        .filter(|(registry, _)| world_registries.contains(registry.as_str()))
    {
        let registry = registry.as_str();
        assert_eq!(
            entries
                .iter()
                .map(|(name, _)| name.to_string())
                .collect::<Vec<_>>(),
            loaded_names(set, registry),
            "{registry}: the synced registry numbers the entries differently from the loader"
        );
        snapshots += 1;
    }
    assert!(snapshots > 0, "no synced world registry was compared");
}

fn recorded_by_registry(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut registries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.split(' ');
        let registry = parts.next().expect("a line names its registry");
        let names = registries.entry(registry.to_string()).or_default();
        if let (Some(_id), Some(name)) = (parts.next(), parts.next()) {
            names.push(name.to_string());
        }
    }
    registries
}

#[test]
fn the_loader_keeps_every_recorded_world_registry_id() {
    let set = test_registries();
    let changed: BTreeSet<String> =
        std::fs::read_to_string(crate_path("tests/fixtures/world_registry_id_changes.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();

    for (registry, recorded) in recorded_by_registry(&read("tests/fixtures/world_registry_ids.txt"))
    {
        let loaded = loaded_names(set, &registry);
        if changed.contains(&registry) {
            assert_ne!(
                loaded, recorded,
                "{registry} is listed as renumbered but keeps its recorded order"
            );
            let loaded: BTreeSet<_> = loaded.into_iter().collect();
            let recorded: BTreeSet<_> = recorded.into_iter().collect();
            assert_eq!(loaded, recorded, "{registry}: the set of names changed");
        } else {
            assert_eq!(
                loaded, recorded,
                "{registry}: the loader numbers the entries differently from the recorded ids"
            );
        }
    }
}

#[test]
fn a_world_registry_without_files_is_empty() {
    let names = loaded_names(test_registries(), "minecraft:dimension");
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn a_wire_id_the_registries_lack_is_refused_by_registry_and_a_holder_id_round_trips() {
    let registries = test_registries();
    let ctx = WireCtx::new(registries);
    let past = |registry: &str, offset: usize| {
        let len = loaded_names(registries, registry).len();
        let mut wire = Vec::new();
        VarInt(i32::try_from(len + offset).unwrap())
            .encode(&mut wire)
            .unwrap();
        wire
    };

    let error =
        Id::<DimensionType>::decode_ctx(&ctx, &mut &past("minecraft:dimension_type", 0)[..])
            .unwrap_err();
    assert!(
        format!("{error:#}").contains("minecraft:dimension_type"),
        "{error:#}"
    );

    type Sound = Ref<Resolved, SoundEvent, SoundEvent>;
    let error = Sound::decode_ctx(&ctx, &mut &past("minecraft:sound_event", 1)[..]).unwrap_err();
    assert!(
        format!("{error:#}").contains("minecraft:sound_event"),
        "{error:#}"
    );

    let sound = Sound::Id(mcrs_minecraft_sound::keys::sound_event::ENTITY_ITEM_BREAK.id());
    let mut wire = Vec::new();
    sound.encode_ctx(&ctx, &mut wire).unwrap();
    let mut r = &wire[..];
    assert_eq!(Sound::decode_ctx(&ctx, &mut r).unwrap(), sound);
    assert!(r.is_empty());
}
