//! The recipe-book packets as the vanilla 26.3-snapshot-10 stream codecs
//! wrote them, with the registry ids the vanilla buffer resolved against.

use std::collections::BTreeMap;

use mcrs_minecraft_core::codec::{Bounded, Validate};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_protocol::item::ctx::MAX_NESTING;
use mcrs_minecraft_protocol::item::{
    ComponentPatch, Damage, DecodeCtx, EncodeCtx, Holder, ItemComponentKind, ItemComponentValue,
    ItemReg, Raw, Template, TrimPattern,
};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundRecipeBookAdd, ClientboundRecipeBookRemove, ClientboundRecipeBookSettings,
    ClientboundUpdateRecipes,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundPlaceRecipe, ServerboundRecipeBookChangeSettings, ServerboundRecipeBookSeenRecipe,
};
use mcrs_minecraft_protocol::recipe::{
    Ingredient, RecipeBookCategory, RecipeBookEntry, RecipeBookSettings, RecipeBookType,
    RecipeBookTypeSettings, RecipeDisplay, RecipeDisplayType, SelectableRecipe, SlotDisplay,
    SlotDisplayType,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registry::RegistryLookup;

use crate::harness::TestLookup;

const GOLDEN: &str = include_str!("../fixtures/recipe_packets_golden.txt");

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn golden() -> (TestLookup, BTreeMap<String, Vec<u8>>) {
    let mut lookup = TestLookup::new();
    let mut ids: BTreeMap<&str, Vec<(&str, u32)>> = BTreeMap::new();
    let mut wires = BTreeMap::new();
    for line in GOLDEN.lines() {
        let mut parts = line.splitn(3, ' ');
        let (label, key, value) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        if label == "id" {
            let (name, id) = value.split_once(' ').unwrap();
            let path = name.strip_prefix("minecraft:").unwrap();
            ids.entry(key)
                .or_default()
                .push((path, id.parse().unwrap()));
        } else {
            assert_eq!(key, "wire");
            wires.insert(label.to_string(), hex(value));
        }
    }
    for (registry, entries) in ids {
        let registry: &'static str = Box::leak(registry.to_string().into_boxed_str());
        lookup.registry_with_ids(registry, &entries);
    }
    (lookup, wires)
}

fn key(path: &str) -> ResourceKey<ItemReg> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

fn item(path: &str) -> SlotDisplay {
    SlotDisplay::Item { item: key(path) }
}

fn nested(display: SlotDisplay) -> Box<SlotDisplay> {
    Box::new(display)
}

fn planks() -> HolderSet<ResourceKey<ItemReg>> {
    HolderSet::Tag(ResourceLocation::minecraft("planks"))
}

fn expected_entries() -> Vec<RecipeBookEntry> {
    let sword = Template::new(
        key("diamond_sword"),
        2,
        ComponentPatch {
            added: vec![ItemComponentValue::Damage(Damage(Bounded(3)))],
            removed: vec![],
        },
    )
    .unwrap();
    vec![
        RecipeBookEntry {
            id: VarInt(0),
            display: RecipeDisplay::CraftingShapeless {
                ingredients: vec![
                    item("stone"),
                    SlotDisplay::Tag { tag: planks() },
                    SlotDisplay::Tag {
                        tag: HolderSet::List(vec![key("stone"), key("apple")]),
                    },
                ],
                result: SlotDisplay::ItemStack { item: sword },
                crafting_station: item("crafting_table"),
            },
            group: Some(5),
            category: RecipeBookCategory::CraftingMisc,
            crafting_requirements: Some(vec![
                Ingredient(HolderSet::One(key("stone"))),
                Ingredient(planks()),
            ]),
            flags: RecipeBookEntry::FLAG_NOTIFICATION | RecipeBookEntry::FLAG_HIGHLIGHT,
        },
        RecipeBookEntry {
            id: VarInt(1),
            display: RecipeDisplay::CraftingShaped {
                width: 2,
                height: 1,
                ingredients: vec![SlotDisplay::Empty, SlotDisplay::AnyFuel],
                result: SlotDisplay::WithRemainder {
                    input: nested(item("water_bucket")),
                    remainder: nested(item("bucket")),
                },
                crafting_station: SlotDisplay::Composite {
                    contents: vec![item("crafting_table"), item("stone")],
                },
            },
            group: None,
            category: RecipeBookCategory::CraftingBuildingBlocks,
            crafting_requirements: None,
            flags: 0,
        },
        RecipeBookEntry {
            id: VarInt(2),
            display: RecipeDisplay::Furnace {
                ingredient: SlotDisplay::WithAnyPotion {
                    contents: nested(item("potion")),
                },
                fuel: SlotDisplay::AnyFuel,
                result: SlotDisplay::OnlyWithComponent {
                    contents: nested(item("diamond_sword")),
                    component: ItemComponentKind::Damage,
                },
                crafting_station: item("furnace"),
                duration: 200,
                experience: 0.35,
            },
            group: Some(0),
            category: RecipeBookCategory::FurnaceFood,
            crafting_requirements: Some(vec![]),
            flags: RecipeBookEntry::FLAG_NOTIFICATION,
        },
        RecipeBookEntry {
            id: VarInt(300),
            display: RecipeDisplay::Stonecutter {
                input: SlotDisplay::Dyed {
                    dye: nested(item("red_dye")),
                    target: nested(item("leather_helmet")),
                },
                result: SlotDisplay::SmithingTrim {
                    base: nested(item("iron_chestplate")),
                    material: nested(item("stone")),
                    pattern: Holder::reference(ResourceLocation::minecraft("coast")),
                },
                crafting_station: SlotDisplay::Empty,
            },
            group: None,
            category: RecipeBookCategory::Stonecutter,
            crafting_requirements: None,
            flags: RecipeBookEntry::FLAG_HIGHLIGHT,
        },
        RecipeBookEntry {
            id: VarInt(4),
            display: RecipeDisplay::Smithing {
                template: item("netherite_upgrade_smithing_template"),
                base: item("iron_chestplate"),
                addition: item("netherite_ingot"),
                result: SlotDisplay::SmithingTrim {
                    base: nested(item("iron_chestplate")),
                    material: nested(item("netherite_ingot")),
                    pattern: Holder::Direct(TrimPattern {
                        asset_id: ResourceLocation::new("mcrs", "wave"),
                        description: Text::text("Wave"),
                        decal: true,
                    }),
                },
                crafting_station: item("smithing_table"),
            },
            group: None,
            category: RecipeBookCategory::Smithing,
            crafting_requirements: None,
            flags: 0,
        },
    ]
}

fn check<'a, P: Encode + Decode<'a> + PartialEq + std::fmt::Debug>(wire: &'a [u8], expected: &P) {
    let mut r = wire;
    let decoded = P::decode(&mut r).expect("decode");
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    assert_eq!(&decoded, expected);
    let mut out = Vec::new();
    expected.encode(&mut out).expect("encode");
    assert_eq!(out, wire);
}

fn raw<T: EncodeCtx + for<'a> DecodeCtx<'a>>(value: &T, lookup: &dyn RegistryLookup) -> Raw<T> {
    Raw::from_value(value, lookup).unwrap()
}

#[test]
fn recipe_book_add_matches_vanilla_and_resolves_every_display() {
    let (lookup, wires) = golden();
    let entries = expected_entries();
    let packet = ClientboundRecipeBookAdd {
        entries: entries.iter().map(|entry| raw(entry, &lookup)).collect(),
        replace: true,
    };
    check(&wires["recipe_book_add"], &packet);

    let mut r = &wires["recipe_book_add"][..];
    let decoded = ClientboundRecipeBookAdd::decode(&mut r).unwrap();
    let resolved: Vec<RecipeBookEntry> = decoded
        .entries
        .iter()
        .map(|entry| entry.resolve(&lookup).unwrap())
        .collect();
    assert_eq!(resolved, entries);
    let kinds: Vec<RecipeDisplayType> = resolved.iter().map(|e| e.display.kind()).collect();
    assert_eq!(
        kinds,
        [
            RecipeDisplayType::CraftingShapeless,
            RecipeDisplayType::CraftingShaped,
            RecipeDisplayType::Furnace,
            RecipeDisplayType::Stonecutter,
            RecipeDisplayType::Smithing,
        ]
    );
}

#[test]
fn update_recipes_matches_vanilla() {
    let (lookup, wires) = golden();
    let smithing_base: Vec<ResourceKey<ItemReg>> = vec![key("iron_chestplate")];
    let stonecutter = [
        SelectableRecipe {
            input: Ingredient(HolderSet::One(key("stone"))),
            option_display: SlotDisplay::ItemStack {
                item: Template::new(key("stone_bricks"), 4, ComponentPatch::EMPTY).unwrap(),
            },
        },
        SelectableRecipe {
            input: Ingredient(planks()),
            option_display: item("oak_planks"),
        },
    ];
    let packet = ClientboundUpdateRecipes {
        item_sets: vec![
            (
                ResourceLocation::minecraft("smithing_base"),
                raw(&smithing_base, &lookup),
            ),
            (
                ResourceLocation::minecraft("furnace_input"),
                raw(&Vec::new(), &lookup),
            ),
        ],
        stonecutter_recipes: stonecutter.iter().map(|e| raw(e, &lookup)).collect(),
    };
    check(&wires["update_recipes"], &packet);
    assert_eq!(
        packet.item_sets[0].1.resolve(&lookup).unwrap(),
        smithing_base
    );
    assert_eq!(
        packet.stonecutter_recipes[1].resolve(&lookup).unwrap(),
        stonecutter[1]
    );
}

#[test]
fn registry_free_recipe_packets_match_vanilla() {
    let (_, wires) = golden();
    check(
        &wires["recipe_book_remove"],
        &ClientboundRecipeBookRemove {
            recipes: vec![VarInt(1), VarInt(300)],
        },
    );
    let on = |open, filtering| RecipeBookTypeSettings { open, filtering };
    check(
        &wires["recipe_book_settings"],
        &ClientboundRecipeBookSettings {
            book_settings: RecipeBookSettings {
                crafting: on(true, false),
                furnace: on(false, true),
                blast_furnace: on(false, false),
                smoker: on(true, true),
            },
        },
    );
    check(
        &wires["place_recipe"],
        &ServerboundPlaceRecipe {
            container_id: VarInt(7),
            recipe: VarInt(300),
            use_max_items: true,
        },
    );
    check(
        &wires["recipe_book_change_settings"],
        &ServerboundRecipeBookChangeSettings {
            book_type: RecipeBookType::BlastFurnace,
            is_open: true,
            is_filtering: false,
        },
    );
    check(
        &wires["recipe_book_seen_recipe"],
        &ServerboundRecipeBookSeenRecipe { recipe: VarInt(9) },
    );
}

#[test]
fn shaped_display_rejects_mismatched_dimensions() {
    let (lookup, _) = golden();
    let bad = RecipeDisplay::CraftingShaped {
        width: 2,
        height: 2,
        ingredients: vec![SlotDisplay::Empty],
        result: SlotDisplay::Empty,
        crafting_station: SlotDisplay::Empty,
    };
    let error = Raw::from_value(&bad, &lookup).unwrap_err();
    assert_eq!(error.to_string(), "Invalid shaped recipe display contents");
    let json = r#"{"type":"minecraft:crafting_shaped","width":2,"height":2,"ingredients":[{"type":"empty"}],"result":{"type":"empty"},"crafting_station":{"type":"empty"}}"#;
    let error = serde_json::from_str::<RecipeDisplay>(json).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Invalid shaped recipe display contents"),
        "{error}"
    );
    let wire = [1u8, 2, 2, 1, 0, 0, 0];
    let error = Raw::<RecipeDisplay>::decode(&mut &wire[..]).unwrap_err();
    assert_eq!(error.to_string(), "Invalid shaped recipe display contents");
}

