use mcrs_minecraft_core::TagKey;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use mcrs_minecraft_core::Id;
use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_registries::{Declaration, Diagnostic, Missing, Referrer};
use serde::Deserialize;

use crate::support::{
    Block, Packs, VillagerTrade, assets, damage, declarations, get, report_errors, rl, tag,
};

fn block_tags(files: &[(&str, &str)]) -> Packs {
    let files: Vec<(String, &str)> = files
        .iter()
        .map(|(name, body)| {
            let (namespace, path) = name.split_once(':').unwrap_or(("t", name));
            (format!("data/{namespace}/tags/block/{path}.json"), *body)
        })
        .collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
    Packs::new().world("a", &files)
}

fn block_tag_files(count: u32) -> Packs {
    let mut packs = Packs::new().world("a", &[]);
    for n in 0..count {
        packs.files_mut().insert(
            "file/a",
            format!("data/t/tags/block/e{n:05}.json"),
            r#"{"values":["stone"]}"#,
        );
    }
    packs
}

#[test]
fn a_registry_past_the_tag_range_fails_to_load() {
    assert_eq!(
        report_errors(&block_tag_files(65_537).load(declarations())),
        [Diagnostic::TooManyTags {
            registry: rl("block"),
            count: 65_537,
        }]
    );
}

mod exhaustive {
    use super::*;

    #[test]
    fn a_registry_at_the_end_of_the_tag_range_loads() {
        let (registries, _) = block_tag_files(65_536).load(declarations()).unwrap();
        let tags = registries.tags::<Block>().unwrap();
        let last = tags.id(&TagKey::from_location(rl("t:e65535"))).unwrap();
        assert_eq!(last.number(), u16::MAX);
        assert_eq!(
            tags.members(last).collect::<Vec<_>>(),
            [get::<Block>(&registries, "stone")]
        );
    }
}

