use std::num::NonZeroU8;

use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::ModifierOperation;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::DuplicateKey;
use mcrs_minecraft_item_table::ItemRegistry;
use mcrs_minecraft_item_table::RowHashing;
use mcrs_minecraft_resource::ResourceLocationBuf;

struct Case {
    key: BuiltinKey,
    left: PatchEntry,
    right: PatchEntry,
    equal: bool,
    left_neutral: bool,
}

fn case<T: PartialEq>(key: BuiltinKey, wrap: fn(T) -> Builtin, left: T, right: T) -> Case {
    Case {
        key,
        equal: left == right,
        left: PatchEntry::Value(wrap(left)),
        right: PatchEntry::Value(wrap(right)),
        left_neutral: false,
    }
}

fn neutral_case<T: Neutral>(key: BuiltinKey, wrap: fn(T) -> Builtin, right: T) -> Case {
    Case {
        left_neutral: true,
        ..case(key, wrap, T::NEUTRAL.clone(), right)
    }
}

fn location(text: &str) -> ResourceLocationBuf {
    ResourceLocationBuf::read(text).expect("a valid identifier")
}

fn food(saturation: f32) -> Food {
    Food::new(4, saturation).expect("the nutrition is not negative")
}

fn cases() -> Vec<Case> {
    let enchantment = EnchantmentLevel {
        enchantment: EnchantmentId(1),
        level: NonZeroU8::MIN,
    };
    let modifier = AttributeModifier::new(
        AttributeId(3),
        location("test:bonus"),
        1.0,
        ModifierOperation::AddValue,
    );
    let mut faster = Tool::new(Vec::new());
    faster.default_mining_speed = 4.0;
    vec![
        case(
            BuiltinKey::MaxStackSize,
            Builtin::MaxStackSize,
            MaxStackSize(64),
            MaxStackSize(16),
        ),
        case(
            BuiltinKey::MaxDamage,
            Builtin::MaxDamage,
            MaxDamage(250),
            MaxDamage(250),
        ),
        case(BuiltinKey::Damage, Builtin::Damage, Damage(0), Damage(3)),
        case(
            BuiltinKey::Unbreakable,
            Builtin::Unbreakable,
            Unbreakable,
            Unbreakable,
        ),
        case(
            BuiltinKey::CustomName,
            Builtin::CustomName,
            CustomName("a".into()),
            CustomName("b".into()),
        ),
        case(
            BuiltinKey::ItemName,
            Builtin::ItemName,
            ItemName("a".into()),
            ItemName("a".into()),
        ),
        case(
            BuiltinKey::ItemModel,
            Builtin::ItemModel,
            ItemModel::new(location("minecraft:stick")),
            ItemModel::new(location("minecraft:stone")),
        ),
        neutral_case(
            BuiltinKey::Lore,
            Builtin::Lore,
            Lore::new(vec!["line".into()]).expect("one line"),
        ),
        neutral_case(BuiltinKey::Rarity, Builtin::Rarity, Rarity::Epic),
        neutral_case(
            BuiltinKey::Enchantments,
            Builtin::Enchantments,
            Enchantments::new([enchantment]).expect("one enchantment"),
        ),
        neutral_case(
            BuiltinKey::AttributeModifiers,
            Builtin::AttributeModifiers,
            AttributeModifiers {
                modifiers: vec![modifier].into(),
            },
        ),
        neutral_case(
            BuiltinKey::TooltipDisplay,
            Builtin::TooltipDisplay,
            TooltipDisplay::hiding([ComponentType(1)]),
        ),
        neutral_case(BuiltinKey::RepairCost, Builtin::RepairCost, RepairCost(5)),
        case(
            BuiltinKey::Food,
            Builtin::Food,
            food(f32::NAN),
            food(-f32::NAN),
        ),
        case(
            BuiltinKey::Tool,
            Builtin::Tool,
            Tool::new(Vec::new()),
            faster,
        ),
    ]
}

#[test]
fn each_builtin_row_agrees_with_the_operations_of_its_type() {
    let registry = ItemRegistry::new();
    for case in cases() {
        let key = case.key;
        let row = registry.builtin(key).expect("each built-in type has a row");
        let removal = PatchEntry::Removal(key);
        assert!(
            row.equal(&case.left, &case.left.clone()),
            "{key:?}: a value equals its clone"
        );
        assert_eq!(
            row.equal(&case.left, &case.right),
            case.equal,
            "{key:?}: equality"
        );
        assert!(
            !row.equal(&case.left, &removal),
            "{key:?}: a removal is not a value"
        );
        let hash = row.hash(&case.left);
        assert!(hash.is_some(), "{key:?}: a value of the type has a hash");
        if case.equal {
            assert_eq!(
                hash,
                row.hash(&case.right),
                "{key:?}: equal values hash the same"
            );
        }
        assert_eq!(row.hash(&removal), None, "{key:?}: a removal has no hash");
        assert_eq!(
            row.clone_entry(&case.left),
            Some(case.left.clone()),
            "{key:?}: clone"
        );
        assert_eq!(
            row.is_neutral(&case.left),
            case.left_neutral,
            "{key:?}: neutral"
        );
        assert!(
            !row.is_neutral(&case.right),
            "{key:?}: the other value is not neutral"
        );
        assert_eq!(row.hashing(), RowHashing::Included, "{key:?}: row hashing");
    }
}

#[test]
fn a_builtin_row_reads_no_value_of_another_type() {
    let registry = ItemRegistry::new();
    let row = registry
        .builtin(BuiltinKey::Damage)
        .expect("damage has a row");
    let other = PatchEntry::Value(Builtin::RepairCost(RepairCost(0)));
    assert!(!row.equal(&other, &other), "a repair cost is not a damage");
    assert_eq!(row.clone_entry(&other), None);
}

#[derive(Clone, PartialEq, Hash)]
struct Soulbound(u64);

impl CustomComponent for Soulbound {
    const KEY: &'static str = "example:soulbound";

    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.extend(self.0.to_le_bytes());
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Self(u64::from_le_bytes(bytes.try_into().ok()?)))
    }
}

#[test]
fn a_custom_type_adds_one_row_for_its_key() {
    let mut registry = ItemRegistry::new();
    assert_eq!(registry.register::<Soulbound>(RowHashing::Excluded), Ok(()));
    assert_eq!(
        registry.register::<Soulbound>(RowHashing::Included),
        Err(DuplicateKey),
    );
    let row = registry
        .custom(Soulbound::KEY)
        .expect("the type is registered");
    assert_eq!(
        row.hashing(),
        RowHashing::Excluded,
        "the first registration stays"
    );
    let entry = |value: u64| {
        ItemStack::new(Kind(0), Count::new(1).expect("one is a count"))
            .set(Soulbound(value))
            .patch()
            .first()
            .cloned()
            .expect("the patch has the custom entry")
    };
    assert!(row.equal(&entry(7), &entry(7)), "equal values");
    assert!(!row.equal(&entry(7), &entry(8)), "different values");
    assert_eq!(row.clone_entry(&entry(7)), Some(entry(7)));
}