#[test]
fn ingredient_rejects_what_vanilla_refuses_to_construct() {
    let lookup = TestLookup::new();
    let air = ResourceLocation::minecraft("air");
    let air_id = lookup.id("item", &air).unwrap() as u8;
    let empty_stonecutter = [1, SlotDisplayType::Empty as u8];
    let error = Raw::<SelectableRecipe>::decode(&mut &empty_stonecutter[..]).unwrap_err();
    assert_eq!(error.to_string(), "Ingredients can't be empty");
    let air_stonecutter = [3, air_id, 1, SlotDisplayType::Empty as u8];
    let raw = Raw::<SelectableRecipe>::decode(&mut &air_stonecutter[..]).unwrap();
    let error = raw.resolve(&lookup).unwrap_err();
    assert_eq!(error.to_string(), "Ingredient can't contain air");
    let error = Ingredient(HolderSet::List(vec![])).validate().unwrap_err();
    assert_eq!(error, "Ingredients can't be empty");
    let error = serde_json::from_str::<Ingredient>(r#"["minecraft:air"]"#).unwrap_err();
    assert!(
        error.to_string().contains("Ingredient can't contain air"),
        "{error}"
    );
    assert_eq!(
        serde_json::from_str::<Ingredient>("\"#planks\"").unwrap(),
        Ingredient(planks())
    );
}

#[test]
fn nested_displays_stop_at_the_depth_bound_instead_of_overflowing() {
    let wrapped = |levels: u32| {
        let mut wire = [SlotDisplayType::WithAnyPotion as u8].repeat(levels as usize);
        wire.push(SlotDisplayType::Empty as u8);
        wire
    };
    let wire = wrapped(MAX_NESTING - 1);
    let raw = Raw::<SlotDisplay>::decode(&mut &wire[..]).unwrap();
    assert_eq!(
        raw.resolve(&TestLookup::new()).unwrap().kind(),
        SlotDisplayType::WithAnyPotion
    );
    let error = Raw::<SlotDisplay>::decode(&mut &wrapped(MAX_NESTING)[..]).unwrap_err();
    assert_eq!(error.to_string(), "value nested deeper than 64 levels");
    let error = Raw::<SlotDisplay>::decode(&mut &wrapped(10_000)[..]).unwrap_err();
    assert_eq!(error.to_string(), "value nested deeper than 64 levels");
    let error = Raw::<SlotDisplay>::decode(&mut &wrapped(MAX_NESTING - 1)[..]);
    assert!(error.is_ok(), "the bound is reset after a failed decode");
}

#[test]
fn displays_round_trip_through_json() {
    for entry in expected_entries() {
        let json = serde_json::to_string(&entry.display).unwrap();
        let back: RecipeDisplay = serde_json::from_str(&json).unwrap();
        assert_eq!(back, entry.display, "{json}");
    }
}

#[test]
fn dispatch_ids_are_the_registry_protocol_ids() {
    let report: BTreeMap<String, serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    fn pin(entries: &serde_json::Value, ids: &[&str]) {
        assert_eq!(entries.as_object().unwrap().len(), ids.len());
        for (wire, id) in ids.iter().enumerate() {
            assert_eq!(entries[*id]["protocol_id"], wire, "{id}");
        }
    }
    pin(
        &report["minecraft:slot_display"]["entries"],
        &[
            "minecraft:empty",
            "minecraft:any_fuel",
            "minecraft:with_any_potion",
            "minecraft:only_with_component",
            "minecraft:item",
            "minecraft:item_stack",
            "minecraft:tag",
            "minecraft:dyed",
            "minecraft:smithing_trim",
            "minecraft:with_remainder",
            "minecraft:composite",
        ],
    );
    pin(
        &report["minecraft:recipe_display"]["entries"],
        &[
            "minecraft:crafting_shapeless",
            "minecraft:crafting_shaped",
            "minecraft:furnace",
            "minecraft:stonecutter",
            "minecraft:smithing",
        ],
    );
    pin(
        &report["minecraft:recipe_book_category"]["entries"],
        &[
            "minecraft:crafting_building_blocks",
            "minecraft:crafting_redstone",
            "minecraft:crafting_equipment",
            "minecraft:crafting_misc",
            "minecraft:furnace_food",
            "minecraft:furnace_blocks",
            "minecraft:furnace_misc",
            "minecraft:blast_furnace_blocks",
            "minecraft:blast_furnace_misc",
            "minecraft:smoker_food",
            "minecraft:stonecutter",
            "minecraft:smithing",
            "minecraft:campfire",
        ],
    );
}
