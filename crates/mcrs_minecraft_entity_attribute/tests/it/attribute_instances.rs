use mcrs_minecraft_entity_attribute::AttributeBases;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::AttributeMask;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_resource::ResourceLocationBuf;

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
