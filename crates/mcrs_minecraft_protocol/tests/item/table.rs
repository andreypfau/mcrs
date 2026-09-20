use std::collections::BTreeMap;

use mcrs_minecraft_protocol::item::ItemComponentKind;
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

#[derive(Deserialize)]
struct Report {
    entries: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    protocol_id: u16,
}

#[test]
fn the_kind_table_matches_kinds_json() {
    let table: Kinds = serde_json::from_str(include_str!("../fixtures/item/kinds.json")).unwrap();
    assert_eq!(table.kinds.len(), ItemComponentKind::COUNT);
    for (kind, row) in ItemComponentKind::ALL.iter().zip(&table.kinds) {
        assert_eq!(kind.id().path(), row.id);
        assert_eq!(kind.wire_id(), row.wire_id);
        assert_eq!(!kind.is_persistent(), row.transient, "{kind}");
        assert_eq!(kind.is_unit(), row.unit, "{kind}");
        assert_eq!(
            kind.ignores_swap_animation(),
            row.ignore_swap_animation,
            "{kind}"
        );
        assert_eq!(kind.is_nested(), row.nested_stacks, "{kind}");
        assert_eq!(ItemComponentKind::from_id(&row.id), Some(*kind));
        assert_eq!(ItemComponentKind::from_id(kind.id().as_str()), Some(*kind));
        assert_eq!(ItemComponentKind::from_wire_id(row.wire_id), Some(*kind));
    }
    assert_eq!(
        ItemComponentKind::from_wire_id(ItemComponentKind::COUNT as u16),
        None
    );
    assert_eq!(ItemComponentKind::from_id("!custom_data"), None);
}

#[test]
fn wire_ids_are_the_data_component_type_protocol_ids() {
    let report: BTreeMap<String, Report> = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    let types = &report["minecraft:data_component_type"].entries;
    assert_eq!(types.len(), 122);
    for (id, entry) in types {
        let kind = ItemComponentKind::from_id(id).unwrap_or_else(|| panic!("{id} is not a kind"));
        assert_eq!(kind.wire_id(), entry.protocol_id, "{id}");
    }
}
