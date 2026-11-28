use mcrs_minecraft_item_stack::BeeOccupant;
use mcrs_minecraft_item_stack::Bees;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::CustomData;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EntityData;
use mcrs_minecraft_item_stack::EntityTypeId;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

#[derive(Clone, PartialEq, Hash)]
struct Owner(u8);

impl CustomComponent for Owner {
    const KEY: &'static str = "test:owner";

    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.0);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [owner] => Some(Self(*owner)),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Hash)]
struct Batch(u8);

impl CustomComponent for Batch {
    const KEY: &'static str = "test:batch";

    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.0);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [batch] => Some(Self(*batch)),
            _ => None,
        }
    }
}

fn compound(entries: &[(&str, NbtTag)]) -> NbtCompound {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn bee(data: NbtCompound) -> Bees {
    Bees(vec![BeeOccupant {
        entity_data: EntityData::new(EntityTypeId(4), data),
        ticks_in_hive: 0,
        min_ticks_in_hive: 0,
    }])
}

fn stack(kind: u16, count: u8) -> ItemStack {
    let count = Count::new(count).expect("the test counts are not zero");
    ItemStack::new(Kind(kind), count)
}

#[test]
fn stacks_are_equal_when_item_count_and_patch_entries_are_equal() {
    let plain = || stack(7, 1);
    let one = NbtTag::Int(1);
    let half = NbtTag::Float(0.5);
    let xy = NbtTag::Compound(compound(&[("x", one.clone()), ("y", half.clone())]));
    let yx = NbtTag::Compound(compound(&[("y", half), ("x", one.clone())]));
    let cases: [(&str, ItemStack, ItemStack, bool); 14] = [
        (
            "the order of two built-in values has no effect",
            plain().set(Damage(12)).set(Rarity::Epic),
            plain().set(Rarity::Epic).set(Damage(12)),
            true,
        ),
        (
            "the order of a value and a removal has no effect",
            plain().unset::<RepairCost>().set(Unbreakable),
            plain().set(Unbreakable).unset::<RepairCost>(),
            true,
        ),
        (
            "the order of built-in and custom values has no effect",
            plain().set(Owner(3)).set(Damage(1)).set(Batch(9)),
            plain().set(Batch(9)).set(Owner(3)).set(Damage(1)),
            true,
        ),
        (
            "the last value of a type replaces the earlier one",
            plain().set(Damage(1)).set(Damage(2)),
            plain().set(Damage(2)),
            true,
        ),
        (
            "a removal replaces the value of the type",
            plain().set(Damage(1)).unset::<Damage>(),
            plain().unset::<Damage>(),
            true,
        ),
        (
            "the removal of a custom component leaves no entry",
            plain().set(Owner(3)).unset::<Owner>(),
            plain(),
            true,
        ),
        (
            "the key order of custom data has no effect at any depth",
            plain().set(CustomData(compound(&[
                ("a", one.clone()),
                ("b", xy.clone()),
            ]))),
            plain().set(CustomData(compound(&[
                ("b", yx.clone()),
                ("a", one.clone()),
            ]))),
            true,
        ),
        (
            "the key order of bee entity data has no effect at any depth",
            plain().set(bee(compound(&[("a", one.clone()), ("b", xy)]))),
            plain().set(bee(compound(&[("b", yx), ("a", one)]))),
            true,
        ),
        (
            "different values of one type",
            plain().set(Damage(1)),
            plain().set(Damage(2)),
            false,
        ),
        (
            "different values of one custom type",
            plain().set(Owner(1)),
            plain().set(Owner(2)),
            false,
        ),
        (
            "a removal entry against no entry",
            plain().unset::<Damage>(),
            plain(),
            false,
        ),
        (
            "a removal entry against a value entry",
            plain().unset::<Damage>(),
            plain().set(Damage(0)),
            false,
        ),
        (
            "different items",
            stack(7, 1).set(Damage(1)),
            stack(8, 1).set(Damage(1)),
            false,
        ),
        (
            "different counts",
            stack(7, 1).set(Damage(1)),
            stack(7, 2).set(Damage(1)),
            false,
        ),
    ];
    for (name, left, right, equal) in cases {
        assert_eq!(left == right, equal, "{name}");
    }
}

#[test]
fn stacks_are_mergeable_when_item_and_patch_are_equal() {
    let cases: [(&str, ItemStack, ItemStack, bool); 4] = [
        (
            "different counts",
            stack(7, 1).set(Rarity::Rare).unset::<Damage>(),
            stack(7, 64).unset::<Damage>().set(Rarity::Rare),
            true,
        ),
        ("different items", stack(7, 1), stack(8, 1), false),
        (
            "different values of one type",
            stack(7, 1).set(Damage(1)),
            stack(7, 1).set(Damage(2)),
            false,
        ),
        (
            "a custom value against no entry",
            stack(7, 1).set(Owner(3)),
            stack(7, 1),
            false,
        ),
    ];
    for (name, left, right, mergeable) in cases {
        assert_eq!(left.mergeable(&right), mergeable, "{name}");
    }
}
