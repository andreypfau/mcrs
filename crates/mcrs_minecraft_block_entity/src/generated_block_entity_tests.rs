use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{BlockPos, rl};
use mcrs_minecraft_item::{ComponentPatch, ItemStackValue, ItemStackWithSlot};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::to_nbt_compound;

use crate::generated_block_entity::fresh_listener;
use crate::*;

const POS: BlockPos = BlockPos::new(1, -2, 3);

fn container(loot: Option<&str>) -> ContainerData {
    ContainerData {
        x: 1,
        y: -2,
        z: 3,
        loot_table: loot.map(str::to_owned),
        loot_table_seed: 0,
        items: vec![ItemStackWithSlot {
            slot: 4,
            stack: ItemStackValue {
                item: ResourceKey::from_location(rl!("minecraft:bread").to_arc()),
                count: Bounded(2),
                components: ComponentPatch::EMPTY,
            },
        }],
        custom_name: Some(Text::Plain("box".to_owned())),
        components: None,
    }
}

fn furnace() -> FurnaceData {
    FurnaceData {
        x: 1,
        y: -2,
        z: 3,
        cooking_time_spent: 3,
        cooking_total_time: 200,
        lit_time_remaining: 0,
        lit_total_time: 0,
        speed_multiplier: 1.0,
        items: Vec::new(),
        recipes_used: BTreeMap::from([("minecraft:iron_ingot".to_owned(), 2)]),
        custom_name: None,
        components: None,
    }
}

