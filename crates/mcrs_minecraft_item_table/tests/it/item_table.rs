use std::sync::LazyLock;

use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_table::ItemTable;

const STONE: Kind = Kind(0);
const SWORD: Kind = Kind(1);
const STICK: Kind = Kind(2);

static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    ItemTable::leak([
        vec![
            Builtin::MaxStackSize(MaxStackSize(64)),
            Builtin::Rarity(Rarity::Common),
        ],
        vec![
            Builtin::MaxStackSize(MaxStackSize(1)),
            Builtin::MaxDamage(MaxDamage(250)),
            Builtin::Damage(Damage(0)),
        ],
        vec![Builtin::Rarity(Rarity::Rare)],
    ])
});

fn stack(kind: Kind) -> ItemStack {
    ItemStack::new(kind, Count::new(1).expect("one is a count"))
}

#[test]
fn the_effective_value_is_the_patch_entry_or_else_the_prototype() {
    let cases: [(&str, ItemStack, Option<Damage>); 5] = [
        ("no patch", stack(SWORD), Some(Damage(0))),
        (
            "an override",
            stack(SWORD).set(Damage(12)),
            Some(Damage(12)),
        ),
        ("a removal", stack(SWORD).unset::<Damage>(), None),
        ("a type the prototype lacks", stack(STICK), None),
        (
            "a value the prototype lacks",
            stack(STICK).set(Damage(3)),
            Some(Damage(3)),
        ),
    ];
    for (name, stack, expected) in cases {
        assert_eq!(
            TABLE.effective::<Damage>(&stack),
            expected.as_ref(),
            "{name}"
        );
    }
    assert_eq!(
        TABLE.effective::<RepairCost>(&stack(STICK)),
        Some(&RepairCost(0)),
        "a neutral type the prototype lacks"
    );
}

#[test]
fn an_entry_of_another_type_leaves_the_prototype_value() {
    let stack = stack(STONE).set(Damage(4)).unset::<Rarity>();
    assert_eq!(
        TABLE.effective::<MaxStackSize>(&stack),
        Some(&MaxStackSize(64))
    );
}

#[test]
fn a_kind_out_of_range_has_no_prototype_and_no_effective_value() {
    let kind = Kind(3);
    assert!(TABLE.prototype(kind).is_none(), "the table has three kinds");
    assert_eq!(TABLE.effective::<MaxStackSize>(&stack(kind)), None);
}

#[test]
fn a_prototype_value_is_neutral_only_when_it_equals_the_neutral_value() {
    let cases: [(Kind, bool); 3] = [(STONE, true), (SWORD, false), (STICK, false)];
    for (kind, expected) in cases {
        let prototype = TABLE.prototype(kind).expect("the kind is in the table");
        assert_eq!(
            prototype.get::<Rarity>() == Some(Rarity::NEUTRAL),
            expected,
            "{kind:?}"
        );
    }
}
