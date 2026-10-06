use std::path::Path;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
use mcrs_minecraft_nbt::tag::NbtTag;

use super::*;
use crate::dimension::Dimensions;
use crate::registries::test_registries;
use crate::worldgen::world_preset::WorldPreset;

const OBSERVED_UUID_INTS: [i32; 4] = [-1495452199, -11581732, -1243709312, 65080886];
const OBSERVED_UUID_FILE_NAME: &str = "a6dd35d9-ff4f-46dc-b5de-808003e10e36";
const OBSERVED_YAW: f32 = -139.949_3;
const OBSERVED_PITCH: f32 = 8.099_984;

fn path() -> &'static Path {
    Path::new("fixture.dat")
}

fn gzip(compound: NbtCompound) -> Vec<u8> {
    write_gzip_compound_tag_to_bytes(&compound).unwrap()
}

fn keys(compound: &NbtCompound) -> Vec<&str> {
    compound
        .child_tags
        .iter()
        .map(|(name, _)| name.as_str())
        .collect()
}

fn clock(id: &str) -> ResourceLocation<Arc<str>> {
    ResourceLocation::read(id).unwrap()
}

fn spawn_compound() -> NbtCompound {
    let mut spawn = NbtCompound::new();
    spawn.put("pos", NbtTag::IntArray(vec![0, 70, 64]));
    spawn.put_float("yaw", 0.0);
    spawn.put_float("pitch", 0.0);
    spawn.put_string("dimension", "minecraft:overworld".to_string());
    spawn
}

fn level_data_tagged(data_version: Option<NbtTag>, with_uuid: bool) -> NbtCompound {
    let mut data = NbtCompound::new();
    if let Some(tag) = data_version {
        data.put("DataVersion", tag);
    }
    data.put_string("LevelName", "New World".to_string());
    data.put_long("Time", 25);
    if with_uuid {
        data.put(
            "singleplayer_uuid",
            NbtTag::IntArray(OBSERVED_UUID_INTS.to_vec()),
        );
    }
    data.put_component("spawn", spawn_compound());
    data
}

fn level_dat(data_version: i32, with_uuid: bool) -> Vec<u8> {
    level_dat_tagged(Some(NbtTag::Int(data_version)), with_uuid)
}

fn level_dat_tagged(data_version: Option<NbtTag>, with_uuid: bool) -> Vec<u8> {
    let mut root = NbtCompound::new();
    root.put_component("Data", level_data_tagged(data_version, with_uuid));
    gzip(root)
}

fn level_dat_with_spawn(spawn: NbtCompound) -> Vec<u8> {
    let mut data = NbtCompound::new();
    data.put_int("DataVersion", VERSION.world_version);
    data.put_string("LevelName", "New World".to_string());
    data.put_long("Time", 25);
    data.put(
        "singleplayer_uuid",
        NbtTag::IntArray(OBSERVED_UUID_INTS.to_vec()),
    );
    data.put_component("spawn", spawn);
    let mut root = NbtCompound::new();
    root.put_component("Data", data);
    gzip(root)
}

fn saved_data(data_version: i32, payload: NbtCompound) -> Vec<u8> {
    saved_data_tagged(Some(NbtTag::Int(data_version)), payload)
}

fn saved_data_tagged(data_version: Option<NbtTag>, payload: NbtCompound) -> Vec<u8> {
    let mut root = NbtCompound::new();
    root.put_component("data", payload);
    if let Some(tag) = data_version {
        root.put("DataVersion", tag);
    }
    gzip(root)
}

fn world_clocks_payload() -> NbtCompound {
    let mut overworld = NbtCompound::new();
    overworld.put_long("total_ticks", 1757);
    overworld.put_float("partial_tick", 0.25);
    overworld.put_float("rate", 2.0);
    overworld.put_bool("paused", true);

    let mut the_end = NbtCompound::new();
    the_end.put_long("total_ticks", 1757);

    let mut payload = NbtCompound::new();
    payload.put_component("minecraft:overworld", overworld);
    payload.put_component("minecraft:the_end", the_end);
    payload
}

fn weather_payload() -> NbtCompound {
    let mut payload = NbtCompound::new();
    payload.put_int("clear_weather_time", 0);
    payload.put_int("rain_time", 90784);
    payload.put_int("thunder_time", 163205);
    payload.put_bool("raining", false);
    payload.put_bool("thundering", false);
    payload
}

