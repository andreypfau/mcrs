use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeBases, AttributeId, AttributeInstances, AttributeMask, AttributeModifier,
    AttributeSnapshot, InstanceChange, InstanceEdit, InstanceModifier, ModifierKind,
    ModifierOperation, UnknownAttribute,
};
use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf, rl};

struct Case {
    name: &'static str,
    edits: Vec<(Attribute, InstanceEdit)>,
    dirty: &'static [Attribute],
}

#[test]
fn an_attribute_is_dirty_after_a_change_until_the_take() {
    use Attribute::*;
    let absent = || InstanceEdit::Remove(ResourceLocationBuf::read("test:absent").unwrap());
    let cases = [
        Case {
            name: "change",
            edits: vec![(MaxHealth, InstanceEdit::SetBase(30.0))],
            dirty: &[MaxHealth],
        },
        Case {
            name: "unchanged_base_and_absent_modifier",
            edits: vec![(MaxHealth, InstanceEdit::SetBase(20.0)), (Armor, absent())],
            dirty: &[],
        },
        Case {
            name: "outside_the_set",
            edits: vec![(FollowRange, InstanceEdit::SetBase(16.0))],
            dirty: &[],
        },
        Case {
            name: "two_attributes",
            edits: vec![
                (Scale, InstanceEdit::SetBase(2.0)),
                (Armor, InstanceEdit::SetBase(3.0)),
            ],
            dirty: &[Armor, Scale],
        },
        Case {
            name: "change_then_restore",
            edits: vec![
                (MaxHealth, InstanceEdit::SetBase(30.0)),
                (MaxHealth, InstanceEdit::SetBase(20.0)),
            ],
            dirty: &[MaxHealth],
        },
    ];
    for case in cases {
        let name = case.name;
        let mut instances = AttributeInstances::new(AttributeBases::new(&[], &[]));
        for (attribute, edit) in &case.edits {
            let _ = instances.apply(*attribute, edit);
        }
        let dirty: Vec<Attribute> = instances.take_dirty().iter().collect();
        assert_eq!(dirty, case.dirty, "{name}");
        assert_eq!(
            instances.take_dirty(),
            AttributeMask::EMPTY,
            "{name} after the take"
        );
    }
}

const BASES: AttributeBases = AttributeBases::new(&[], &[]);

fn modifier(
    id: ResourceLocation<'static>,
    amount: f64,
    operation: ModifierOperation,
) -> AttributeModifier<'static> {
    AttributeModifier {
        id,
        amount,
        operation,
    }
}

fn entry(
    attribute: Attribute,
    base: f64,
    modifiers: Vec<AttributeModifier<'static>>,
) -> AttributeSnapshot<'static> {
    AttributeSnapshot {
        attribute: attribute.into(),
        base,
        modifiers,
    }
}

fn modifier_ids(instances: &AttributeInstances, attribute: Attribute) -> Vec<String> {
    instances
        .set()
        .instance(attribute)
        .map(|instance| {
            instance
                .modifiers()
                .iter()
                .map(|modifier| modifier.id().to_string())
                .collect()
        })
        .unwrap_or_default()
}

struct ReceiveCase {
    name: &'static str,
    before: Vec<(Attribute, InstanceEdit)>,
    entry: AttributeSnapshot<'static>,
    value: Option<f64>,
    modifiers: &'static [&'static str],
    change: InstanceChange,
}

fn receive_cases() -> [ReceiveCase; 5] {
    use Attribute::*;
    use ModifierOperation::*;
    let ordered = || {
        entry(
            MovementSpeed,
            0.5,
            vec![
                modifier(rl!("test:b"), 1.0, AddMultipliedTotal),
                modifier(rl!("test:a"), 0.25, AddValue),
                modifier(rl!("test:b"), 100.0, AddValue),
            ],
        )
    };
    let put = |kind: ModifierKind| {
        InstanceEdit::Put(InstanceModifier::new(
            rl!("test:a").to_buf(),
            0.5,
            AddValue,
            kind,
        ))
    };
    [
        ReceiveCase {
            name: "order_of_the_entry_and_first_of_one_identifier",
            before: Vec::new(),
            entry: ordered(),
            value: Some(1.5),
            modifiers: &["test:b", "test:a"],
            change: InstanceChange::Changed,
        },
        ReceiveCase {
            name: "replaces_the_modifiers",
            before: vec![
                (MovementSpeed, put(ModifierKind::Transient)),
                (MovementSpeed, InstanceEdit::SetBase(1.0)),
            ],
            entry: entry(
                MovementSpeed,
                0.25,
                vec![modifier(rl!("test:c"), 0.5, AddValue)],
            ),
            value: Some(0.75),
            modifiers: &["test:c"],
            change: InstanceChange::Changed,
        },
        ReceiveCase {
            name: "the_same_transient_modifier",
            before: vec![
                (MovementSpeed, InstanceEdit::SetBase(0.25)),
                (MovementSpeed, put(ModifierKind::Transient)),
            ],
            entry: entry(
                MovementSpeed,
                0.25,
                vec![modifier(rl!("test:a"), 0.5, AddValue)],
            ),
            value: Some(0.75),
            modifiers: &["test:a"],
            change: InstanceChange::Unchanged,
        },
        ReceiveCase {
            name: "a_permanent_modifier_becomes_transient",
            before: vec![
                (MovementSpeed, InstanceEdit::SetBase(0.25)),
                (MovementSpeed, put(ModifierKind::Permanent)),
            ],
            entry: entry(
                MovementSpeed,
                0.25,
                vec![modifier(rl!("test:a"), 0.5, AddValue)],
            ),
            value: Some(0.75),
            modifiers: &["test:a"],
            change: InstanceChange::Changed,
        },
        ReceiveCase {
            name: "outside_the_set_of_the_type",
            before: Vec::new(),
            entry: entry(BlockInteractionRange, 9.0, Vec::new()),
            value: None,
            modifiers: &[],
            change: InstanceChange::Unchanged,
        },
    ]
}

