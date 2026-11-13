//! A player file with every key of the layout loads into typed components and writes back as the
//! same compounds.

use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_engine::PlayerPersistencePlugin;
use mcrs_minecraft_engine::UnreadablePlayerDat;
use mcrs_minecraft_engine::ability::PlayerFlying;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::load_player;
use mcrs_minecraft_engine::persisted_registry::PersistedRegistry;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine::player_state::PlayerFileState;
use mcrs_minecraft_engine::write_player;
use mcrs_minecraft_engine_item::EnderItems;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::WornEquipment;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_save::PlayerDat;
use mcrs_minecraft_save::read_player_dat;
use mcrs_minecraft_save::write_player_dat;

const UUID: u128 = 0x1234;

fn temp_world() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mcrs-player-file-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn app(dir: &std::path::Path) -> App {
    let mut app = App::new();
    app.add_plugins(PlayerPersistencePlugin)
        .add_plugins(ItemPlugin::new(test_item_table()))
        .insert_resource(WorldSave(dir.to_owned()));
    app
}

fn profile() -> GameProfile {
    GameProfile {
        id: Uuid::from_u128(UUID),
        username: "persisted".to_owned(),
        properties: Vec::new(),
    }
}

fn spawn_player(app: &mut App) -> Entity {
    let dimension = app
        .world_mut()
        .spawn(ResourceKeyBuf::<Dimension>::from(OVERWORLD))
        .id();
    app.world_mut()
        .spawn((
            profile(),
            Transform::default(),
            PlayerGameMode::default(),
            PlayerFlying(false),
            PlayerFileState::default(),
            InDimension(dimension),
        ))
        .id()
}

fn s(value: &str) -> NbtTag {
    NbtTag::String(value.to_owned())
}

fn doubles(values: [f64; 3]) -> NbtTag {
    NbtTag::List(values.map(NbtTag::Double).to_vec())
}

fn compound_of(entries: Vec<(&str, NbtTag)>) -> NbtCompound {
    let mut compound = NbtCompound::new();
    for (key, tag) in entries {
        compound.put(key, tag);
    }
    compound
}

fn compound(entries: Vec<(&str, NbtTag)>) -> NbtTag {
    NbtTag::Compound(compound_of(entries))
}

fn list_of(values: Vec<NbtTag>) -> NbtTag {
    NbtTag::List(values)
}

fn stack(id: &str, count: i32) -> Vec<(&str, NbtTag)> {
    vec![("id", s(id)), ("count", NbtTag::Int(count))]
}

fn slot_stack(slot: i8, id: &str, count: i32) -> NbtTag {
    let mut entries = vec![("Slot", NbtTag::Byte(slot))];
    entries.extend(stack(id, count));
    compound(entries)
}

