use mcrs_minecraft_registries::Diagnostic;

use crate::support::{
    Block, DamageType, Enchantment, Item, LootTable, META, Packs, damage, declarations, get, names,
    rl, tag, value,
};

fn startup() -> Packs {
    Packs::new().world(
        "a",
        &[
            ("data/t/damage_type/x.json", &damage("t:x")),
            ("data/t/damage_type/y.json", &damage("t:y")),
            ("data/t/tags/damage_type/t.json", r#"{"values":["t:x"]}"#),
            ("data/t/tags/item/i.json", r#"{"values":["stone"]}"#),
            (
                "data/t/enchantment/e.json",
                r##"{"supported_items":"#t:i","damage_types":"#t:t"}"##,
            ),
            ("data/t/loot_table/l.json", "{}"),
        ],
    )
}

fn reloaded(loot: &str) -> Packs {
    Packs::new().world(
        "a",
        &[
            ("data/t/damage_type/x.json", &damage("changed")),
            ("data/t/damage_type/y.json", "{"),
            ("data/t/damage_type/new.json", &damage("t:new")),
            ("data/t/tags/damage_type/t.json", r#"{"values":["t:y"]}"#),
            (
                "data/t/tags/damage_type/bad.json",
                r#"{"values":["t:nope"]}"#,
            ),
            (
                "data/t/tags/block/b.json",
                r#"{"values":["stone","t:nope"]}"#,
            ),
            ("data/t/tags/item/j.json", r#"{"values":["dirt"]}"#),
            ("data/t/loot_table/l2.json", loot),
        ],
    )
}

#[test]
fn a_reload_rereads_reloadable_entries_and_every_tag_but_keeps_the_world_stage() {
    let (before, _) = startup().load(&declarations()).unwrap();
    let old_item_tag = before.tags::<Item>().unwrap().by_name(&rl("t:i")).unwrap();
    let old_damage_tag = before
        .tags::<DamageType>()
        .unwrap()
        .by_name(&rl("t:t"))
        .unwrap();

    let (after, diagnostics) = reloaded("{}")
        .load_after(&declarations(), Some(&before))
        .unwrap();

    assert_eq!((before.generation(), after.generation()), (0, 1));
    assert!(std::ptr::eq(
        before.registry::<DamageType>().unwrap(),
        after.registry::<DamageType>().unwrap()
    ));
    assert_eq!(names::<DamageType>(&after), ["t:x", "t:y"]);
    assert_eq!(value::<DamageType>(&after, "t:x").message_id, "t:x");
    assert_eq!(names::<LootTable>(&after), ["t:l2"]);

    let damage_tags = after.tags::<DamageType>().unwrap();
    assert_eq!(damage_tags.by_name(&rl("t:t")), Some(old_damage_tag));
    assert_eq!(tag::<DamageType>(&after, "t:t"), Some(vec!["t:y".into()]));
    assert_eq!(tag::<DamageType>(&after, "t:bad"), None);
    assert_eq!(tag::<Block>(&after, "t:b"), None);
    let failed: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| match d {
            Diagnostic::TagFailed { tag, .. } => Some(tag.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(failed, ["t:b", "t:bad"]);

    let item_tags = after.tags::<Item>().unwrap();
    assert_eq!(item_tags.by_name(&rl("t:i")), None);
    let listed: Vec<String> = item_tags.iter().map(|(_, n)| n.to_string()).collect();
    assert_eq!(listed, ["t:j"]);
    let listed: Vec<String> = damage_tags.iter().map(|(_, n)| n.to_string()).collect();
    assert_eq!(listed, ["t:t"]);
    let stone = get::<Item>(&after, "stone");
    assert!(!item_tags.contains(old_item_tag, stone));
    let enchantment = value::<Enchantment>(&after, "t:e");
    let supported = enchantment.supported_items.as_ref().unwrap();
    assert!(!supported.contains(&item_tags, stone));
    assert_eq!(supported.ids(&item_tags).count(), 0);
    let before_items = before.tags::<Item>().unwrap();
    assert!(before_items.contains(old_item_tag, stone));

    let newer = item_tags.by_name(&rl("t:j")).unwrap();
    assert_eq!(before_items.members(newer).count(), 0);
    assert_eq!(before_items.name(newer), None);
    assert!(!before_items.contains(newer, get::<Item>(&before, "dirt")));

    let (restored, _) = startup().load_after(&declarations(), Some(&after)).unwrap();
    let restored_items = restored.tags::<Item>().unwrap();
    assert_eq!(restored_items.by_name(&rl("t:i")), Some(old_item_tag));
    let enchantment = value::<Enchantment>(&restored, "t:e");
    assert!(
        enchantment
            .supported_items
            .as_ref()
            .unwrap()
            .contains(&restored_items, stone)
    );
}

#[test]
fn a_reload_that_breaks_a_reloadable_entry_fails() {
    let (before, _) = startup().load(&declarations()).unwrap();
    let report = reloaded("{")
        .load_after(&declarations(), Some(&before))
        .unwrap_err();
    assert!(
        report.iter().any(
            |d| matches!(d, Diagnostic::Entry { registry, entry, .. } if *registry == rl("loot_table") && *entry == rl("t:l2"))
        ),
        "{report:?}"
    );
}

#[test]
fn an_entry_origin_follows_the_stack_of_its_stage_when_a_reload_shifts_the_stack() {
    let (before, _) = startup().load(&declarations()).unwrap();
    let packs = Packs::new()
        .feature("trade_rebalance", META.into())
        .world("a", &[("data/t/loot_table/l.json", "{}")])
        .world("b", &[("data/t/loot_table/l.json", "{}")]);
    let (after, _) = packs.load_after(&declarations(), Some(&before)).unwrap();
    let pack = after.origin(get::<DamageType>(&after, "t:x")).unwrap();
    assert_eq!((pack.id(), pack.known_pack()), ("file/a", None));
    let pack = after.origin(get::<LootTable>(&after, "t:l")).unwrap();
    assert_eq!(pack.id(), "file/b");
    let offered: Vec<&str> = after.known_packs().map(|k| k.id.as_str()).collect();
    assert_eq!(offered, ["core"]);
}