fn sign_text(text: &str) -> SignText {
    SignText {
        messages: vec![
            Text::Plain(text.to_owned()),
            Text::Plain(String::new()),
            Text::Plain(String::new()),
            Text::Plain(String::new()),
        ],
        filtered_messages: None,
        color: "black".to_owned(),
        has_glowing_text: false,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "lists one block entity of every kind with its own field values, so each kind is covered by the same fixture"
)]
fn one_of_each() -> Vec<GeneratedBlockEntity> {
    use GeneratedBlockEntity::*;
    let mut sherds = NbtCompound::new();
    sherds.put_string("back", "minecraft:brick".to_owned());
    let mut profile = NbtCompound::new();
    profile.put_string("name", "Steve".to_owned());
    vec![
        Beehive {
            x: 1,
            y: -2,
            z: 3,
            bees: vec![BeeOccupant::bee(5)],
        },
        Chest(container(Some(
            "minecraft:chests/village/village_toolsmith",
        ))),
        TrappedChest(container(Some("minecraft:chests/woodland_mansion"))),
        EnderChest {
            x: 1,
            y: -2,
            z: 3,
            components: None,
        },
        Barrel(container(None)),
        Dispenser(container(None)),
        Hopper {
            x: 1,
            y: -2,
            z: 3,
            loot_table: None,
            loot_table_seed: 0,
            items: Vec::new(),
            custom_name: None,
            components: None,
            transfer_cooldown: -1,
        },
        Furnace(furnace()),
        BlastFurnace(furnace()),
        Smoker(furnace()),
        BrewingStand {
            x: 1,
            y: -2,
            z: 3,
            brew_time: 0,
            total_brew_time: 400,
            items: Vec::new(),
            fuel: 0,
            total_fuel: 20,
            speed_multiplier: 1.0,
            custom_name: None,
            components: None,
        },
        Campfire {
            x: 1,
            y: -2,
            z: 3,
            items: Vec::new(),
            cooking_times: [0, 1, 2, 3],
            cooking_total_times: [600; 4],
            components: None,
        },
        Comparator {
            x: 1,
            y: -2,
            z: 3,
            output_signal: 7,
            components: None,
        },
        Bell {
            x: 1,
            y: -2,
            z: 3,
            components: None,
        },
        CopperGolemStatue {
            x: 1,
            y: -2,
            z: 3,
            components: None,
        },
        Lectern {
            x: 1,
            y: -2,
            z: 3,
            book: Some(SavedItem {
                id: "minecraft:written_book".to_owned(),
                count: 1,
                components: None,
            }),
            page: Some(2),
            components: None,
        },
        Jigsaw {
            x: 1,
            y: -2,
            z: 3,
            name: "minecraft:empty".to_owned(),
            target: "minecraft:empty".to_owned(),
            pool: "minecraft:empty".to_owned(),
            final_state: "minecraft:netherrack".to_owned(),
            joint: Some(Joint::Aligned),
            placement_priority: 0,
            selection_priority: 0,
            components: None,
        },
        CreakingHeart {
            x: 1,
            y: -2,
            z: 3,
            creaking: Some(Uuid([1, 2, 3, 4])),
            components: None,
        },
        DecoratedPot {
            x: 1,
            y: -2,
            z: 3,
            sherds: Some(sherds),
            loot_table: Some("minecraft:archaeology/trail_ruins_common".to_owned()),
            loot_table_seed: 9,
            item: None,
            components: None,
        },
        BrushableBlock {
            x: 1,
            y: -2,
            z: 3,
            loot_table: Some("minecraft:archaeology/desert_well".to_owned()),
            loot_table_seed: -5,
            item: None,
            components: None,
        },
        Banner {
            x: 1,
            y: -2,
            z: 3,
            patterns: vec![BannerLayer {
                pattern: "minecraft:rhombus".to_owned(),
                color: "cyan".to_owned(),
            }],
            custom_name: None,
            components: None,
        },
        Sign {
            x: 1,
            y: -2,
            z: 3,
            front_text: sign_text("hello"),
            back_text: sign_text(""),
            is_waxed: true,
            allow_op_features: false,
            components: None,
        },
        Skull {
            x: 1,
            y: -2,
            z: 3,
            profile: Some(NbtTag::Compound(profile)),
            note_block_sound: None,
            custom_name: None,
            components: None,
        },
        SculkSensor {
            x: 1,
            y: -2,
            z: 3,
            last_vibration_frequency: 0,
            listener: fresh_listener(),
            components: None,
        },
        GeneratedBlockEntity::mob_spawner(POS, "minecraft:zombie"),
        TrialSpawner {
            x: 1,
            y: -2,
            z: 3,
            normal_config: Some(NbtTag::String("minecraft:trial_chamber/breeze".to_owned())),
            ominous_config: None,
            target_cooldown_length: None,
            required_player_range: None,
            registered_players: vec![Uuid([5, 6, 7, 8])],
            current_mobs: Vec::new(),
            cooldown_ends_at: 0,
            next_mob_spawns_at: 0,
            total_mobs_spawned: 0,
            spawn_data: None,
            ejecting_loot_table: None,
            components: None,
        },
        Vault {
            x: 1,
            y: -2,
            z: 3,
            config: VaultConfig::default(),
            shared_data: NbtCompound::new(),
            server_data: NbtCompound::new(),
            components: None,
        },
        EndGateway(EndGatewayData {
            x: 1,
            y: -2,
            z: 3,
            age: 4,
            exit_portal: Some([1, 2, 3]),
            exact_teleport: true,
        }),
    ]
}

#[test]
fn every_kind_round_trips_and_is_named_by_its_id() {
    let all = one_of_each();
    assert_eq!(all.len(), GeneratedBlockEntity::KINDS.len());
    for entity in all {
        let compound = to_nbt_compound(&entity).unwrap();
        let id = compound.get_string("id").unwrap();
        assert_eq!(entity.position(), POS);
        assert_eq!(entity.kind().as_static_str(), id);
        assert_eq!(
            GeneratedBlockEntity::from_compound(&compound).unwrap(),
            entity,
            "{id}"
        );
    }
}

#[test]
fn an_end_gateway_writes_its_exit_portal_as_an_int_array_and_defaults_its_age() {
    let gateway = EndGatewayData {
        x: 1,
        y: -2,
        z: 3,
        age: 4,
        exit_portal: Some([5, 6, 7]),
        exact_teleport: false,
    };
    let written = to_nbt_compound(&GeneratedBlockEntity::EndGateway(gateway)).unwrap();
    assert_eq!(
        written.get("exit_portal"),
        Some(&NbtTag::IntArray(vec![5, 6, 7]))
    );
    let mut compound = NbtCompound::new();
    compound.put_string("id", "minecraft:end_gateway".to_owned());
    for (axis, value) in [("x", 1), ("y", -2), ("z", 3)] {
        compound.put_int(axis, value);
    }
    let Ok(GeneratedBlockEntity::EndGateway(read)) = GeneratedBlockEntity::from_compound(&compound)
    else {
        panic!("an end gateway");
    };
    assert_eq!((read.age, read.exit_portal), (0, None));
}

