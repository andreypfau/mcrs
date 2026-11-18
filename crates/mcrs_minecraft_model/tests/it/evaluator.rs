use std::sync::LazyLock;

use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_item::RgbInt;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item_stack::ArgbColor;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::DyedColor;
use mcrs_minecraft_item_stack::FireworkExplosion;
use mcrs_minecraft_item_stack::FireworkShape;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_model::colormap::load_colormap;
use mcrs_minecraft_model::item_model_asset::{
    ConditionProperty, DisplayContext, RangeProperty, TintSource,
};
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::item_model_eval::Evaluator;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_resource::ResourceLocation;

use crate::client_pack::client_pack;

static TABLE: LazyLock<ItemTable> = LazyLock::new(test_item_table);

static GRASS: LazyLock<Vec<u8>> = LazyLock::new(|| load_colormap(client_pack(), "grass").unwrap());

fn stack(path: &str, count: u8) -> ItemStack {
    let name = format!("minecraft:{path}");
    let kind = Kind(
        TABLE
            .id(NameTable::Item, ResourceLocation::read(&name).unwrap())
            .unwrap(),
    );
    ItemStack::new(kind, Count::new(count).unwrap())
}

fn eval(stack: &ItemStack) -> Evaluator<'_> {
    Evaluator {
        stack: StackView {
            stack,
            table: &TABLE,
        },
        registries: None,
        grass_colormap: Some(&GRASS),
        context: ItemModelContext::new(DisplayContext::Gui),
    }
}

#[test]
fn corpus_stacks_evaluate_against_the_item_table() {
    damage_and_count_ranges_follow_the_stack();
    has_component_distinguishes_prototype_and_patch();
    tints_follow_vanilla_colour_rules();
}

fn damage_and_count_ranges_follow_the_stack() {
    let pick = stack("diamond_pickaxe", 1).set(Damage(100));
    let e = eval(&pick);
    assert!((e.range(&RangeProperty::Damage { normalize: true }) - 0.0640615).abs() < 1e-6);
    assert_eq!(e.range(&RangeProperty::Damage { normalize: false }), 100.0);
    assert!(e.condition(&ConditionProperty::Damaged));
    assert!(!e.condition(&ConditionProperty::Broken));
    let pick = stack("diamond_pickaxe", 1).set(Damage(1560));
    assert!(eval(&pick).condition(&ConditionProperty::Broken));
    let stone = stack("stone", 17);
    let e = eval(&stone);
    assert_eq!(e.range(&RangeProperty::Count { normalize: true }), 0.265625);
    assert!(!e.condition(&ConditionProperty::Damaged));
}

fn has_component_distinguishes_prototype_and_patch() {
    let max_stack = |ignore_default| ConditionProperty::HasComponent {
        component: DataComponentType::MaxStackSize,
        ignore_default,
    };
    let dyed = |ignore_default| ConditionProperty::HasComponent {
        component: DataComponentType::DyedColor,
        ignore_default,
    };
    let stone = stack("stone", 1);
    let e = eval(&stone);
    assert!(e.condition(&max_stack(false)));
    assert!(!e.condition(&max_stack(true)));
    assert!(!e.condition(&dyed(false)));
    let stone = stack("stone", 1)
        .set(DyedColor(ArgbColor(0xFF0000)))
        .unset::<MaxStackSize>();
    let e = eval(&stone);
    assert!(e.condition(&dyed(false)));
    assert!(e.condition(&dyed(true)));
    assert!(!e.condition(&max_stack(false)));
}

fn tints_follow_vanilla_colour_rules() {
    let star = stack("firework_star", 1).set(FireworkExplosion {
        shape: FireworkShape::SmallBall,
        colors: vec![0x112233, 0x445566, 0x778899],
        fade_colors: Vec::new(),
        has_trail: false,
        has_twinkle: false,
    });
    let plain = stack("firework_star", 1);
    let firework = TintSource::Firework {
        default: RgbInt(-7697782),
    };
    let potion = stack("potion", 1);
    let dyed = stack("leather_chestplate", 1).set(DyedColor(ArgbColor(0x123456)));
    assert_eq!(eval(&star).tint(&firework), 0xFF44_5566);
    assert_eq!(eval(&plain).tint(&firework), (-7697782i32) as u32);
    assert_eq!(
        eval(&dyed).tint(&TintSource::Dye { default: RgbInt(0) }),
        0xFF12_3456
    );
    let e = eval(&potion);
    assert_eq!(
        e.tint(&TintSource::Potion {
            default: RgbInt(-13083194)
        }),
        (-13083194i32) as u32
    );
    assert_eq!(
        e.tint(&TintSource::Constant {
            value: RgbInt(0xFF0000)
        }),
        0xFFFF_0000
    );
    assert_eq!(
        e.tint(&TintSource::Dye {
            default: RgbInt(0x123456)
        }),
        0x0012_3456
    );
    let grass = e.tint(&TintSource::Grass {
        temperature: 0.5,
        downfall: 1.0,
    });
    assert_eq!(grass, 0xFF7C_BD6B, "{grass:#x}");
}
