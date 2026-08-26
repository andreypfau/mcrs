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
    data.put_int("DataVersion", WORLD_VERSION);
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
    let level = parse_level_dat(&level_dat(WORLD_VERSION, true), path()).unwrap();
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
    root.put_component("data", level_data(WORLD_VERSION, true));
    let err = parse_level_dat(&gzip(root), path()).unwrap_err();
    assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
}

#[test]
fn saved_data_wrapped_like_level_dat_is_an_error() {
    let mut root = NbtCompound::new();
    root.put_component("Data", weather_payload());
    root.put_int("DataVersion", WORLD_VERSION);
    let err = parse_weather(&gzip(root), path()).unwrap_err();
    assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
}

#[test]
fn player_data_has_no_wrapper() {
    let player = parse_player(&player_data(WORLD_VERSION), path()).unwrap();
    assert_eq!(
        player.pos,
        [48.8682436000792, 73.02442408821369, -28.14235365298639]
    );
    assert_eq!(player.yaw, OBSERVED_YAW);
    assert_eq!(player.pitch, OBSERVED_PITCH);
    assert_eq!(player.dimension, "minecraft:overworld");
}

#[test]
fn omitted_clock_fields_take_the_codec_defaults() {
    let clocks =
        parse_world_clocks(&saved_data(WORLD_VERSION, world_clocks_payload()), path()).unwrap();

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

    let err = parse_world_clocks(&saved_data(WORLD_VERSION, payload), path()).unwrap_err();
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
    let clocks =
        parse_world_clocks(&saved_data(WORLD_VERSION, world_clocks_payload()), path()).unwrap();

    let mut payload = NbtCompound::new();
    for (id, state) in &clocks {
        payload.put_component(
            &id.to_string(),
            mcrs_minecraft_nbt::to_nbt_compound(state).unwrap(),
        );
    }
    let reread = parse_world_clocks(&saved_data(WORLD_VERSION, payload), path()).unwrap();

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

        let err = parse_world_clocks(&saved_data(WORLD_VERSION, payload), path()).unwrap_err();
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
    let weather = parse_weather(&saved_data(WORLD_VERSION, weather_payload()), path()).unwrap();
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
    let err = parse_weather(&saved_data(WORLD_VERSION, payload), path()).unwrap_err();
    assert!(matches!(err, SaveError::Nbt { .. }), "{err}");
}

#[test]
fn advance_time_defaults_to_true_and_unrelated_rules_are_ignored() {
    let rules =
        parse_game_rules(&saved_data(WORLD_VERSION, game_rules_payload(None)), path()).unwrap();
    assert!(rules.advance_time);

    let rules = parse_game_rules(
        &saved_data(WORLD_VERSION, game_rules_payload(Some(false))),
        path(),
    )
    .unwrap();
    assert!(!rules.advance_time);
}

#[test]
fn an_unnamespaced_advance_time_key_is_not_the_rule() {
    let mut payload = NbtCompound::new();
    payload.put_bool("advance_time", false);
    let rules = parse_game_rules(&saved_data(WORLD_VERSION, payload), path()).unwrap();
    assert!(rules.advance_time);
}

#[test]
fn singleplayer_uuid_ints_name_the_player_file() {
    let level = parse_level_dat(&level_dat(WORLD_VERSION, true), path()).unwrap();
    assert_eq!(
        level.singleplayer_uuid.unwrap().hyphenated().to_string(),
        OBSERVED_UUID_FILE_NAME
    );
}

#[test]
fn a_world_no_player_has_opened_has_no_singleplayer_uuid() {
    let level = parse_level_dat(&level_dat(WORLD_VERSION, false), path()).unwrap();
    assert!(level.singleplayer_uuid.is_none());
}

#[test]
fn the_previous_data_version_is_rejected_by_name_on_every_file_kind() {
    let stale = 4903;
    let errors = [
        parse_level_dat(&level_dat(stale, true), path()).unwrap_err(),
        parse_world_clocks(&saved_data(stale, world_clocks_payload()), path()).unwrap_err(),
        parse_weather(&saved_data(stale, weather_payload()), path()).unwrap_err(),
        parse_game_rules(&saved_data(stale, game_rules_payload(None)), path()).unwrap_err(),
        parse_player(&player_data(stale), path()).unwrap_err(),
    ];
    for err in errors {
        assert!(
            matches!(
                err,
                SaveError::DataVersion {
                    found: 4903,
                    expected: WORLD_VERSION,
                    ..
                }
            ),
            "{err}"
        );
        let message = err.to_string();
        assert!(
            message.contains("4903") && message.contains("5011"),
            "{message}"
        );
    }
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
