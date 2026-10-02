use std::path::Path;
use std::sync::Arc;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
use mcrs_minecraft_nbt::tag::NbtTag;

use super::*;

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
    ResourceLocation::parse(id).unwrap()
}

fn spawn_compound() -> NbtCompound {
    let mut spawn = NbtCompound::new();
    spawn.put("pos", NbtTag::IntArray(vec![0, 70, 64]));
    spawn.put_float("yaw", 0.0);
    spawn.put_float("pitch", 0.0);
    spawn.put_string("dimension", "minecraft:overworld".to_string());
    spawn
}

fn level_data(data_version: i32, with_uuid: bool) -> NbtCompound {
    let mut data = NbtCompound::new();
    data.put_int("DataVersion", data_version);
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
    let mut root = NbtCompound::new();
    root.put_component("Data", level_data(data_version, with_uuid));
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
    let mut root = NbtCompound::new();
    root.put_component("data", payload);
    root.put_int("DataVersion", data_version);
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
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", data_version);
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
            dimension: "minecraft:overworld".to_string(),
            pos: [0, 70, 64],
            yaw: 0.0,
            pitch: 0.0,
        }
    );
}

#[test]
fn level_dat_wrapped_like_a_saved_data_file_is_an_error() {
    let mut root = NbtCompound::new();
    root.put_component("data", level_data(VERSION.world_version, true));
    let err = parse_level_dat(&gzip(root), path()).unwrap_err();
    assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
}

#[test]
fn saved_data_wrapped_like_level_dat_is_an_error() {
    let mut root = NbtCompound::new();
    root.put_component("Data", weather_payload());
    root.put_int("DataVersion", VERSION.world_version);
    let err = parse_weather(&gzip(root), path()).unwrap_err();
    assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
}

#[test]
fn player_data_has_no_wrapper() {
    let player = player::parse_player_dat(&player_data(VERSION.world_version), path()).unwrap();
    assert_eq!(
        player.pos,
        [48.8682436000792, 73.02442408821369, -28.14235365298639]
    );
    assert_eq!(player.rotation, [OBSERVED_YAW, OBSERVED_PITCH]);
    assert_eq!(player.dimension, "minecraft:overworld");
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
fn a_zero_clock_rate_is_rejected() {
    let mut overworld = NbtCompound::new();
    overworld.put_long("total_ticks", 1757);
    overworld.put_float("rate", 0.0);
    let mut payload = NbtCompound::new();
    payload.put_component("minecraft:overworld", overworld);

    let err = parse_world_clocks(&saved_data(VERSION.world_version, payload), path()).unwrap_err();
    assert!(
        matches!(err, SaveError::OutOfRange { field: "rate", .. }),
        "{err}"
    );
}

#[test]
fn a_clock_state_writes_back_only_what_the_save_held() {
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
}

#[test]
fn a_clock_state_round_trips_through_the_save_shape() {
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

#[test]
fn a_non_finite_clock_rate_is_rejected() {
    for rate in [f32::NAN, f32::INFINITY] {
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
}

#[test]
fn a_spawn_angle_outside_its_range_is_rejected() {
    for (field, yaw, pitch) in [("spawn.yaw", 180.5, 0.0), ("spawn.pitch", 0.0, -90.5)] {
        let mut spawn = NbtCompound::new();
        spawn.put("pos", NbtTag::IntArray(vec![0, 70, 64]));
        spawn.put_float("yaw", yaw);
        spawn.put_float("pitch", pitch);
        spawn.put_string("dimension", "minecraft:overworld".to_string());

        let err = parse_level_dat(&level_dat_with_spawn(spawn), path()).unwrap_err();
        assert!(
            matches!(err, SaveError::OutOfRange { field: f, .. } if f == field),
            "{field}: {err}"
        );
    }
}

#[test]
fn a_spawn_position_of_the_wrong_length_is_rejected() {
    let mut spawn = NbtCompound::new();
    spawn.put("pos", NbtTag::IntArray(vec![0, 70]));
    spawn.put_float("yaw", 0.0);
    spawn.put_float("pitch", 0.0);
    spawn.put_string("dimension", "minecraft:overworld".to_string());

    let err = parse_level_dat(&level_dat_with_spawn(spawn), path()).unwrap_err();
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
fn weather_missing_a_required_field_is_an_error() {
    let mut payload = weather_payload();
    payload
        .child_tags
        .retain(|(name, _)| name != "thunder_time");
    let err = parse_weather(&saved_data(VERSION.world_version, payload), path()).unwrap_err();
    assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
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
fn singleplayer_uuid_ints_name_the_player_file() {
    let level = parse_level_dat(&level_dat(VERSION.world_version, true), path()).unwrap();
    assert_eq!(
        level.singleplayer_uuid.unwrap().hyphenated().to_string(),
        OBSERVED_UUID_FILE_NAME
    );
}

#[test]
fn a_world_no_player_has_opened_has_no_singleplayer_uuid() {
    let level = parse_level_dat(&level_dat(VERSION.world_version, false), path()).unwrap();
    assert!(level.singleplayer_uuid.is_none());
}

fn world_gen_settings_payload() -> NbtCompound {
    let mut generator = NbtCompound::new();
    generator.put_string("type", "minecraft:noise".to_string());
    generator.put_string("settings", "minecraft:overworld".to_string());
    let mut overworld = NbtCompound::new();
    overworld.put_string("type", "minecraft:overworld".to_string());
    overworld.put_component("generator", generator);
    let mut dimensions = NbtCompound::new();
    dimensions.put_component("minecraft:overworld", overworld);

    let mut payload = NbtCompound::new();
    payload.put_bool("bonus_chest", false);
    payload.put_long("seed", 2);
    payload.put_bool("generate_structures", true);
    payload.put_component("dimensions", dimensions);
    payload
}

fn every_file_kind_at(data_version: i32) -> [Result<(), SaveError>; 6] {
    [
        parse_level_dat(&level_dat(data_version, true), path()).map(drop),
        parse_world_clocks(&saved_data(data_version, world_clocks_payload()), path()).map(drop),
        parse_world_gen_settings(
            &saved_data(data_version, world_gen_settings_payload()),
            path(),
        )
        .map(drop),
        parse_weather(&saved_data(data_version, weather_payload()), path()).map(drop),
        parse_game_rules(&saved_data(data_version, game_rules_payload(None)), path()).map(drop),
        player::parse_player_dat(&player_data(data_version), path()).map(drop),
    ]
}

#[test]
fn every_file_kind_loads_at_the_world_version() {
    for result in every_file_kind_at(VERSION.world_version) {
        result.unwrap();
    }
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
fn every_error_names_the_file() {
    let err = read_weather(Path::new("/nonexistent-world")).unwrap_err();
    assert!(
        err.to_string()
            .contains("/nonexistent-world/data/minecraft/weather.dat"),
        "{err}"
    );
}

#[test]
fn world_gen_settings_reads_the_seed_past_the_fields_we_ignore() {
    let payload = world_gen_settings_payload();
    let settings =
        parse_world_gen_settings(&saved_data(VERSION.world_version, payload), path()).unwrap();
    assert_eq!(settings, WorldGenSettings { seed: 2 });
}