fn game_rules_payload(advance_time: Option<bool>) -> NbtCompound {
    let mut payload = NbtCompound::new();
    if let Some(value) = advance_time {
        payload.put_bool("minecraft:advance_time", value);
    }
    payload.put_bool("minecraft:advance_weather", true);
    payload.put_int("minecraft:random_tick_speed", 3);
    payload.put_int("minecraft:respawn_radius", 10);
    payload
}

fn player_data(data_version: i32) -> Vec<u8> {
    player_data_tagged(Some(NbtTag::Int(data_version)))
}

fn player_data_tagged(data_version: Option<NbtTag>) -> Vec<u8> {
    let mut root = NbtCompound::new();
    if let Some(tag) = data_version {
        root.put("DataVersion", tag);
    }
    root.put_list(
        "Pos",
        vec![
            NbtTag::Double(48.8682436000792),
            NbtTag::Double(73.02442408821369),
            NbtTag::Double(-28.14235365298639),
        ],
    );
    root.put_list(
        "Rotation",
        vec![NbtTag::Float(OBSERVED_YAW), NbtTag::Float(OBSERVED_PITCH)],
    );
    root.put_string("Dimension", "minecraft:overworld".to_string());
    root.put("UUID", NbtTag::IntArray(OBSERVED_UUID_INTS.to_vec()));
    gzip(root)
}

#[test]
fn level_dat_reads_the_fields_we_consume() {
    let level = parse_level_dat(&level_dat(VERSION.world_version, true), path()).unwrap();
    assert_eq!(level.level_name, "New World");
    assert_eq!(level.time, 25);
    assert_eq!(
        level.spawn,
        RespawnData {
            dimension: mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
            pos: [0, 70, 64],
            yaw: 0.0,
            pitch: 0.0,
        }
    );
}

#[test]
fn omitted_clock_fields_take_the_codec_defaults() {
    let clocks = parse_world_clocks(
        &saved_data(VERSION.world_version, world_clocks_payload()),
        path(),
    )
    .unwrap();

    assert_eq!(
        clocks[&clock("minecraft:overworld")],
        ClockState {
            total_ticks: 1757,
            partial_tick: 0.25,
            rate: 2.0,
            paused: true,
        }
    );
    assert_eq!(
        clocks[&clock("minecraft:the_end")],
        ClockState {
            total_ticks: 1757,
            partial_tick: 0.0,
            rate: 1.0,
            paused: false,
        }
    );
}

#[test]
fn a_clock_state_writes_back_only_what_the_save_held_and_round_trips() {
    let written = mcrs_minecraft_nbt::to_nbt_compound(&ClockState {
        total_ticks: 1757,
        ..ClockState::default()
    })
    .unwrap();
    assert_eq!(keys(&written), ["total_ticks"]);

    let written = mcrs_minecraft_nbt::to_nbt_compound(&ClockState {
        total_ticks: 1757,
        partial_tick: 0.25,
        rate: 0.5,
        paused: true,
    })
    .unwrap();
    assert_eq!(
        keys(&written),
        ["total_ticks", "partial_tick", "rate", "paused"]
    );

    let clocks = parse_world_clocks(
        &saved_data(VERSION.world_version, world_clocks_payload()),
        path(),
    )
    .unwrap();
    let mut payload = NbtCompound::new();
    for (id, state) in &clocks {
        payload.put_component(
            &id.to_string(),
            mcrs_minecraft_nbt::to_nbt_compound(state).unwrap(),
        );
    }
    let reread = parse_world_clocks(&saved_data(VERSION.world_version, payload), path()).unwrap();
    assert_eq!(clocks, reread);
}

fn spawn(pos: Vec<i32>, yaw: f32, pitch: f32) -> NbtCompound {
    let mut spawn = NbtCompound::new();
    spawn.put("pos", NbtTag::IntArray(pos));
    spawn.put_float("yaw", yaw);
    spawn.put_float("pitch", pitch);
    spawn.put_string("dimension", "minecraft:overworld".to_string());
    spawn
}

