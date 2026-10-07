use mcrs_minecraft_item::keys::DataComponentType;
use serde::Deserialize;

#[derive(Deserialize)]
struct Kinds {
    kinds: Vec<Kind>,
}

#[derive(Deserialize)]
struct Kind {
    id: String,
    wire_id: u16,
    transient: bool,
    unit: bool,
    ignore_swap_animation: bool,
    nested_stacks: bool,
}

#[test]
fn the_kind_table_matches_kinds_json() {
    let table: Kinds =
        serde_json::from_str(include_str!("../../fixtures/item/kinds.json")).unwrap();
    assert_eq!(table.kinds.len(), DataComponentType::ALL.len());
    for (kind, row) in DataComponentType::ALL.iter().zip(&table.kinds) {
        assert_eq!(kind.location().path(), row.id);
        assert_eq!(kind.id().number(), row.wire_id);
        assert_eq!(!kind.is_persistent(), row.transient, "{kind:?}");
        assert_eq!(kind.is_unit(), row.unit, "{kind:?}");
        assert_eq!(
            kind.ignores_swap_animation(),
            row.ignore_swap_animation,
            "{kind:?}"
        );
        assert_eq!(kind.is_nested(), row.nested_stacks, "{kind:?}");
        assert_eq!(DataComponentType::read(&row.id), Some(*kind));
        assert_eq!(DataComponentType::read(kind.as_static_str()), Some(*kind));
        assert_eq!(
            DataComponentType::from_protocol_id(row.wire_id),
            Some(*kind)
        );
    }
    assert_eq!(
        DataComponentType::from_protocol_id(DataComponentType::ALL.len() as u16),
        None
    );
    assert_eq!(DataComponentType::read("!custom_data"), None);
}
