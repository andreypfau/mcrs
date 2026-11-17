use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeBases, AttributeInstances, InstanceEdit, InstanceModifier, ModifierKind,
    ModifierOperation, SavedAttribute, SavedAttributes, SavedModifier, TypeBase,
};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocationBuf;

const BASES: AttributeBases = AttributeBases::new(
    &[Attribute::BlockInteractionRange, Attribute::Luck],
    &[TypeBase {
        attribute: Attribute::MovementSpeed,
        base: 0.10000000149011612,
    }],
);

fn modifier(
    id: &str,
    amount: f64,
    operation: ModifierOperation,
    kind: ModifierKind,
) -> InstanceEdit {
    InstanceEdit::Put(InstanceModifier::new(
        ResourceLocationBuf::read(id).unwrap(),
        amount,
        operation,
        kind,
    ))
}

fn permanent(id: &str, amount: f64) -> InstanceEdit {
    modifier(
        id,
        amount,
        ModifierOperation::AddValue,
        ModifierKind::Permanent,
    )
}

fn instances(edits: &[(Attribute, InstanceEdit)]) -> AttributeInstances {
    let mut instances = AttributeInstances::new(BASES);
    for (attribute, edit) in edits {
        let _ = instances.apply(*attribute, edit);
    }
    instances
}

fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(mut compound) => {
            compound
                .child_tags
                .sort_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut compound.child_tags {
                *child = sorted(child.clone());
            }
            NbtTag::Compound(compound)
        }
        NbtTag::List(list) => NbtTag::List(list.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn snbt(text: &str) -> NbtTag {
    mcrs_minecraft_snbt::parse_tag(text).unwrap()
}

#[test]
fn atr_sav_001_the_save_holds_changed_bases_and_permanent_modifiers_only() {
    use Attribute::*;
    use ModifierOperation::AddMultipliedTotal;
    let instances = instances(&[
        (MaxHealth, InstanceEdit::SetBase(5000.0)),
        (Luck, permanent("test:a", 1.0)),
        (
            BlockInteractionRange,
            modifier(
                "minecraft:creative_mode_block_range",
                0.5,
                ModifierOperation::AddValue,
                ModifierKind::Transient,
            ),
        ),
        (
            MovementSpeed,
            modifier(
                "minecraft:effect.speed",
                0.20000000298023224,
                AddMultipliedTotal,
                ModifierKind::Permanent,
            ),
        ),
        (
            MovementSpeed,
            modifier(
                "minecraft:sprinting",
                0.3,
                AddMultipliedTotal,
                ModifierKind::Transient,
            ),
        ),
        (
            MovementSpeed,
            modifier(
                "minecraft:effect.slowness",
                -0.45000001788139343,
                AddMultipliedTotal,
                ModifierKind::Permanent,
            ),
        ),
        (Scale, InstanceEdit::SetBase(2.0)),
        (Scale, InstanceEdit::SetBase(1.0)),
    ]);

    let saved =
        mcrs_minecraft_nbt::to_nbt_tag(&SavedAttributes::from_instances(&instances)).unwrap();

    let expected = snbt(
        r#"[
            {id: "minecraft:luck", modifiers: [{amount: 1.0d, id: "test:a", operation: "add_value"}], base: 0.0d},
            {id: "minecraft:max_health", base: 5000.0d},
            {id: "minecraft:movement_speed", modifiers: [
                {amount: 0.20000000298023224d, id: "minecraft:effect.speed", operation: "add_multiplied_total"},
                {amount: -0.45000001788139343d, id: "minecraft:effect.slowness", operation: "add_multiplied_total"}
            ], base: 0.10000000149011612d}
        ]"#,
    );
    assert_eq!(sorted(saved), sorted(expected));
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn atr_sav_002_a_load_leaves_out_each_bad_part_and_keeps_the_rest() {
    use Attribute::*;
    let cases: [(&str, &str, Vec<(Attribute, InstanceEdit)>); 18] = [
        ("a value that is not a list", r#""x""#, vec![]),
        (
            "an element that is not a compound",
            r#"[1, "x", [], {id: "minecraft:scale", base: 2.0d}]"#,
            vec![(Scale, InstanceEdit::SetBase(2.0))],
        ),
        (
            "an absent id, an id that is not a string, an unknown id and an attribute outside the set",
            r#"[{base: 3.0d}, {id: 5, base: 3.0d}, {id: "test:unknown", base: 1.0d}, {id: "minecraft:attack_damage", base: 7.0d}, {id: "minecraft:scale", base: 2.0d}]"#,
            vec![(Scale, InstanceEdit::SetBase(2.0))],
        ),
        (
            "an id without a namespace",
            r#"[{id: "scale", base: 2.0d, modifiers: [{id: "g", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![
                (Scale, InstanceEdit::SetBase(2.0)),
                (Scale, permanent("minecraft:g", 1.0)),
            ],
        ),
        (
            "a base that is not a number",
            r#"[{id: "minecraft:scale", base: "x", modifiers: [{id: "test:g", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![
                (Scale, InstanceEdit::SetBase(0.0)),
                (Scale, permanent("test:g", 1.0)),
            ],
        ),
        (
            "no base",
            r#"[{id: "minecraft:max_health"}]"#,
            vec![(MaxHealth, InstanceEdit::SetBase(0.0))],
        ),
        (
            "a base and an amount of other number types",
            r#"[{id: "minecraft:scale", base: 3, modifiers: [{id: "test:g", amount: 0.5f, operation: "add_value"}]}, {id: "minecraft:max_health", base: 4b}, {id: "minecraft:luck", base: 5L}]"#,
            vec![
                (Scale, InstanceEdit::SetBase(3.0)),
                (Scale, permanent("test:g", 0.5)),
                (MaxHealth, InstanceEdit::SetBase(4.0)),
                (Luck, InstanceEdit::SetBase(5.0)),
            ],
        ),
        (
            "modifiers that are not a list",
            r#"[{id: "minecraft:scale", base: 2.0d, modifiers: {id: "test:g", amount: 1.0d, operation: "add_value"}}]"#,
            vec![(Scale, InstanceEdit::SetBase(2.0))],
        ),
        (
            "a modifier that is not a compound",
            r#"[{id: "minecraft:scale", base: 2.0d, modifiers: [1, "x", {id: "test:g", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![
                (Scale, InstanceEdit::SetBase(2.0)),
                (Scale, permanent("test:g", 1.0)),
            ],
        ),
        (
            "a modifier without its id",
            r#"[{id: "minecraft:luck", base: 0.0d, modifiers: [{amount: 1.0d, operation: "add_value"}, {id: "test:a", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![(Luck, permanent("test:a", 1.0))],
        ),
        (
            "a modifier without its amount",
            r#"[{id: "minecraft:luck", base: 0.0d, modifiers: [{id: "test:x", operation: "add_value"}, {id: "test:a", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![(Luck, permanent("test:a", 1.0))],
        ),
        (
            "a modifier without its operation",
            r#"[{id: "minecraft:luck", base: 0.0d, modifiers: [{id: "test:x", amount: 1.0d}, {id: "test:a", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![(Luck, permanent("test:a", 1.0))],
        ),
        (
            "a modifier id that is not an identifier",
            r#"[{id: "minecraft:luck", base: 0.0d, modifiers: [{id: "Bad Id", amount: 1.0d, operation: "add_value"}, {id: "test:a", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![(Luck, permanent("test:a", 1.0))],
        ),
        (
            "an amount that is not a number",
            r#"[{id: "minecraft:luck", base: 0.0d, modifiers: [{id: "test:x", amount: "1", operation: "add_value"}, {id: "test:a", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![(Luck, permanent("test:a", 1.0))],
        ),
        (
            "a bad operation beside a good modifier",
            r#"[{id: "minecraft:scale", base: 2.0d, modifiers: [{id: "test:g", amount: 1.0d, operation: "add_value"}, {id: "test:m", amount: 1.0d, operation: "no_such_operation"}]}]"#,
            vec![
                (Scale, InstanceEdit::SetBase(2.0)),
                (Scale, permanent("test:g", 1.0)),
            ],
        ),
        (
            "a numeric operation",
            r#"[{id: "minecraft:luck", base: 0.0d, modifiers: [{id: "test:x", amount: 1.0d, operation: 0}, {id: "test:a", amount: 1.0d, operation: "add_value"}]}]"#,
            vec![(Luck, permanent("test:a", 1.0))],
        ),
        (
            "two modifiers of one id",
            r#"[{id: "minecraft:scale", base: 1.0d, modifiers: [{id: "test:m", amount: 1.0d, operation: "add_value"}, {id: "test:b", amount: 1.0d, operation: "add_value"}, {id: "test:m", amount: 2.0d, operation: "add_value"}]}]"#,
            vec![
                (Scale, permanent("test:b", 1.0)),
                (Scale, permanent("test:m", 2.0)),
            ],
        ),
        (
            "two compounds of one id",
            r#"[{id: "minecraft:scale", base: 2.0d, modifiers: [{id: "test:a", amount: 1.0d, operation: "add_value"}]}, {id: "minecraft:scale", base: 3.0d, modifiers: [{id: "test:b", amount: 1.0d, operation: "add_value"}, {id: "test:a", amount: 2.0d, operation: "add_value"}]}]"#,
            vec![
                (Scale, InstanceEdit::SetBase(3.0)),
                (Scale, permanent("test:b", 1.0)),
                (Scale, permanent("test:a", 2.0)),
            ],
        ),
    ];
    for (name, input, edits) in cases {
        let saved: SavedAttributes = mcrs_minecraft_nbt::from_tag(snbt(input)).unwrap();
        assert_eq!(saved.into_instances(BASES), instances(&edits), "{name}");
    }
}

