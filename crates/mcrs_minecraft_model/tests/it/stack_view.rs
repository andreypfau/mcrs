use std::num::NonZeroU8;
use std::sync::LazyLock;

use mcrs_minecraft_item_stack::ArgbColor;
use mcrs_minecraft_item_stack::BlockState;
use mcrs_minecraft_item_stack::BlockStateProperty;
use mcrs_minecraft_item_stack::BundleContents;
use mcrs_minecraft_item_stack::ChargedProjectiles;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomModelData;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::DyedColor;
use mcrs_minecraft_item_stack::EnchantmentGlintOverride;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::FireworkExplosion;
use mcrs_minecraft_item_stack::FireworkShape;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::LodestoneTracker;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::PotionContents;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_stack::UndecodedValue;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_asset::UnbakedItemModel;
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::item_model_eval::Evaluator;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;

const ITEMS: [(&str, &str); 9] = [
    ("compass", r#""minecraft:max_stack_size": 64"#),
    ("firework_rocket", r#""minecraft:max_stack_size": 64"#),
    ("arrow", r#""minecraft:max_stack_size": 64"#),
    (
        "iron_sword",
        r#""minecraft:max_stack_size": 1, "minecraft:max_damage": 250, "minecraft:damage": 0"#,
    ),
    (
        "bundle",
        r#""minecraft:max_stack_size": 1, "minecraft:bundle_contents": []"#,
    ),
    (
        "crossbow",
        r#""minecraft:max_stack_size": 1, "minecraft:charged_projectiles": []"#,
    ),
    ("leather_helmet", r#""minecraft:max_stack_size": 1"#),
    ("bee_nest", r#""minecraft:max_stack_size": 64"#),
    ("firework_star", r#""minecraft:max_stack_size": 64"#),
];

const COMPASS: Kind = Kind(0);
const ROCKET: Kind = Kind(1);
const ARROW: Kind = Kind(2);
const SWORD: Kind = Kind(3);
const BUNDLE: Kind = Kind(4);
const CROSSBOW: Kind = Kind(5);
const HELMET: Kind = Kind(6);
const BEE_NEST: Kind = Kind(7);
const STAR: Kind = Kind(8);

struct NoRegistries;

impl NameLookup for NoRegistries {
    fn id(&self, _: NameTable, _: ResourceLocation<'_>) -> Option<u16> {
        None
    }

    fn name(&self, _: NameTable, _: u16) -> Option<ResourceLocation<'_>> {
        None
    }
}

static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    let files: Vec<(String, String)> = (0u16..)
        .zip(ITEMS)
        .map(|(id, (name, components))| {
            let json = format!(
                r#"{{"minecraft:item": {{"description": {{"identifier": "minecraft:{name}", "protocol_id": {id}}}, "components": {{{components}}}}}}}"#
            );
            (name.to_owned(), json)
        })
        .collect();
    let definitions = files.iter().map(|(name, json)| DefinitionFile {
        name,
        bytes: json.as_bytes(),
    });
    bake(definitions, &NoRegistries).unwrap().table
});

fn stack(kind: Kind, count: u8) -> ItemStack {
    ItemStack::new(kind, Count::new(count).unwrap())
}

fn one(kind: Kind) -> ItemStack {
    stack(kind, 1)
}

fn enchanted(kind: Kind) -> ItemStack {
    let level = EnchantmentLevel {
        enchantment: EnchantmentId(0),
        level: NonZeroU8::MIN,
    };
    one(kind).set(Enchantments::new([level]).unwrap())
}

fn lodestone(kind: Kind) -> ItemStack {
    one(kind).set(LodestoneTracker {
        target: None,
        tracked: true,
    })
}

fn model_data() -> CustomModelData {
    CustomModelData {
        floats: vec![0.75],
        flags: vec![false, true],
        strings: vec!["b".to_owned()],
        colors: vec![ArgbColor(0x0012_3456)],
    }
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Condition(bool),
    Select(Option<usize>),
    Range(f32),
    Tint(u32),
    Foil(bool),
}

const LEAF: &str = r#"{"type": "minecraft:empty"}"#;

fn condition(property: &str) -> String {
    format!(
        r#"{{"type": "minecraft:condition", {property}, "on_true": {LEAF}, "on_false": {LEAF}}}"#
    )
}

fn select(property: &str, cases: &[&str]) -> String {
    let cases: Vec<String> = cases
        .iter()
        .map(|when| format!(r#"{{"when": {when}, "model": {LEAF}}}"#))
        .collect();
    format!(
        r#"{{"type": "minecraft:select", {property}, "cases": [{}]}}"#,
        cases.join(", ")
    )
}

fn range(property: &str) -> String {
    format!(r#"{{"type": "minecraft:range_dispatch", {property}, "entries": []}}"#)
}

fn tint(source: &str) -> String {
    format!(
        r#"{{"type": "minecraft:model", "model": "minecraft:item/stick", "tints": [{source}]}}"#
    )
}

fn evaluate(stack: &ItemStack, model: &str) -> Option<Outcome> {
    let evaluator = Evaluator {
        stack: StackView {
            stack,
            table: &TABLE,
        },
        registries: None,
        grass_colormap: None,
        context: ItemModelContext::new(DisplayContext::Gui),
    };
    if model == "foil" {
        return Some(Outcome::Foil(evaluator.stack.has_foil()));
    }
    let model: UnbakedItemModel = serde_json::from_str(model).unwrap();
    let outcome = match &model {
        UnbakedItemModel::Condition { property, .. } => {
            Outcome::Condition(evaluator.condition(property))
        }
        UnbakedItemModel::Select { switch, .. } => Outcome::Select(evaluator.select(switch)),
        UnbakedItemModel::RangeDispatch { property, .. } => {
            Outcome::Range(evaluator.range(property))
        }
        UnbakedItemModel::Model { tints, .. } => Outcome::Tint(evaluator.tint(&tints[0])),
        _ => return None,
    };
    Some(outcome)
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the table holds one row per evaluated case"
)]
fn a_stack_view_evaluates_the_item_model_properties() {
    let damaged = condition(r#""property": "minecraft:damaged""#);
    let broken = condition(r#""property": "minecraft:broken""#);
    let has_lodestone = condition(
        r#""property": "minecraft:has_component", "component": "minecraft:lodestone_tracker""#,
    );
    let has_consumable =
        condition(r#""property": "minecraft:has_component", "component": "minecraft:consumable""#);
    let has_damage =
        condition(r#""property": "minecraft:has_component", "component": "minecraft:damage""#);
    let changed_damage = condition(
        r#""property": "minecraft:has_component", "component": "minecraft:damage", "ignore_default": true"#,
    );
    let model_flag = condition(r#""property": "minecraft:custom_model_data", "index": 1"#);
    let charge = select(
        r#""property": "minecraft:charge_type""#,
        &[r#""none""#, r#""arrow""#, r#""rocket""#],
    );
    let honey = select(
        r#""property": "minecraft:block_state", "block_state_property": "honey_level""#,
        &[r#""0""#, r#""5""#],
    );
    let model_string = select(
        r#""property": "minecraft:custom_model_data""#,
        &[r#""a""#, r#"["c", "b"]"#],
    );
    let context = select(
        r#""property": "minecraft:display_context""#,
        &[r#""fixed""#, r#""gui""#],
    );
    let stack_size = select(
        r#""property": "minecraft:component", "component": "minecraft:max_stack_size""#,
        &["16", "64"],
    );
    let consumable_select = select(
        r#""property": "minecraft:component", "component": "minecraft:consumable""#,
        &["{}"],
    );
    let damage = range(r#""property": "minecraft:damage""#);
    let raw_damage = range(r#""property": "minecraft:damage", "normalize": false"#);
    let count = range(r#""property": "minecraft:count""#);
    let model_float = range(r#""property": "minecraft:custom_model_data""#);
    let fullness = range(r#""property": "minecraft:bundle/fullness""#);
    let dye = tint(r#"{"type": "minecraft:dye", "default": -6265536}"#);
    let potion = tint(r#"{"type": "minecraft:potion", "default": -13083194}"#);
    let firework = tint(r#"{"type": "minecraft:firework", "default": -7697782}"#);
    let model_color = tint(r#"{"type": "minecraft:custom_model_data", "default": 255}"#);
    let consumable = || {
        PatchEntry::Undecoded(UndecodedValue {
            name: rl!("minecraft:consumable").to_buf(),
            bytes: Box::new([]),
        })
    };
    let honey_level = |value: &str| {
        BlockState::new([BlockStateProperty {
            name: "honey_level".to_owned(),
            value: value.to_owned(),
        }])
    };
    let inner_bundle = one(BUNDLE).set(BundleContents(vec![stack(ARROW, 32)]));
    let explosion = |colors: Vec<i32>| FireworkExplosion {
        shape: FireworkShape::SmallBall,
        colors,
        fade_colors: Vec::new(),
        has_trail: false,
        has_twinkle: false,
    };

    let cases: Vec<(&str, ItemStack, &str, Outcome)> = vec![
        (
            "a used sword is damaged",
            one(SWORD).set(Damage(10)),
            &damaged,
            Outcome::Condition(true),
        ),
        (
            "a new sword is not damaged",
            one(SWORD),
            &damaged,
            Outcome::Condition(false),
        ),
        (
            "an unbreakable sword is not damaged",
            one(SWORD).set(Damage(10)).set(Unbreakable),
            &damaged,
            Outcome::Condition(false),
        ),
        (
            "one use from breaking is broken",
            one(SWORD).set(Damage(249)),
            &broken,
            Outcome::Condition(true),
        ),
        (
            "two uses from breaking is not broken",
            one(SWORD).set(Damage(248)),
            &broken,
            Outcome::Condition(false),
        ),
        (
            "a patch value is present",
            lodestone(COMPASS),
            &has_lodestone,
            Outcome::Condition(true),
        ),
        (
            "an absent type is not present",
            one(COMPASS),
            &has_lodestone,
            Outcome::Condition(false),
        ),
        (
            "an undecoded entry is present",
            one(ARROW).with_entry(consumable()),
            &has_consumable,
            Outcome::Condition(true),
        ),
        (
            "a prototype value is present",
            one(SWORD),
            &has_damage,
            Outcome::Condition(true),
        ),
        (
            "a removed prototype value is absent",
            one(SWORD).unset::<Damage>(),
            &has_damage,
            Outcome::Condition(false),
        ),
        (
            "the prototype value is a default",
            one(SWORD).set(Damage(0)),
            &changed_damage,
            Outcome::Condition(false),
        ),
        (
            "a removed prototype value is not a changed value",
            one(SWORD).unset::<Damage>(),
            &changed_damage,
            Outcome::Condition(false),
        ),
        (
            "a typed component matches the case of its prototype value",
            one(ARROW),
            &stack_size,
            Outcome::Select(Some(1)),
        ),
        (
            "a typed component whose patch value is in no case selects nothing",
            one(ARROW).set(MaxStackSize(1)),
            &stack_size,
            Outcome::Select(None),
        ),
        (
            "an untyped component gives no value",
            one(ARROW).with_entry(consumable()),
            &consumable_select,
            Outcome::Select(None),
        ),
        (
            "another value is not a default",
            one(SWORD).set(Damage(3)),
            &changed_damage,
            Outcome::Condition(true),
        ),
        (
            "a custom model flag",
            one(ARROW).set(model_data()),
            &model_flag,
            Outcome::Condition(true),
        ),
        (
            "an empty crossbow",
            one(CROSSBOW),
            &charge,
            Outcome::Select(Some(0)),
        ),
        (
            "a crossbow with an arrow",
            one(CROSSBOW).set(ChargedProjectiles(vec![one(ARROW)])),
            &charge,
            Outcome::Select(Some(1)),
        ),
        (
            "a crossbow with a rocket",
            one(CROSSBOW).set(ChargedProjectiles(vec![one(ARROW), one(ROCKET)])),
            &charge,
            Outcome::Select(Some(2)),
        ),
        (
            "a block state case",
            one(BEE_NEST).set(honey_level("5")),
            &honey,
            Outcome::Select(Some(1)),
        ),
        (
            "a block state without a case",
            one(BEE_NEST).set(honey_level("3")),
            &honey,
            Outcome::Select(None),
        ),
        (
            "no block state",
            one(BEE_NEST),
            &honey,
            Outcome::Select(None),
        ),
        (
            "a custom model string",
            one(ARROW).set(model_data()),
            &model_string,
            Outcome::Select(Some(1)),
        ),
        (
            "the display context is the gui",
            one(ARROW),
            &context,
            Outcome::Select(Some(1)),
        ),
        (
            "normalized damage",
            one(SWORD).set(Damage(125)),
            &damage,
            Outcome::Range(0.5),
        ),
        (
            "damage above the maximum",
            one(SWORD).set(Damage(400)),
            &raw_damage,
            Outcome::Range(250.0),
        ),
        (
            "normalized count",
            stack(COMPASS, 16),
            &count,
            Outcome::Range(0.25),
        ),
        (
            "a custom model float",
            one(ARROW).set(model_data()),
            &model_float,
            Outcome::Range(0.75),
        ),
        (
            "an empty bundle",
            one(BUNDLE),
            &fullness,
            Outcome::Range(0.0),
        ),
        (
            "a bundle of arrows and a sword",
            one(BUNDLE).set(BundleContents(vec![stack(ARROW, 16), one(SWORD)])),
            &fullness,
            Outcome::Range(1.25),
        ),
        (
            "a bundle in a bundle",
            one(BUNDLE).set(BundleContents(vec![inner_bundle])),
            &fullness,
            Outcome::Range(0.5625),
        ),
        (
            "a dyed helmet",
            one(HELMET).set(DyedColor(ArgbColor(0x0012_3456))),
            &dye,
            Outcome::Tint(0xFF12_3456),
        ),
        (
            "an undyed helmet",
            one(HELMET),
            &dye,
            Outcome::Tint(0xFFA0_6540),
        ),
        (
            "a potion colour",
            one(ARROW).set(PotionContents {
                custom_color: Some(0x00FF00),
                ..PotionContents::default()
            }),
            &potion,
            Outcome::Tint(0xFF00_FF00),
        ),
        (
            "one firework colour",
            one(STAR).set(explosion(vec![0x0011_2233])),
            &firework,
            Outcome::Tint(0xFF11_2233),
        ),
        (
            "firework colours average",
            one(STAR).set(explosion(vec![0x00FF_0000, 0x0000_00FF])),
            &firework,
            Outcome::Tint(0xFF7F_007F),
        ),
        (
            "a custom model colour",
            one(ARROW).set(model_data()),
            &model_color,
            Outcome::Tint(0xFF12_3456),
        ),
        (
            "an enchanted stack glints",
            enchanted(SWORD),
            "foil",
            Outcome::Foil(true),
        ),
        (
            "the glint override wins",
            enchanted(SWORD).set(EnchantmentGlintOverride(false)),
            "foil",
            Outcome::Foil(false),
        ),
        (
            "a forced glint",
            one(ARROW).set(EnchantmentGlintOverride(true)),
            "foil",
            Outcome::Foil(true),
        ),
        (
            "a lodestone compass glints",
            lodestone(COMPASS),
            "foil",
            Outcome::Foil(true),
        ),
        (
            "a lodestone tracker glints only on a compass",
            lodestone(ARROW),
            "foil",
            Outcome::Foil(false),
        ),
        (
            "a plain stack does not glint",
            one(SWORD),
            "foil",
            Outcome::Foil(false),
        ),
    ];
    for (name, stack, model, expected) in cases {
        assert_eq!(evaluate(&stack, model), Some(expected), "{name}");
    }
}
