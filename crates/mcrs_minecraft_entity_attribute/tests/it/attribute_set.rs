use mcrs_minecraft_entity_attribute::AttributeBases;
use mcrs_minecraft_entity_attribute::AttributeError;
use mcrs_minecraft_entity_attribute::AttributeSet;
use mcrs_minecraft_entity_attribute::InstanceChange;
use mcrs_minecraft_entity_attribute::InstanceModifier;
use mcrs_minecraft_entity_attribute::ModifierKind;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

#[derive(Clone, Copy)]
enum Call {
    SetBase(f64),
    Add(&'static str, f64, ModifierOperation),
    Set(&'static str, f64, ModifierOperation),
    Remove(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    Added,
    Change(InstanceChange),
    Refused(AttributeError),
}

struct Case {
    name: &'static str,
    attribute: Attribute,
    calls: &'static [(Call, Outcome)],
    base: Option<f64>,
    value: Option<f64>,
    sequence: &'static [(&'static str, f64)],
    entries: usize,
}

use Call::*;
use InstanceChange::*;
use ModifierOperation::*;
use Outcome::*;

const BASES: AttributeBases = AttributeBases::new(&[Attribute::Luck], &[]);

const CASES: &[Case] = &[
    Case {
        name: "attribute_not_in_set",
        attribute: Attribute::FollowRange,
        calls: &[
            (SetBase(16.0), Refused(AttributeError::NotInSet)),
            (
                Add("test:a", 1.0, AddValue),
                Refused(AttributeError::NotInSet),
            ),
        ],
        base: None,
        value: None,
        sequence: &[],
        entries: 0,
    },
    Case {
        name: "base_above_maximum",
        attribute: Attribute::MaxHealth,
        calls: &[
            (SetBase(5000.0), Change(Changed)),
            (SetBase(5000.0), Change(Unchanged)),
        ],
        base: Some(5000.0),
        value: Some(1024.0),
        sequence: &[],
        entries: 1,
    },
    Case {
        name: "base_above_maximum_then_add_value",
        attribute: Attribute::MaxHealth,
        calls: &[
            (SetBase(5000.0), Change(Changed)),
            (Add("test:a", -4990.0, AddValue), Added),
        ],
        base: Some(5000.0),
        value: Some(10.0),
        sequence: &[("test:a", -4990.0)],
        entries: 1,
    },
    Case {
        name: "base_restored_leaves_the_set",
        attribute: Attribute::MaxHealth,
        calls: &[
            (SetBase(5000.0), Change(Changed)),
            (SetBase(20.0), Change(Changed)),
        ],
        base: Some(20.0),
        value: Some(20.0),
        sequence: &[],
        entries: 0,
    },
    Case {
        name: "base_out_of_range",
        attribute: Attribute::Scale,
        calls: &[(SetBase(100.0), Change(Changed))],
        base: Some(100.0),
        value: Some(16.0),
        sequence: &[],
        entries: 1,
    },
    Case {
        name: "modifier_add_again",
        attribute: Attribute::Luck,
        calls: &[
            (Add("test:a", 1.0, AddValue), Added),
            (
                Add("test:a", 2.0, AddMultipliedTotal),
                Refused(AttributeError::ModifierPresent),
            ),
        ],
        base: Some(0.0),
        value: Some(1.0),
        sequence: &[("test:a", 1.0)],
        entries: 1,
    },
    Case {
        name: "modifier_remove_again_leaves_the_set",
        attribute: Attribute::Luck,
        calls: &[
            (Add("test:a", 1.0, AddValue), Added),
            (Remove("test:a"), Change(Changed)),
            (Remove("test:a"), Change(Unchanged)),
        ],
        base: Some(0.0),
        value: Some(0.0),
        sequence: &[],
        entries: 0,
    },
    Case {
        name: "set_of_an_equal_modifier_keeps_its_place",
        attribute: Attribute::Luck,
        calls: &[
            (Set("test:a", 1.0, AddValue), Change(Changed)),
            (Set("test:b", 2.0, AddValue), Change(Changed)),
            (Set("test:a", 1.0, AddValue), Change(Unchanged)),
        ],
        base: Some(0.0),
        value: Some(3.0),
        sequence: &[("test:a", 1.0), ("test:b", 2.0)],
        entries: 1,
    },
    Case {
        name: "set_of_a_changed_modifier_moves_it_to_the_end",
        attribute: Attribute::Luck,
        calls: &[
            (Set("test:a", 1.0, AddValue), Change(Changed)),
            (Set("test:b", 2.0, AddValue), Change(Changed)),
            (Set("test:a", 4.0, AddValue), Change(Changed)),
        ],
        base: Some(0.0),
        value: Some(6.0),
        sequence: &[("test:b", 2.0), ("test:a", 4.0)],
        entries: 1,
    },
];

#[test]
fn each_call_changes_the_instance_as_the_rules_state() {
    let modifier = |id: &str, amount: f64, operation: ModifierOperation| {
        InstanceModifier::new(
            ResourceLocationBuf::read(id).unwrap(),
            amount,
            operation,
            ModifierKind::Permanent,
        )
    };
    for case in CASES {
        let name = case.name;
        let bases = BASES;
        let mut set = AttributeSet::new();
        for (call, expected) in case.calls {
            let outcome = match *call {
                SetBase(base) => set.set_base(bases, case.attribute, base).map(Change),
                Add(id, amount, operation) => set
                    .add_modifier(bases, case.attribute, modifier(id, amount, operation))
                    .map(|()| Added),
                Set(id, amount, operation) => set
                    .set_modifier(bases, case.attribute, modifier(id, amount, operation))
                    .map(Change),
                Remove(id) => set
                    .remove_modifier(bases, case.attribute, ResourceLocation::read(id).unwrap())
                    .map(Change),
            };
            assert_eq!(outcome.unwrap_or_else(Refused), *expected, "{name}");
        }
        assert_eq!(set.base(bases, case.attribute), case.base, "{name} base");
        assert_eq!(set.value(bases, case.attribute), case.value, "{name} value");
        let sequence: Vec<(String, f64)> = set
            .instances()
            .filter(|instance| instance.attribute() == case.attribute)
            .flat_map(|instance| instance.modifiers())
            .map(|modifier| (modifier.id().to_string(), modifier.amount()))
            .collect();
        let expected: Vec<(String, f64)> = case
            .sequence
            .iter()
            .map(|(id, amount)| ((*id).to_owned(), *amount))
            .collect();
        assert_eq!(sequence, expected, "{name} sequence");
        assert_eq!(set.instances().count(), case.entries, "{name} entries");
    }
}