#[test]
fn atr_sav_002_an_unknown_key_fails_the_load_naming_the_key() {
    let inputs = [
        r#"[{id: "minecraft:scale", base: 2.0d, Surprise: 1b}]"#,
        r#"[{id: "test:unknown", modifiers: [{id: "test:a", amount: 1.0d, operation: "add_value", Surprise: 1b}]}]"#,
    ];
    for input in inputs {
        let error = mcrs_minecraft_nbt::from_tag::<SavedAttributes>(snbt(input)).unwrap_err();
        assert!(error.to_string().contains("Surprise"), "{input}: {error}");
    }
}

#[test]
fn a_save_and_a_load_keep_the_permanent_state_and_the_bits_of_a_base() {
    use Attribute::*;
    let changed = instances(&[
        (Luck, InstanceEdit::SetBase(-0.0)),
        (Armor, permanent("test:a", 2.0)),
        (Armor, permanent("test:b", 3.0)),
        (
            Armor,
            modifier(
                "test:t",
                1.0,
                ModifierOperation::AddValue,
                ModifierKind::Transient,
            ),
        ),
    ]);

    let loaded = SavedAttributes::from_instances(&changed).into_instances(BASES);

    let without_transient = instances(&[
        (Luck, InstanceEdit::SetBase(-0.0)),
        (Armor, permanent("test:a", 2.0)),
        (Armor, permanent("test:b", 3.0)),
    ]);
    assert_eq!(loaded, without_transient);
    assert_eq!(
        loaded.base(Luck).map(f64::to_bits),
        Some((-0.0f64).to_bits())
    );
}

fn saved(base: f64, amount: f64) -> SavedAttributes {
    SavedAttributes {
        attributes: vec![SavedAttribute {
            id: Attribute::MaxHealth,
            base,
            modifiers: vec![SavedModifier {
                id: ResourceLocationBuf::read("minecraft:test").unwrap(),
                amount,
                operation: ModifierOperation::AddValue,
            }],
        }],
    }
}

#[test]
fn the_saved_base_and_amount_compare_by_bits() {
    let cases = [
        (saved(-0.0, 1.0), saved(0.0, 1.0), false),
        (saved(f64::NAN, 1.0), saved(f64::NAN, 1.0), true),
        (saved(1.0, -0.0), saved(1.0, 0.0), false),
        (saved(1.0, f64::NAN), saved(1.0, f64::NAN), true),
    ];
    for (left, right, equal) in cases {
        assert_eq!(left == right, equal, "{left:?} == {right:?}");
    }
}