#[test]
fn tag_members_resolve_in_order_without_duplicates() {
    type Files<'a> = &'a [(&'a str, &'a str)];
    let cases: [(Files<'_>, &str, Option<&[&str]>); 9] = [
        (
            &[
                ("x", r##"{"values":["c","#t:y","a"]}"##),
                ("y", r#"{"values":["b","c","d"]}"#),
            ],
            "t:x",
            Some(&["minecraft:c", "minecraft:b", "minecraft:d", "minecraft:a"]),
        ),
        (
            &[(
                "x",
                r#"{"values":["stone",{"id":"dirt","zzz":1},"stone"],"zzz":2}"#,
            )],
            "t:x",
            Some(&["minecraft:stone", "minecraft:dirt"]),
        ),
        (
            &[(
                "x",
                r#"{"values":["stone",{"id":"t:nope","required":false}]}"#,
            )],
            "t:x",
            Some(&["minecraft:stone"]),
        ),
        (
            &[(
                "x",
                r##"{"values":["stone",{"id":"#t:none","required":false}]}"##,
            )],
            "t:x",
            Some(&["minecraft:stone"]),
        ),
        (&[("x", r#"{"values":["stone","t:nope"]}"#)], "t:x", None),
        (
            &[("x", r#"{"values":["stone",{"id":"t:nope"}]}"#)],
            "t:x",
            None,
        ),
        (
            &[
                ("x", r##"{"values":["stone","#t:y"]}"##),
                ("y", r#"{"values":["t:nope"]}"#),
            ],
            "t:x",
            None,
        ),
        (
            &[
                (
                    "x",
                    r##"{"values":["stone",{"id":"#t:y","required":false}]}"##,
                ),
                ("y", r#"{"values":["t:nope"]}"#),
            ],
            "t:x",
            Some(&["minecraft:stone"]),
        ),
        (&[("e", r#"{"values":[]}"#)], "t:e", Some(&[])),
    ];
    for (files, name, expected) in cases {
        let (registries, _) = block_tags(files).load(declarations()).unwrap();
        let expected = expected.map(|e| e.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(tag::<Block>(&registries, name), expected, "{files:?}");
    }
}

#[test]
fn files_merge_in_stack_order_from_the_last_valid_replace() {
    let cases: [(&[&str], &[&str]); 3] = [
        (
            &[
                r#"{"values":["a"]}"#,
                r#"{"replace":true,"values":["b"]}"#,
                r#"{"values":["c"]}"#,
            ],
            &["minecraft:b", "minecraft:c"],
        ),
        (
            &[r#"{"values":["a"]}"#, r#"{"values":["b","a"]}"#],
            &["minecraft:a", "minecraft:b"],
        ),
        (
            &[
                r#"{"values":["stone"]}"#,
                r#"{"replace":true,"values":[3]}"#,
            ],
            &["minecraft:stone"],
        ),
    ];
    for (bodies, expected) in cases {
        let mut packs = Packs::new();
        for (n, body) in bodies.iter().enumerate() {
            packs = packs.world(
                &format!("p{n}"),
                &[("data/minecraft/tags/block/t.json", body)],
            );
        }
        let (registries, _) = packs.load(declarations()).unwrap();
        assert_eq!(
            tag::<Block>(&registries, "t"),
            Some(expected.iter().map(|s| s.to_string()).collect()),
            "{bodies:?}"
        );
    }
}

#[test]
fn a_schema_invalid_tag_file_still_makes_the_tag_exist_and_malformed_json_does_not() {
    let files = [
        ("schema", r#"{"values":[3]}"#, true),
        (
            "duplicate",
            r#"{"values":["stone"],"values":["dirt"]}"#,
            true,
        ),
        ("empty", "", true),
        ("blank", " \n\t ", true),
        ("malformed", "{", false),
    ];
    let bodies: Vec<(&str, &str)> = files.iter().map(|(n, b, _)| (*n, *b)).collect();
    let (registries, diagnostics) = block_tags(&bodies).load(declarations()).unwrap();
    for (name, _, exists) in files {
        let expected = exists.then(Vec::new);
        assert_eq!(
            tag::<Block>(&registries, &format!("t:{name}")),
            expected,
            "{name}"
        );
    }
    let skipped: BTreeSet<(String, String, String)> = diagnostics
        .iter()
        .filter_map(|d| match d {
            Diagnostic::TagFileSkipped {
                tag, file, pack, ..
            } => Some((tag.to_string(), file.clone(), pack.clone())),
            _ => None,
        })
        .collect();
    let expected: BTreeSet<(String, String, String)> = files
        .iter()
        .map(|(name, _, _)| {
            (
                format!("t:{name}"),
                format!("data/t/tags/block/{name}.json"),
                "file/a".to_owned(),
            )
        })
        .collect();
    assert_eq!(skipped, expected);
}

#[test]
fn cycles_fail_when_required_and_drop_one_optional_edge_otherwise() {
    type Expected<'a> = &'a [(&'a str, Option<&'a [&'a str]>)];
    let cases: [(&[(&str, &str)], Expected<'_>); 8] = [
        (
            &[
                ("a", r##"{"values":["#t:b"]}"##),
                ("b", r##"{"values":["#t:a"]}"##),
                ("c", r##"{"values":["x","#t:a"]}"##),
            ],
            &[("t:a", None), ("t:b", None), ("t:c", None)],
        ),
        (&[("a", r##"{"values":["x","#t:a"]}"##)], &[("t:a", None)]),
        (
            &[
                ("a", r##"{"values":["x","#t:b"]}"##),
                ("b", r##"{"values":["y",{"id":"#t:a","required":false}]}"##),
            ],
            &[
                ("t:a", Some(&["minecraft:x", "minecraft:y"])),
                ("t:b", Some(&["minecraft:y"])),
            ],
        ),
        (
            &[
                ("a", r##"{"values":["x",{"id":"#t:b","required":false}]}"##),
                ("b", r##"{"values":["y",{"id":"#t:a","required":false}]}"##),
            ],
            &[
                ("t:a", Some(&["minecraft:x", "minecraft:y"])),
                ("t:b", Some(&["minecraft:y"])),
            ],
        ),
        (
            &[
                (
                    "z:a",
                    r##"{"values":["x",{"id":"#a:b","required":false}]}"##,
                ),
                (
                    "a:b",
                    r##"{"values":["y",{"id":"#z:a","required":false}]}"##,
                ),
            ],
            &[
                ("z:a", Some(&["minecraft:x", "minecraft:y"])),
                ("a:b", Some(&["minecraft:y"])),
            ],
        ),
        (
            &[
                ("a", r##"{"values":["a",{"id":"#t:b","required":false}]}"##),
                ("b", r##"{"values":["b",{"id":"#t:c","required":false}]}"##),
                ("c", r##"{"values":["c",{"id":"#t:a","required":false}]}"##),
            ],
            &[
                ("t:a", Some(&["minecraft:a", "minecraft:b", "minecraft:c"])),
                ("t:b", Some(&["minecraft:b", "minecraft:c"])),
                ("t:c", Some(&["minecraft:c"])),
            ],
        ),
        (
            &[
                ("a", r##"{"values":["x",{"id":"#t:b","required":false}]}"##),
                ("b", r##"{"values":["y","#t:a"]}"##),
            ],
            &[
                ("t:a", Some(&["minecraft:x"])),
                ("t:b", Some(&["minecraft:y", "minecraft:x"])),
            ],
        ),
        (
            &[
                ("a", r##"{"values":["x",{"id":"#t:b","required":false}]}"##),
                (
                    "b",
                    r##"{"values":["y",{"id":"#t:a","required":false},{"id":"#t:c","required":false}]}"##,
                ),
                ("c", r#"{"values":["d"]}"#),
            ],
            &[
                ("t:a", Some(&["minecraft:x", "minecraft:y", "minecraft:d"])),
                ("t:b", Some(&["minecraft:y", "minecraft:d"])),
            ],
        ),
    ];
    for (files, expected) in cases {
        let (registries, _) = block_tags(files).load(declarations()).unwrap();
        for (name, members) in expected {
            let members = members.map(|m| m.iter().map(|s| s.to_string()).collect::<Vec<_>>());
            assert_eq!(tag::<Block>(&registries, name), members, "{files:?} {name}");
        }
    }
}

#[test]
fn a_failed_tag_is_reported_with_every_missing_member_and_its_pack() {
    let (_, diagnostics) = Packs::new()
        .world(
            "a",
            &[
                ("data/minecraft/tags/block/bad.json", r#"{"values":[3]}"#),
                ("data/minecraft/tags/item/bad.json", r#"{"values":[3]}"#),
                (
                    "data/minecraft/tags/block/t.json",
                    r#"{"values":["t:nope"]}"#,
                ),
            ],
        )
        .world(
            "b",
            &[(
                "data/minecraft/tags/block/t.json",
                r##"{"values":["#t:gone","stone"]}"##,
            )],
        )
        .load(declarations())
        .unwrap();
    let interesting: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|d| {
            matches!(
                d,
                Diagnostic::TagFileSkipped { .. } | Diagnostic::TagFailed { .. }
            )
        })
        .collect();
    let failed = Diagnostic::TagFailed {
        registry: rl("block"),
        tag: rl("t"),
        missing: vec![
            Missing {
                name: rl("t:nope"),
                tag: false,
                referrer: Referrer::TagFile {
                    tag: rl("t"),
                    pack: "file/a".into(),
                },
            },
            Missing {
                name: rl("t:gone"),
                tag: true,
                referrer: Referrer::TagFile {
                    tag: rl("t"),
                    pack: "file/b".into(),
                },
            },
        ],
    };
    let skipped = |d: &Diagnostic, registry: &str| {
        matches!(d, Diagnostic::TagFileSkipped { registry: r, tag, pack, .. }
            if *r == rl(registry) && *tag == rl("bad") && pack == "file/a")
    };
    assert!(
        matches!(
            interesting.as_slice(),
            [block, item, _] if skipped(block, "block") && skipped(item, "item")
        ),
        "{interesting:?}"
    );
    assert_eq!(interesting[2..], [&failed]);
    let text = failed.to_string();
    for part in ["minecraft:t", "t:nope", "#t:gone", "file/a", "file/b"] {
        assert!(text.contains(part), "{text}");
    }
}

#[test]
fn a_missing_required_entry_of_a_data_pack_registry_fails_the_startup_load() {
    let result = Packs::new()
        .world(
            "a",
            &[
                ("data/t/damage_type/a.json", &damage("a")),
                (
                    "data/t/tags/damage_type/t.json",
                    r#"{"values":["t:a","t:nope"]}"#,
                ),
            ],
        )
        .load(declarations());
    assert_eq!(
        report_errors(&result),
        [Diagnostic::Unresolved {
            registry: rl("damage_type"),
            missing: vec![Missing {
                name: rl("t:nope"),
                tag: false,
                referrer: Referrer::TagFile {
                    tag: rl("t:t"),
                    pack: "file/a".into()
                },
            }],
        }]
    );
}

#[derive(Deserialize)]
struct Report {
    entries: BTreeMap<String, Protocol>,
}

#[derive(Deserialize)]
struct Protocol {
    protocol_id: u32,
}

fn files_under(root: &Path, prefix: &str, pack: &str, packs: &mut Packs) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            packs.files_mut().insert(
                pack,
                format!("{prefix}{relative}"),
                fs::read(&path).unwrap(),
            );
        }
    }
}

#[test]
fn shipped_block_tags_load_as_the_reported_set() {
    let report: BTreeMap<String, Report> =
        serde_json::from_slice(&fs::read(assets().join("mcrs/reports/registries.json")).unwrap())
            .unwrap();
    let mut entries: Vec<(&String, u32)> = report["minecraft:block"]
        .entries
        .iter()
        .map(|(name, p)| (name, p.protocol_id))
        .collect();
    entries.sort_by_key(|e| e.1);
    let names = entries.into_iter().map(|(name, _)| rl(name)).collect();
    let declarations = [Declaration::fixed(rl("block"), names)];

    let mut packs = Packs::new();
    files_under(
        &vanilla_dir().join("tags/block"),
        "data/minecraft/tags/block/",
        "vanilla",
        &mut packs,
    );
    let (registries, diagnostics) = packs.load(declarations).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let ids: Vec<Id<Block>> = registries
        .entries::<Block>()
        .unwrap()
        .iter()
        .map(|(id, _)| id)
        .collect();
    let tags = registries.tags::<Block>().unwrap();

    #[derive(Deserialize)]
    struct Names {
        tags: BTreeMap<String, BTreeSet<String>>,
    }
    let listed: Names =
        serde_json::from_slice(&fs::read(assets().join("mcrs/reports/names.json")).unwrap())
            .unwrap();
    let loaded: BTreeSet<String> = tags.iter().map(|(_, n)| n.to_string()).collect();
    assert_eq!(loaded, listed.tags["minecraft:block"]);

    for (tag_id, _) in tags.iter() {
        let members: BTreeSet<_> = tags.members(tag_id).collect();
        for &id in &ids {
            assert_eq!(tags.contains(tag_id, id), members.contains(&id));
        }
    }
}

#[test]
fn a_replacing_feature_pack_tag_holds_only_its_own_members() {
    let declarations: Vec<Declaration> = vec![Declaration::data::<VillagerTrade>().into()];
    let trade = vanilla_dir().join("datapacks/trade_rebalance");
    let mut packs = Packs::new().feature(
        "trade_rebalance",
        fs::read(trade.join("pack.mcmeta")).unwrap(),
    );
    files_under(
        &vanilla_dir().join("villager_trade"),
        "data/minecraft/villager_trade/",
        "vanilla",
        &mut packs,
    );
    files_under(
        &vanilla_dir().join("tags/villager_trade"),
        "data/minecraft/tags/villager_trade/",
        "vanilla",
        &mut packs,
    );
    files_under(&trade.join("data"), "data/", "trade_rebalance", &mut packs);
    let (registries, _) = packs.load(declarations).unwrap();

    #[derive(Deserialize)]
    struct TagFile {
        values: Vec<String>,
    }
    let file: TagFile = serde_json::from_slice(
        &fs::read(trade.join("data/minecraft/tags/villager_trade/librarian/level_1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        tag::<VillagerTrade>(&registries, "librarian/level_1"),
        Some(file.values)
    );
}