#[test]
fn a_sign_without_texts_reads_them_as_blank() {
    let mut compound = NbtCompound::new();
    compound.put_string("id", "minecraft:sign".to_owned());
    for (axis, value) in [("x", 1), ("y", -2), ("z", 3)] {
        compound.put_int(axis, value);
    }
    let Ok(GeneratedBlockEntity::Sign {
        front_text,
        back_text,
        ..
    }) = GeneratedBlockEntity::from_compound(&compound)
    else {
        panic!("a sign");
    };
    assert_eq!(front_text, SignText::default());
    assert_eq!(back_text, SignText::default());
}

fn template_chest(with_table: bool) -> NbtCompound {
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:chest".to_owned());
    if with_table {
        nbt.put_string(
            "LootTable",
            "minecraft:chests/village/village_armorer".to_owned(),
        );
    }
    nbt.put_list("Items", Vec::new());
    nbt
}

#[test]
fn a_template_chest_takes_the_placement_seed_only_beside_a_table() {
    let seeded = GeneratedBlockEntity::from_template(&template_chest(true), POS, Some(77))
        .unwrap()
        .unwrap();
    assert_eq!(
        seeded,
        GeneratedBlockEntity::chest(
            POS,
            "minecraft:chests/village/village_armorer".to_owned(),
            77
        )
    );
    let unseeded = GeneratedBlockEntity::from_template(&template_chest(false), POS, Some(77))
        .unwrap()
        .unwrap();
    let compound = to_nbt_compound(&unseeded).unwrap();
    assert!(compound.get("LootTableSeed").is_none());
    assert!(compound.get("Items").is_none());
    assert_eq!(compound.get_int("x"), Some(1));
}

#[test]
fn foreign_ids_are_not_entities_and_only_containers_want_a_seed() {
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:structure_block".to_owned());
    assert_eq!(
        GeneratedBlockEntity::from_template(&nbt, POS, None).unwrap(),
        None
    );
    assert!(GeneratedBlockEntity::wants_loot_seed(&template_chest(true)));
    let mut brushable = NbtCompound::new();
    brushable.put_string("id", "minecraft:brushable_block".to_owned());
    assert!(!GeneratedBlockEntity::wants_loot_seed(&brushable));
    assert!(!GeneratedBlockEntity::wants_loot_seed(&nbt));
}

#[test]
fn short_timers_load_as_ints_and_save_as_ints() {
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:furnace".to_owned());
    nbt.put_short("cooking_time_spent", 12);
    nbt.put_byte("lit_total_time", 3);
    let entity = GeneratedBlockEntity::from_template(&nbt, POS, None)
        .unwrap()
        .unwrap();
    let GeneratedBlockEntity::Furnace(data) = &entity else {
        panic!("a furnace");
    };
    assert_eq!(data.cooking_time_spent, 12);
    assert_eq!(data.lit_total_time, 3);
    assert_eq!(data.speed_multiplier, 1.0);
    let compound = to_nbt_compound(&entity).unwrap();
    assert_eq!(compound.get("cooking_time_spent"), Some(&NbtTag::Int(12)));
    assert_eq!(compound.get("lit_total_time"), Some(&NbtTag::Int(3)));
    assert_eq!(compound.get("speed_multiplier"), Some(&NbtTag::Float(1.0)));
    assert!(compound.get("RecipesUsed").is_none());
}

