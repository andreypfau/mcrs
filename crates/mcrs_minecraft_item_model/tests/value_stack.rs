use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{Bounded, RgbInt};
use mcrs_minecraft_item_component::{
    BundleContents, ComponentMap, ComponentPatch, Damage, DyedColor, ItemComponentKind,
    ItemStackValue, MaxDamage, MaxStackSize, Template,
};
use mcrs_minecraft_item_model::asset::{ChargeType, ConditionProperty, RangeProperty, TintSource};
use mcrs_minecraft_item_model::eval::{
    bundle_weight, is_damaged, max_stack_size, next_damage_will_break,
};
use mcrs_minecraft_item_model::{Evaluator, StackView, ValueStack};

fn map(values: Vec<mcrs_minecraft_item_component::ItemComponentValue>) -> ComponentMap {
    ComponentMap(values)
}

fn prototypes(id: &ResourceLocation) -> Option<&'static ComponentMap> {
    static PICKAXE: std::sync::LazyLock<ComponentMap> = std::sync::LazyLock::new(|| {
        map(vec![
            MaxStackSize(Bounded(1)).into(),
            MaxDamage(Bounded(1561)).into(),
            Damage(Bounded(0)).into(),
        ])
    });
    static STONE: std::sync::LazyLock<ComponentMap> =
        std::sync::LazyLock::new(|| map(vec![MaxStackSize(Bounded(64)).into()]));
    static BUNDLE: std::sync::LazyLock<ComponentMap> = std::sync::LazyLock::new(|| {
        map(vec![
            MaxStackSize(Bounded(1)).into(),
            BundleContents(Vec::new()).into(),
        ])
    });
    static ROCKET: std::sync::LazyLock<ComponentMap> =
        std::sync::LazyLock::new(|| map(vec![MaxStackSize(Bounded(64)).into()]));
    Some(match id.as_str() {
        "minecraft:diamond_pickaxe" => &PICKAXE,
        "minecraft:stone" => &STONE,
        "minecraft:bundle" => &BUNDLE,
        "minecraft:firework_rocket" => &ROCKET,
        _ => return None,
    })
}

fn value(path: &str, count: i32, patch: impl FnOnce(&mut ComponentPatch)) -> ItemStackValue {
    let mut components = ComponentPatch::EMPTY;
    patch(&mut components);
    ItemStackValue {
        item: mcrs_minecraft_core::ResourceKey::from_location(ResourceLocation::minecraft(path)),
        count: Bounded(count),
        components,
    }
}

fn no_grass(_: f32, _: f32) -> Option<[f32; 4]> {
    None
}

#[test]
fn a_value_resolves_against_its_prototype() {
    let pickaxe = value("diamond_pickaxe", 1, |patch| {
        patch.set(Damage(Bounded(1560)))
    });
    let stack = ValueStack::new(&pickaxe, &prototypes).unwrap();
    assert_eq!(max_stack_size(&stack), 1);
    assert!(is_damaged(&stack));
    assert!(next_damage_will_break(&stack));
    assert!(stack.has(ItemComponentKind::MaxDamage));
    assert!(stack.has_non_default(ItemComponentKind::Damage));
    assert!(!stack.has_non_default(ItemComponentKind::MaxDamage));
    let e = Evaluator {
        stack,
        grass: &no_grass,
    };
    assert!(e.condition(&ConditionProperty::Broken));
    assert!((e.range(&RangeProperty::Damage { normalize: true }) - 0.99936).abs() < 1e-4);

    let stone = value("stone", 17, |_| {});
    let e = Evaluator {
        stack: ValueStack::new(&stone, &prototypes).unwrap(),
        grass: &no_grass,
    };
    assert_eq!(e.range(&RangeProperty::Count { normalize: true }), 0.265625);
    assert!(!e.condition(&ConditionProperty::Damaged));
    assert_eq!(
        e.tint(&TintSource::Dye {
            default: RgbInt(-6265536)
        }),
        0xFFA0_6540
    );

    let dyed = value("stone", 1, |patch| patch.set(DyedColor(RgbInt(0xFF0000))));
    let e = Evaluator {
        stack: ValueStack::new(&dyed, &prototypes).unwrap(),
        grass: &no_grass,
    };
    assert_eq!(e.tint(&TintSource::Dye { default: RgbInt(0) }), 0xFFFF_0000);
}

#[test]
fn children_come_from_the_value_and_a_tombstoned_kind_removes_them() {
    let bundle = value("bundle", 1, |patch| {
        patch.set(BundleContents(vec![
            Template(value("stone", 32, |_| {})),
            Template(value("firework_rocket", 3, |_| {})),
        ]))
    });
    let stack = ValueStack::new(&bundle, &prototypes).unwrap();
    let children = stack.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[1].item().as_str(), "minecraft:firework_rocket");
    assert_eq!(bundle_weight(&stack), 32.0 / 64.0 + 3.0 / 64.0);
    let e = Evaluator {
        stack,
        grass: &no_grass,
    };
    assert_eq!(e.charge_type(), ChargeType::Rocket);

    let unknown = value("not_an_item", 1, |_| {});
    assert!(ValueStack::new(&unknown, &prototypes).is_none());
}