#[test]
fn values_outside_their_range_are_rejected() {
    for rate in [0.0, f32::NAN, f32::INFINITY] {
        let mut overworld = NbtCompound::new();
        overworld.put_long("total_ticks", 1757);
        overworld.put_float("rate", rate);
        let mut payload = NbtCompound::new();
        payload.put_component("minecraft:overworld", overworld);

        let err =
            parse_world_clocks(&saved_data(VERSION.world_version, payload), path()).unwrap_err();
        assert!(
            matches!(err, SaveError::OutOfRange { field: "rate", .. }),
            "rate {rate} accepted: {err}"
        );
    }

    for (field, yaw, pitch) in [("spawn.yaw", 180.5, 0.0), ("spawn.pitch", 0.0, -90.5)] {
        let err = parse_level_dat(
            &level_dat_with_spawn(spawn(vec![0, 70, 64], yaw, pitch)),
            path(),
        )
        .unwrap_err();
        assert!(
            matches!(err, SaveError::OutOfRange { field: f, .. } if f == field),
            "{field}: {err}"
        );
    }

    let err =
        parse_level_dat(&level_dat_with_spawn(spawn(vec![0, 70], 0.0, 0.0)), path()).unwrap_err();
    assert!(
        matches!(
            err,
            SaveError::WrongLength {
                found: 2,
                expected: 3,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn weather_reads_all_five_fields() {
    let weather = parse_weather(
        &saved_data(VERSION.world_version, weather_payload()),
        path(),
    )
    .unwrap();
    assert_eq!(
        weather,
        WeatherData {
            clear_weather_time: 0,
            rain_time: 90784,
            thunder_time: 163205,
            raining: false,
            thundering: false,
        }
    );
}

#[test]
fn advance_time_defaults_to_true_and_unrelated_rules_are_ignored() {
    let rules = parse_game_rules(
        &saved_data(VERSION.world_version, game_rules_payload(None)),
        path(),
    )
    .unwrap();
    assert!(rules.advance_time);

    let rules = parse_game_rules(
        &saved_data(VERSION.world_version, game_rules_payload(Some(false))),
        path(),
    )
    .unwrap();
    assert!(!rules.advance_time);
}

#[test]
fn an_unnamespaced_advance_time_key_is_not_the_rule() {
    let mut payload = NbtCompound::new();
    payload.put_bool("advance_time", false);
    let rules = parse_game_rules(&saved_data(VERSION.world_version, payload), path()).unwrap();
    assert!(rules.advance_time);
}

#[test]
fn singleplayer_uuid_ints_name_the_player_file_when_a_player_has_opened_the_world() {
    let level = parse_level_dat(&level_dat(VERSION.world_version, true), path()).unwrap();
    assert_eq!(
        level.singleplayer_uuid.unwrap().hyphenated().to_string(),
        OBSERVED_UUID_FILE_NAME
    );

    let level = parse_level_dat(&level_dat(VERSION.world_version, false), path()).unwrap();
    assert!(level.singleplayer_uuid.is_none());
}

fn normal_dimensions() -> Dimensions {
    let set = test_registries();
    let id = set
        .registry::<crate::worldgen::world_preset::WorldPreset>()
        .and_then(|registry| registry.get(&crate::keys::world_preset::NORMAL))
        .expect("the normal preset is loaded");
    set.entries::<crate::worldgen::world_preset::WorldPreset, WorldPreset>()
        .unwrap()[id]
        .dimensions
        .clone()
}

fn world_gen_settings_payload() -> NbtCompound {
    let dimensions = test_registries()
        .scope(|| mcrs_minecraft_nbt::to_nbt_compound(&normal_dimensions()))
        .unwrap();

    let mut payload = NbtCompound::new();
    payload.put_bool("bonus_chest", false);
    payload.put_long("seed", 2);
    payload.put_bool("generate_structures", true);
    payload.put_component("dimensions", dimensions);
    payload
}

fn every_file_kind_at(data_version: i32) -> [Result<(), SaveError>; 6] {
    every_file_kind_tagged(Some(NbtTag::Int(data_version)))
}

fn every_file_kind_tagged(data_version: Option<NbtTag>) -> [Result<(), SaveError>; 6] {
    let tag = || data_version.clone();
    [
        parse_level_dat(&level_dat_tagged(tag(), true), path()).map(drop),
        parse_world_clocks(&saved_data_tagged(tag(), world_clocks_payload()), path()).map(drop),
        parse_world_gen_settings(
            &saved_data_tagged(tag(), world_gen_settings_payload()),
            path(),
            test_registries(),
        )
        .map(drop),
        parse_weather(&saved_data_tagged(tag(), weather_payload()), path()).map(drop),
        parse_game_rules(&saved_data_tagged(tag(), game_rules_payload(None)), path()).map(drop),
        player::parse_player_dat(&player_data_tagged(tag()), path()).map(drop),
    ]
}

#[test]
fn a_data_version_other_than_the_world_version_is_rejected_by_name_on_every_file_kind() {
    let current = VERSION.world_version;
    for found in [4903, current - 1, current + 1] {
        for result in every_file_kind_at(found) {
            let err = result.unwrap_err();
            assert!(
                matches!(
                    err,
                    SaveError::DataVersion { found: f, expected, .. }
                        if f == found && expected == current
                ),
                "{err}"
            );
            let message = err.to_string();
            assert!(
                message.ends_with(&format!("DataVersion {found}, expected {current}")),
                "{message}"
            );
        }
    }
}

#[test]
fn a_file_without_a_data_version_is_rejected_on_every_file_kind() {
    for (kind, result) in every_file_kind_tagged(None).into_iter().enumerate() {
        let Err(err) = result else {
            panic!("file kind {kind} loaded without a DataVersion");
        };
        assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
        assert!(
            err.to_string().contains("missing field `DataVersion`"),
            "{err}"
        );
    }
}

#[test]
fn a_refused_player_file_is_left_byte_identical() {
    let world = std::env::temp_dir().join(format!("mcrs_save_refused_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&world);
    let uuid = Uuid::from_u128(0x77);
    let dir = world.join("players/data");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{}.dat", uuid.hyphenated()));
    let stale = player_data(VERSION.world_version - 1);
    std::fs::write(&file, &stale).unwrap();

    let err = read_player_dat(&world, uuid).unwrap_err();
    assert!(matches!(err, SaveError::DataVersion { .. }), "{err}");
    assert_eq!(std::fs::read(&file).unwrap(), stale);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(world);
}

#[test]
fn a_missing_file_is_distinguishable_from_a_corrupt_one() {
    let err = read_weather(Path::new("/nonexistent-world")).unwrap_err();
    assert!(matches!(err, SaveError::Missing { .. }), "{err}");
}

#[test]
fn world_gen_settings_reads_the_seed_past_the_fields_we_ignore() {
    let payload = world_gen_settings_payload();
    let settings = parse_world_gen_settings(
        &saved_data(VERSION.world_version, payload),
        path(),
        test_registries(),
    )
    .unwrap();
    assert_eq!(
        settings,
        WorldGenSettings {
            seed: 2,
            dimensions: normal_dimensions(),
        }
    );
}

#[test]
fn world_gen_settings_keep_their_dimensions() {
    let set = test_registries();
    let world = std::env::temp_dir().join(format!("mcrs_save_gen_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&world);
    let mut dimensions = normal_dimensions();
    let extra =
        ResourceKey::<mcrs_minecraft_dimension::Dimension>::from_location(clock("test:extra"));
    let overworld = dimensions["minecraft:overworld"].clone();
    dimensions.insert(extra, overworld);
    let settings = WorldGenSettings {
        seed: -7,
        dimensions,
    };

    write_world_gen_settings(&world, &settings, set).unwrap();
    assert_eq!(read_world_gen_settings(&world, set).unwrap(), settings);
    let _ = std::fs::remove_dir_all(world);
}

#[derive(Serialize)]
struct PlayerFile<'a> {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "Pos")]
    pos: Vec<f64>,
    #[serde(rename = "Rotation")]
    rotation: Vec<f32>,
    #[serde(rename = "Dimension", skip_serializing_if = "Option::is_none")]
    dimension: Option<&'a str>,
    #[serde(rename = "Inventory")]
    inventory: Vec<i32>,
    #[serde(rename = "SelectedItemSlot")]
    selected_item_slot: i32,
}

fn player_file(dimension: Option<&str>) -> Vec<u8> {
    mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(&PlayerFile {
        data_version: VERSION.world_version,
        pos: vec![1.5, 64.0, -2.5],
        rotation: vec![90.0, 0.0],
        dimension,
        inventory: Vec::new(),
        selected_item_slot: 0,
    })
    .unwrap()
}

#[test]
fn a_saved_dimension_keeps_its_text() {
    let written = player_file(Some("minecraft:the_nether"));
    let dat = player::parse_player_dat(&written, path()).unwrap();
    assert_eq!(dat.dimension.as_str(), "minecraft:the_nether");

    let rewritten = mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(&dat).unwrap();
    assert_eq!(rewritten, written);

    let again = player::parse_player_dat(&rewritten, path()).unwrap();
    assert_eq!(
        mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(&again).unwrap(),
        rewritten
    );

    let error = player::parse_player_dat(&player_file(Some("Not A Dimension")), path())
        .unwrap_err()
        .to_string();
    assert!(error.contains("Not A Dimension"), "{error}");
}

#[test]
fn a_player_save_without_a_dimension_reads_as_the_overworld() {
    let dat = player::parse_player_dat(&player_file(None), path()).unwrap();
    assert_eq!(dat.dimension.as_str(), "minecraft:overworld");
    assert_eq!(dat.pos, [1.5, 64.0, -2.5]);
}