#[test]
fn an_entry_replaces_the_base_and_the_modifiers_in_its_order() {
    for case in receive_cases() {
        let name = case.name;
        let mut instances = AttributeInstances::new(BASES);
        for (attribute, edit) in &case.before {
            let _ = instances.apply(*attribute, edit);
        }
        let _ = instances.take_dirty();
        let attribute = case.entry.attribute.resolve().unwrap();
        assert_eq!(instances.receive(&case.entry), Ok(case.change), "{name}");
        assert_eq!(instances.value(attribute), case.value, "{name}");
        assert_eq!(
            modifier_ids(&instances, attribute),
            case.modifiers,
            "{name}"
        );
        let dirty: Vec<Attribute> = instances.take_dirty().iter().collect();
        let expected: &[Attribute] = match case.change {
            InstanceChange::Changed => &[attribute],
            InstanceChange::Unchanged => &[],
        };
        assert_eq!(dirty, expected, "{name}");
        assert_eq!(
            instances.receive(&case.entry),
            Ok(InstanceChange::Unchanged),
            "{name} again"
        );
    }
}

#[test]
fn an_unknown_attribute_id_is_an_error_and_changes_nothing() {
    let id = AttributeId(i32::try_from(Attribute::ALL.len()).unwrap());
    let mut instances = AttributeInstances::new(BASES);
    let unknown = AttributeSnapshot {
        attribute: id,
        base: 5.0,
        modifiers: vec![modifier(rl!("test:a"), 1.0, ModifierOperation::AddValue)],
    };
    assert_eq!(instances.receive(&unknown), Err(UnknownAttribute(id)));
    assert_eq!(instances, AttributeInstances::new(BASES));
    assert_eq!(instances.take_dirty(), AttributeMask::EMPTY);
}

#[test]
fn the_snapshot_of_each_instance_rebuilds_an_equal_value() {
    use ModifierOperation::*;
    let mut instances = AttributeInstances::new(BASES);
    let transient = |id: &str, amount: f64, operation: ModifierOperation| {
        InstanceEdit::Put(InstanceModifier::new(
            ResourceLocationBuf::read(id).unwrap(),
            amount,
            operation,
            ModifierKind::Transient,
        ))
    };
    for (attribute, edit) in [
        (Attribute::MaxHealth, InstanceEdit::SetBase(30.0)),
        (
            Attribute::MovementSpeed,
            transient("test:z", 0.5, AddMultipliedBase),
        ),
        (
            Attribute::MovementSpeed,
            transient("test:a", -0.25, AddValue),
        ),
        (Attribute::Armor, InstanceEdit::SetBase(-1.0)),
        (
            Attribute::Armor,
            transient("test:m", 2.0, AddMultipliedTotal),
        ),
    ] {
        let _ = instances.apply(attribute, &edit);
    }
    let mut rebuilt = AttributeInstances::new(BASES);
    for attribute in BASES.attributes() {
        let snapshot = instances.snapshot(attribute).unwrap();
        let _ = rebuilt.receive(&snapshot).unwrap();
    }
    assert_eq!(rebuilt, instances);
    assert_eq!(instances.snapshot(Attribute::BlockInteractionRange), None);
}

#[test]
fn clearing_the_modifiers_keeps_the_bases_and_marks_what_lost_one() {
    let mut instances = AttributeInstances::new(BASES);
    let _ = instances.receive(&entry(Attribute::MaxHealth, 30.0, Vec::new()));
    let _ = instances.receive(&entry(
        Attribute::Armor,
        2.0,
        vec![modifier(rl!("test:a"), 1.0, ModifierOperation::AddValue)],
    ));
    let _ = instances.take_dirty();
    assert_eq!(instances.clear_modifiers(), InstanceChange::Changed);
    assert_eq!(instances.value(Attribute::MaxHealth), Some(30.0));
    assert_eq!(instances.value(Attribute::Armor), Some(2.0));
    let dirty: Vec<Attribute> = instances.take_dirty().iter().collect();
    assert_eq!(dirty, [Attribute::Armor]);
    assert_eq!(instances.clear_modifiers(), InstanceChange::Unchanged);
}