#[test]
fn text_collapses_a_bare_literal_and_keeps_a_styled_one() {
    let mut literal = NbtCompound::new();
    literal.put_string("text", "a".to_owned());
    let mut styled = literal.clone();
    styled.put_string("color", "red".to_owned());
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:banner".to_owned());
    nbt.put_component("CustomName", literal);
    let banner = GeneratedBlockEntity::from_template(&nbt, POS, None)
        .unwrap()
        .unwrap();
    let GeneratedBlockEntity::Banner { custom_name, .. } = &banner else {
        panic!("a banner");
    };
    assert_eq!(custom_name, &Some(Text::Plain("a".to_owned())));
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:banner".to_owned());
    nbt.put_component("CustomName", styled.clone());
    let banner = GeneratedBlockEntity::from_template(&nbt, POS, None)
        .unwrap()
        .unwrap();
    let GeneratedBlockEntity::Banner { custom_name, .. } = &banner else {
        panic!("a banner");
    };
    assert_eq!(
        custom_name,
        &Some(Text::Rich(NbtTag::Compound(styled.clone())))
    );
    assert_eq!(
        to_nbt_compound(&banner).unwrap().get("CustomName"),
        Some(&NbtTag::Compound(styled))
    );
}

#[test]
fn defaults_fill_what_a_template_omits() {
    let load = |id: &str| {
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", id.to_owned());
        GeneratedBlockEntity::from_template(&nbt, POS, None)
            .unwrap()
            .unwrap()
    };
    let GeneratedBlockEntity::BrewingStand {
        total_brew_time,
        total_fuel,
        speed_multiplier,
        ..
    } = load("minecraft:brewing_stand")
    else {
        panic!("a brewing stand");
    };
    assert_eq!(
        (total_brew_time, total_fuel, speed_multiplier),
        (400, 20, 1.0)
    );
    let GeneratedBlockEntity::SculkSensor { listener, .. } = load("minecraft:sculk_sensor") else {
        panic!("a sensor");
    };
    assert_eq!(listener, fresh_listener());
    assert_eq!(
        listener.get_compound("selector").unwrap().get("tick"),
        Some(&NbtTag::Long(-1))
    );
    let vault = load("minecraft:vault");
    let GeneratedBlockEntity::Vault { config, .. } = &vault else {
        panic!("a vault");
    };
    assert_eq!(config, &VaultConfig::default());
    let saved = to_nbt_compound(&vault).unwrap();
    let config = saved.get_compound("config").unwrap();
    assert!(config.get("loot_table").is_none());
    assert!(config.get("activation_range").is_none());
    assert_eq!(
        config.get_compound("key_item").unwrap().get_string("id"),
        Some("minecraft:trial_key")
    );
    assert_eq!(
        saved.get("shared_data"),
        Some(&NbtTag::Compound(NbtCompound::new()))
    );
    let GeneratedBlockEntity::Hopper {
        transfer_cooldown, ..
    } = load("minecraft:hopper")
    else {
        panic!("a hopper");
    };
    assert_eq!(transfer_cooldown, -1);
    let GeneratedBlockEntity::MobSpawner {
        delay,
        max_spawn_delay,
        spawn_data,
        ..
    } = load("minecraft:mob_spawner")
    else {
        panic!("a spawner");
    };
    assert_eq!((delay, max_spawn_delay, spawn_data), (20, 800, None));
}

#[test]
fn a_sign_drops_filtered_messages_equal_to_its_messages() {
    let mut front = sign_text("kept");
    front.filtered_messages = Some(front.messages.clone());
    let mut back = sign_text("shown");
    back.filtered_messages = Some(sign_text("hidden").messages);
    let sign = GeneratedBlockEntity::Sign {
        x: 0,
        y: 0,
        z: 0,
        front_text: front,
        back_text: back.clone(),
        is_waxed: false,
        allow_op_features: false,
        components: None,
    };
    let compound = to_nbt_compound(&sign).unwrap();
    assert_eq!(compound.get("is_waxed"), Some(&NbtTag::Byte(0)));
    assert!(compound.get("allow_op_features").is_none());
    let GeneratedBlockEntity::Sign {
        front_text,
        back_text,
        ..
    } = GeneratedBlockEntity::from_template(&compound, POS, None)
        .unwrap()
        .unwrap()
    else {
        panic!("a sign");
    };
    assert_eq!(front_text, sign_text("kept"));
    assert_eq!(back_text, back);
}
