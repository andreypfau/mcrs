use bevy_app::App;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::{PlayerPersistencePlugin, load_player, write_player};
use mcrs_minecraft_engine_player_vitals::PlayerVitalsPlugin;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_save::{PlayerDat, read_player_dat, write_player_dat};

#[test]
fn a_joining_player_takes_the_saved_health_and_a_save_writes_the_change() {
    let dir = std::env::temp_dir().join(format!("mcrs-player-vitals-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let id = Uuid::from_u128(0x7);
    let mut dat = PlayerDat::default();
    dat.root.child_tags.clear();
    dat.root.put("Health", NbtTag::Float(7.0));
    write_player_dat(&dir, id, &dat).unwrap();

    let mut app = App::new();
    app.add_plugins((PlayerPersistencePlugin, PlayerVitalsPlugin))
        .insert_resource(WorldSave(dir.clone()));
    let profile = GameProfile {
        id,
        username: "hurt".to_owned(),
        properties: Vec::new(),
    };
    let player = app
        .world_mut()
        .spawn((Player, profile, Transform::default()))
        .id();
    load_player(app.world_mut(), player);

    let vitals = *app.world().get::<Vitals>(player).unwrap();
    assert_eq!(
        (vitals.health, vitals.is_alive()),
        (7.0, true),
        "a joining player takes the health of its player file and stays alive above zero"
    );

    app.world_mut().get_mut::<Vitals>(player).unwrap().health = 3.5;
    write_player(app.world_mut(), player, CaptureCause::Autosave);
    let saved = read_player_dat(&dir, id).unwrap().unwrap();
    assert_eq!(
        saved.get("Health"),
        Some(&NbtTag::Float(3.5)),
        "a save writes the health the player has at that moment"
    );
    assert_eq!(
        saved.get("DeathTime"),
        Some(&NbtTag::Short(0)),
        "a living player saves a death time of zero"
    );

    let newcomer = GameProfile {
        id: Uuid::from_u128(0x8),
        username: "new".to_owned(),
        properties: Vec::new(),
    };
    let newcomer = app
        .world_mut()
        .spawn((Player, newcomer, Transform::default()))
        .id();
    load_player(app.world_mut(), newcomer);
    assert_eq!(
        app.world().get::<Vitals>(newcomer).map(|v| v.health),
        Some(20.0),
        "a player without a player file starts at the maximum health"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_saved_health_outside_the_valid_range_loads_as_the_maximum_health() {
    let dir = std::env::temp_dir().join(format!("mcrs-player-vitals-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = App::new();
    app.add_plugins((PlayerPersistencePlugin, PlayerVitalsPlugin))
        .insert_resource(WorldSave(dir.clone()));
    let cases = [
        (0x10, f32::NAN, "a health that is not a number"),
        (0x11, f32::INFINITY, "an infinite health"),
        (0x12, 1.0e6, "a health above the maximum"),
    ];
    for (raw_id, saved_health, case) in cases {
        let id = Uuid::from_u128(raw_id);
        let mut dat = PlayerDat::default();
        dat.root.child_tags.clear();
        dat.root.put("Health", NbtTag::Float(saved_health));
        write_player_dat(&dir, id, &dat).unwrap();
        let profile = GameProfile {
            id,
            username: "edge".to_owned(),
            properties: Vec::new(),
        };
        let player = app
            .world_mut()
            .spawn((Player, profile, Transform::default()))
            .id();
        load_player(app.world_mut(), player);
        assert_eq!(
            app.world()
                .get::<Vitals>(player)
                .map(|v| (v.health, v.is_alive())),
            Some((20.0, true)),
            "{case} loads as the maximum health of a player"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}