fn flags(keys: &[&'static str], value: i8) -> Vec<(&'static str, NbtTag)> {
    keys.iter().map(|key| (*key, NbtTag::Byte(value))).collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "the file is one table with a row for each key of the layout"
)]
/// Every key of the layout except `DataVersion` and `Team`, which the game never writes.
fn vanilla_player() -> NbtCompound {
    let uuid = NbtTag::IntArray(vec![0, 0, 0, UUID as i32]);
    let ints = |values: &[i32]| NbtTag::IntArray(values.to_vec());
    let entity = |id: &str| {
        compound(vec![
            ("id", s(id)),
            ("Pos", doubles([1.0, 2.0, 3.0])),
            ("UUID", ints(&[7, 8, 9, 10])),
        ])
    };
    let mut recipe_book = vec![
        ("recipes", list_of(vec![s("minecraft:stick")])),
        ("toBeDisplayed", list_of(vec![s("minecraft:stick")])),
    ];
    recipe_book.extend(flags(
        &[
            "isGuiOpen",
            "isFilteringCraftable",
            "isFurnaceGuiOpen",
            "isFurnaceFilteringCraftable",
            "isBlastingFurnaceGuiOpen",
            "isBlastingFurnaceFilteringCraftable",
            "isSmokerGuiOpen",
            "isSmokerFilteringCraftable",
        ],
        1,
    ));
    let entries: Vec<(&str, NbtTag)> = vec![
        ("Pos", doubles([1.5, 64.0, -2.5])),
        ("Motion", doubles([0.125, 0.0, -0.25])),
        (
            "Rotation",
            list_of(vec![NbtTag::Float(90.0), NbtTag::Float(-12.5)]),
        ),
        ("fall_distance", NbtTag::Double(2.5)),
        ("Fire", NbtTag::Short(-20)),
        ("Air", NbtTag::Short(299)),
        ("OnGround", NbtTag::Byte(1)),
        ("Invulnerable", NbtTag::Byte(0)),
        ("PortalCooldown", NbtTag::Int(5)),
        ("invulnerable_time", NbtTag::Int(10)),
        ("UUID", uuid.clone()),
        ("CustomName", s("Steve")),
        ("CustomNameVisible", NbtTag::Byte(1)),
        ("Silent", NbtTag::Byte(1)),
        ("NoGravity", NbtTag::Byte(1)),
        ("Glowing", NbtTag::Byte(1)),
        ("TicksFrozen", NbtTag::Int(3)),
        ("HasVisualFire", NbtTag::Byte(1)),
        ("Tags", list_of(vec![s("a"), s("b")])),
        (
            "data",
            compound(vec![("foo", NbtTag::Int(1)), ("bar", ints(&[1, 2]))]),
        ),
        ("Passengers", list_of(vec![entity("minecraft:pig")])),
        ("Health", NbtTag::Float(17.5)),
        ("HurtTime", NbtTag::Short(2)),
        ("DeathTime", NbtTag::Short(0)),
        ("AbsorptionAmount", NbtTag::Float(4.0)),
        (
            "attributes",
            list_of(vec![
                compound(vec![
                    ("id", s("minecraft:max_health")),
                    ("base", NbtTag::Double(20.0)),
                    (
                        "modifiers",
                        list_of(vec![compound(vec![
                            ("id", s("example:boost")),
                            ("amount", NbtTag::Double(0.5)),
                            ("operation", s("add_multiplied_base")),
                        ])]),
                    ),
                ]),
                compound(vec![
                    ("id", s("minecraft:scale")),
                    ("base", NbtTag::Double(1.0)),
                ]),
            ]),
        ),
        (
            "active_effects",
            list_of(vec![compound(vec![
                ("id", s("minecraft:speed")),
                ("amplifier", NbtTag::Byte(1)),
                ("duration", NbtTag::Int(600)),
                ("ambient", NbtTag::Byte(0)),
                ("show_particles", NbtTag::Byte(1)),
                ("show_icon", NbtTag::Byte(0)),
                (
                    "hidden_effect",
                    compound(vec![
                        ("amplifier", NbtTag::Byte(0)),
                        ("duration", NbtTag::Int(10)),
                        ("ambient", NbtTag::Byte(0)),
                        ("show_particles", NbtTag::Byte(1)),
                    ]),
                ),
            ])]),
        ),
        ("FallFlying", NbtTag::Byte(0)),
        ("sleeping_pos", ints(&[1, 2, 3])),
        ("Brain", compound(vec![("memories", compound(vec![]))])),
        ("last_hurt_by_player", ints(&[1, 2, 3, 4])),
        ("last_hurt_by_player_memory_time", NbtTag::Int(7)),
        ("last_hurt_by_mob", ints(&[5, 6, 7, 8])),
        ("ticks_since_last_hurt_by_mob", NbtTag::Int(9)),
        (
            "equipment",
            compound(vec![
                ("offhand", compound(stack("minecraft:shield", 1))),
                (
                    "chest",
                    compound(vec![
                        ("id", s("minecraft:leather_chestplate")),
                        ("count", NbtTag::Int(1)),
                        (
                            "components",
                            compound(vec![("minecraft:damage", NbtTag::Int(5))]),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "locator_bar_icon",
            compound(vec![
                ("style", s("minecraft:default")),
                ("color", NbtTag::Int(0x00ff_0000)),
            ]),
        ),
        ("current_explosion_impact_pos", doubles([4.0, 5.0, 6.0])),
        ("current_impulse_context_reset_grace_time", NbtTag::Int(4)),
        ("Dimension", s("minecraft:overworld")),
        ("playerGameType", NbtTag::Int(0)),
        ("previousPlayerGameType", NbtTag::Int(1)),
        (
            "abilities",
            compound(vec![
                ("invulnerable", NbtTag::Byte(0)),
                ("flying", NbtTag::Byte(0)),
                ("mayfly", NbtTag::Byte(0)),
                ("instabuild", NbtTag::Byte(0)),
                ("mayBuild", NbtTag::Byte(1)),
                ("flySpeed", NbtTag::Float(0.05)),
                ("walkSpeed", NbtTag::Float(0.1)),
            ]),
        ),
        ("foodLevel", NbtTag::Int(18)),
        ("foodTickTimer", NbtTag::Int(3)),
        ("foodSaturationLevel", NbtTag::Float(4.5)),
        ("foodExhaustionLevel", NbtTag::Float(0.25)),
        ("XpLevel", NbtTag::Int(7)),
        ("XpTotal", NbtTag::Int(100)),
        ("Score", NbtTag::Int(12)),
        ("XpP", NbtTag::Float(0.5)),
        ("XpSeed", NbtTag::Int(123_456)),
        (
            "Inventory",
            list_of(vec![
                slot_stack(0, "minecraft:diamond_sword", 1),
                slot_stack(9, "minecraft:cobblestone", 64),
                slot_stack(35, "minecraft:cobblestone", 2),
            ]),
        ),
        ("SelectedItemSlot", NbtTag::Int(0)),
        (
            "EnderItems",
            list_of(vec![slot_stack(2, "minecraft:cobblestone", 5)]),
        ),
        ("SleepTimer", NbtTag::Short(0)),
        (
            "LastDeathLocation",
            compound(vec![
                ("dimension", s("minecraft:the_nether")),
                ("pos", ints(&[10, 20, 30])),
            ]),
        ),
        (
            "respawn",
            compound(vec![
                ("dimension", s("minecraft:overworld")),
                ("pos", ints(&[0, 70, 0])),
                ("yaw", NbtTag::Float(45.0)),
                ("pitch", NbtTag::Float(-10.0)),
                ("forced", NbtTag::Byte(1)),
            ]),
        ),
        ("seenCredits", NbtTag::Byte(1)),
        ("spawn_extra_particles_on_fall", NbtTag::Byte(0)),
        ("raid_omen_position", ints(&[3, 4, 5])),
        ("entered_nether_pos", doubles([7.0, 8.0, 9.0])),
        ("last_explosion_impact_pos", doubles([1.0, 1.5, 2.0])),
        (
            "warden_spawn_tracker",
            compound(vec![
                ("ticks_since_last_warning", NbtTag::Int(1)),
                ("warning_level", NbtTag::Int(2)),
                ("cooldown_ticks", NbtTag::Int(3)),
            ]),
        ),
        ("recipeBook", compound(recipe_book)),
        ("post_effects", list_of(vec![s("minecraft:nausea")])),
        ("ShoulderEntityLeft", entity("minecraft:parrot")),
        ("ShoulderEntityRight", entity("minecraft:parrot")),
        (
            "RootVehicle",
            compound(vec![
                ("Attach", ints(&[11, 12, 13, 14])),
                ("Entity", entity("minecraft:boat")),
            ]),
        ),
        (
            "ender_pearls",
            list_of(vec![compound(vec![
                ("id", s("minecraft:ender_pearl")),
                ("ender_pearl_dimension", s("minecraft:overworld")),
                ("Pos", doubles([0.0, 1.0, 2.0])),
            ])]),
        ),
    ];
    compound_of(entries)
}

fn sorted(tag: &NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(compound) => {
            let mut entries: Vec<(String, NbtTag)> = compound
                .child_tags
                .iter()
                .map(|(key, value)| (key.clone(), sorted(value)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(NbtCompound {
                child_tags: entries,
            })
        }
        NbtTag::List(items) => NbtTag::List(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

fn write_file(dir: &std::path::Path, root: NbtCompound) {
    write_player_dat(
        dir,
        profile().id,
        &PlayerDat::new(VERSION.world_version, root),
    )
    .unwrap();
}

#[test]
fn a_player_file_with_every_key_writes_back_as_the_same_compound() {
    let dir = temp_world();
    let written = vanilla_player();
    write_file(&dir, written.clone());

    let mut app = app(&dir);
    let player = spawn_player(&mut app);
    load_player(app.world_mut(), player);
    let world = app.world();
    assert!(world.get::<UnreadablePlayerDat>(player).is_none());
    assert_eq!(world.get::<PlayerFileState>(player).unwrap().score, 12);
    assert_eq!(world.get::<Selected>(player), Some(&Selected(0)));
    assert_eq!(world.get::<Slots>(player).unwrap().occupied().count(), 3);
    assert_eq!(world.get::<EnderItems>(player).unwrap().0.len(), 1);
    let worn = world.get::<WornEquipment>(player).unwrap();
    assert!(worn.chest.is_some() && worn.offhand.is_some() && worn.mainhand.is_none());

    write_player(app.world_mut(), player);
    let rewritten = read_player_dat(&dir, profile().id).unwrap().unwrap();

    assert_eq!(
        sorted(&NbtTag::Compound(rewritten.root)),
        sorted(&NbtTag::Compound(written))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_key_that_the_layout_does_not_list_fails_the_load() {
    let dir = temp_world();
    let mut root = vanilla_player();
    root.put_string("Surprise", "x".to_owned());
    write_file(&dir, root);

    let mut app = app(&dir);
    let player = spawn_player(&mut app);
    load_player(app.world_mut(), player);

    assert!(app.world().get::<UnreadablePlayerDat>(player).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_without_state_keys_loads_the_defaults_of_the_layout() {
    let dir = temp_world();
    let mut root = NbtCompound::new();
    root.put_string("Dimension", "minecraft:overworld".to_owned());
    write_file(&dir, root);

    let mut app = app(&dir);
    let player = spawn_player(&mut app);
    let arrival = *app.world().get::<Transform>(player).unwrap();
    load_player(app.world_mut(), player);

    let world = app.world();
    assert!(world.get::<UnreadablePlayerDat>(player).is_none());
    assert_eq!(world.get::<Transform>(player), Some(&arrival));
    let state = world.get::<PlayerFileState>(player).unwrap();
    assert_eq!((state.air, state.food_level, state.health), (300, 20, 20.0));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_key_that_the_game_reads_and_never_writes_is_not_written_back() {
    let dir = temp_world();
    let mut root = vanilla_player();
    root.put_string("Team", "red".to_owned());
    write_file(&dir, root);

    let mut app = app(&dir);
    let player = spawn_player(&mut app);
    load_player(app.world_mut(), player);
    assert_eq!(
        app.world()
            .get::<PlayerFileState>(player)
            .unwrap()
            .team
            .as_deref(),
        Some("red")
    );
    write_player(app.world_mut(), player);

    let rewritten = read_player_dat(&dir, profile().id).unwrap().unwrap();
    assert!(rewritten.get("Team").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

fn put_in(tag: &mut NbtTag, path: &[&str], key: &str, value: NbtTag) -> Option<()> {
    match (tag, path.split_first()) {
        (NbtTag::Compound(compound), None) => {
            compound.put(key, value);
            Some(())
        }
        (NbtTag::Compound(compound), Some((head, rest))) => {
            let (_, child) = compound
                .child_tags
                .iter_mut()
                .find(|(name, _)| name == head)?;
            put_in(child, rest, key, value)
        }
        (NbtTag::List(items), Some((&"0", rest))) => put_in(items.first_mut()?, rest, key, value),
        _ => None,
    }
}

#[test]
fn an_unknown_key_inside_a_nested_form_fails_the_load_naming_the_key() {
    let nested: [&[&str]; 12] = [
        &["abilities"],
        &["attributes", "0"],
        &["attributes", "0", "modifiers", "0"],
        &["active_effects", "0"],
        &["active_effects", "0", "hidden_effect"],
        &["locator_bar_icon"],
        &["respawn"],
        &["LastDeathLocation"],
        &["warden_spawn_tracker"],
        &["recipeBook"],
        &["RootVehicle"],
        &["Brain"],
    ];
    for path in nested {
        let dir = temp_world();
        let mut tag = NbtTag::Compound(vanilla_player());
        assert!(put_in(&mut tag, path, "Surprise", NbtTag::Byte(1)).is_some());
        let NbtTag::Compound(root) = tag else {
            return;
        };
        write_file(&dir, root);

        let mut app = app(&dir);
        let player = spawn_player(&mut app);
        let dat = read_player_dat(&dir, profile().id).unwrap().unwrap();
        let registry = app
            .world_mut()
            .remove_resource::<PersistedRegistry>()
            .unwrap();
        let snapshot =
            PersistedSnapshot::from_dat(&dat, &registry, std::path::Path::new("x.dat")).unwrap();
        let message = registry
            .apply(app.world_mut(), player, &snapshot)
            .unwrap_err()
            .to_string();
        assert!(message.contains("Surprise"), "{path:?}: {message}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn an_item_table_without_installed_registries_is_a_startup_error() {
    let result = mcrs_minecraft_engine_registries::install_item_table(
        &mut bevy_ecs::world::World::new(),
        test_item_table(),
    );
    assert!(result.is_err());
}
