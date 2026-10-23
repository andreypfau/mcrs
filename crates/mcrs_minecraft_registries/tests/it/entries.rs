use std::mem::size_of;

use mcrs_minecraft_core::{Id, ResourceLocation};
use mcrs_minecraft_datapack::{Level, PackDiagnostic};
use mcrs_minecraft_registries::{Declaration, Diagnostic, Stage};

use crate::support::{
    Damage, DamageType, LootTable, Packs, Things, damage, declarations, get, names, report_errors,
    rl, tag, value,
};

fn with_damage(packs: Packs, pack: &str, names: &[&str]) -> Packs {
    let files: Vec<(String, String)> = names
        .iter()
        .map(|name| {
            let (namespace, path) = name.split_once(':').unwrap();
            (
                format!("data/{namespace}/damage_type/{path}.json"),
                damage(name),
            )
        })
        .collect();
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    if pack == "vanilla" {
        packs.vanilla(&files)
    } else {
        packs.world(pack, &files)
    }
}

#[test]
fn ids_follow_the_file_name_then_the_namespace() {
    let cases: [(&[&str], &[&str]); 4] = [
        (
            &[
                "minecraft:overworld",
                "minecraft:overworld_caves",
                "minecraft:the_end",
                "minecraft:the_nether",
                "t:aaa",
                "a:the_end",
            ],
            &[
                "t:aaa",
                "minecraft:overworld",
                "minecraft:overworld_caves",
                "a:the_end",
                "minecraft:the_end",
                "minecraft:the_nether",
            ],
        ),
        (&["m:a_b", "m:a", "m:a/b"], &["m:a", "m:a/b", "m:a_b"]),
        (
            &["u:a", "t:a", "t:a-b", "t:a.b", "t:a.k"],
            &["t:a-b", "t:a.b", "t:a", "u:a", "t:a.k"],
        ),
        (&["z:x", "a:y", "b:x", "a:x"], &["a:x", "b:x", "z:x", "a:y"]),
    ];
    for (input, expected) in cases {
        let (built_in, world): (Vec<&str>, Vec<&str>) =
            input.iter().partition(|n| n.starts_with("minecraft:"));
        let packs = with_damage(with_damage(Packs::new(), "vanilla", &built_in), "a", &world);
        let (registries, _) = packs.load(&declarations()).unwrap();
        let loaded = names::<DamageType>(&registries);
        assert_eq!(loaded, expected, "{input:?}");
        for (position, name) in expected.iter().enumerate() {
            assert_eq!(get::<DamageType>(&registries, name).index(), position);
        }
    }
}

fn loot_tables(count: u32) -> Packs {
    let mut packs = Packs::new().world("a", &[]);
    for n in 0..count {
        packs
            .files_mut()
            .insert("file/a", format!("data/t/loot_table/e{n:05}.json"), "{}");
    }
    packs
}

#[test]
fn an_id_is_two_bytes_and_a_registry_past_its_range_fails_to_load() {
    assert!(size_of::<Id<LootTable>>() <= 2);
    assert_eq!(
        report_errors(&loot_tables(65_537).load(&declarations())),
        [Diagnostic::TooManyEntries {
            registry: rl("loot_table"),
            count: 65_537,
        }]
    );
}

mod exhaustive {
    use super::*;

    #[test]
    fn a_registry_at_the_end_of_the_id_range_loads() {
        let (registries, _) = loot_tables(65_536).load(&declarations()).unwrap();
        let last = get::<LootTable>(&registries, "t:e65535");
        assert_eq!(last.number(), u16::MAX);
    }
}

#[test]
fn the_highest_pack_replaces_the_whole_file_without_merging() {
    let full = r#"{"message_id":"low","exhaustion":1.0}"#;
    let packs = Packs::new()
        .vanilla(&[
            ("data/t/damage_type/x.json", full),
            ("data/t/damage_type/v.json", full),
        ])
        .world("a", &[("data/t/damage_type/x.json", full)])
        .world(
            "b",
            &[(
                "data/t/damage_type/x.json",
                r#"{"message_id":"high","exhaustion":2.0}"#,
            )],
        );
    let (registries, _) = packs.load(&declarations()).unwrap();
    assert_eq!(
        value::<Damage>(&registries, "t:x"),
        &Damage {
            message_id: "high".into(),
            exhaustion: 2.0
        }
    );
    let source = |name: &str| {
        let pack = registries
            .origin(get::<DamageType>(&registries, name))
            .unwrap();
        (
            pack.id().to_owned(),
            pack.known_pack().map(|k| k.id.clone()),
        )
    };
    assert_eq!(source("t:x"), ("file/b".to_owned(), None));
    assert_eq!(
        source("t:v"),
        ("vanilla".to_owned(), Some("core".to_owned()))
    );

    for higher in [r#"{"message_id":"high"}"#, "{"] {
        let report = Packs::new()
            .world("a", &[("data/t/damage_type/x.json", full)])
            .world("b", &[("data/t/damage_type/x.json", higher)])
            .load(&declarations())
            .unwrap_err();
        let entry = report
            .iter()
            .find_map(|d| match d {
                Diagnostic::Entry {
                    entry, pack, file, ..
                } => Some((entry.clone(), pack.clone(), file.clone())),
                _ => None,
            })
            .expect("the winning file fails");
        assert_eq!(
            entry,
            (
                rl("t:x"),
                Some("file/b".to_owned()),
                Some("data/t/damage_type/x.json".to_owned())
            ),
            "{higher}"
        );
    }
}

#[test]
fn a_filter_hides_matching_files_of_lower_packs_only() {
    const LONG: &str = "t:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ALL: &[&str] = &[LONG, "t:arrow2", "t:x", "t:y", "t:z"];
    let cases: [(&str, &[&str], bool, bool); 9] = [
        (
            r#"{"block":[{"namespace":"^t$","path":"^damage_type/x\\.json$"}]}"#,
            &[LONG, "t:arrow2", "t:y", "t:z"],
            false,
            true,
        ),
        (r#"{"block":[{"path":"("}]}"#, ALL, true, true),
        (r#"{"block":"x"}"#, ALL, true, true),
        (
            r#"{"block":[{"path":"^damage_type/x"},{"path":"(a*)*\\1z"}]}"#,
            ALL,
            true,
            true,
        ),
        (
            r#"{"block":[{"namespace":"^t$","path":"^damage_type/(?!arrow)"}]}"#,
            &["t:arrow2", "t:z"],
            false,
            true,
        ),
        (
            r#"{"block":[{"namespace":"^t$","path":"^zzz$"},{"namespace":"^u$","path":"y\\.json"}]}"#,
            &[LONG, "t:arrow2", "t:x", "t:z"],
            false,
            true,
        ),
        (
            r#"{"block":[{"path":"^tags/damage_type/k\\.json$"}]}"#,
            ALL,
            false,
            false,
        ),
        (
            r#"{"block":[{"path":"^damage_type/(?>x)\\.json$"}]}"#,
            &[LONG, "t:arrow2", "t:y", "t:z"],
            false,
            true,
        ),
        (
            r#"{"block":[{"path":"^damage_type/y*+y"}]}"#,
            ALL,
            false,
            true,
        ),
    ];
    for (filter, expected, ignored, tag_kept) in cases {
        let meta = format!(
            r#"{{"pack":{{"description":"x","min_format":122,"max_format":122}},"filter":{filter}}}"#
        );
        let packs = with_damage(Packs::new(), "a", &["t:x", "t:y", "t:arrow2", LONG])
            .file(
                "file/a",
                "data/t/tags/damage_type/k.json",
                r#"{"values":[]}"#,
            )
            .world_with_meta("b", &meta, &[("data/t/damage_type/z.json", &damage("z"))]);
        let (_, stack_diagnostics) = packs.stack();
        let (registries, load_diagnostics) = packs.load(&declarations()).unwrap();
        let reported = stack_diagnostics
            .into_iter()
            .map(Diagnostic::Pack)
            .chain(load_diagnostics.iter().cloned())
            .filter(|d| {
                matches!(d, Diagnostic::Pack(PackDiagnostic::FilterIgnored { pack, .. }) if pack == "file/b")
            })
            .count();
        let reported = match reported {
            0 => false,
            1 => true,
            many => panic!("{filter}: the filter is reported {many} times"),
        };
        let mut loaded = names::<DamageType>(&registries);
        loaded.sort();
        assert_eq!(loaded, expected, "{filter}");
        assert_eq!(reported, ignored, "{filter}");
        assert_eq!(
            tag::<DamageType>(&registries, "t:k").is_some(),
            tag_kept,
            "{filter}"
        );
    }
}

#[test]
fn an_applicable_overlay_overrides_the_base_tree_of_its_pack() {
    let current = mcrs_minecraft_core::VERSION.pack_version.data_major;
    let meta = format!(
        r#"{{"pack":{{"description":"x","min_format":{current},"max_format":{current}}},
        "overlays":{{"entries":[
            {{"directory":"o","min_format":{current},"max_format":{current}}},
            {{"directory":"old","min_format":100,"max_format":110}},
            {{"directory":"later","min_format":{current},"max_format":{current}}}]}}}}"#
    );
    let (registries, _) = Packs::new()
        .world_with_meta(
            "a",
            &meta,
            &[
                ("data/t/damage_type/x.json", &damage("base")),
                ("data/t/damage_type/y.json", &damage("base")),
                ("o/data/t/damage_type/x.json", &damage("overlay")),
                ("o/data/t/damage_type/y.json", &damage("overlay")),
                ("later/data/t/damage_type/x.json", &damage("later")),
                ("old/data/t/damage_type/z.json", &damage("old")),
            ],
        )
        .load(&declarations())
        .unwrap();
    assert_eq!(names::<DamageType>(&registries), ["t:x", "t:y"]);
    let message = |name: &str| value::<Damage>(&registries, name).message_id.clone();
    assert_eq!(
        (message("t:x"), message("t:y")),
        ("later".into(), "overlay".into())
    );
}

#[test]
fn a_registry_outside_minecraft_reads_the_directories_of_its_path_alone() {
    let files = [
        ("data/u/things/a.json", damage("a")),
        ("data/u/t/things/b.json", damage("b")),
        (
            "data/u/tags/things/g.json",
            r#"{"values":["u:a"]}"#.to_owned(),
        ),
        (
            "data/u/tags/t/things/h.json",
            r#"{"values":["u:a"]}"#.to_owned(),
        ),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(p, c)| (*p, c.as_str())).collect();
    let declarations = [Declaration::data::<Things>().into()];
    let (registries, _) = Packs::new().world("a", &files).load(&declarations).unwrap();
    assert_eq!(names::<Things>(&registries), ["u:a"]);
    assert_eq!(
        tag::<Things>(&registries, "u:g"),
        Some(vec!["u:a".to_owned()])
    );
    assert_eq!(tag::<Things>(&registries, "u:h"), None);
}

#[test]
fn file_names_become_keys_or_are_skipped() {
    let files = [
        ("data/t/damage_type/Bad.json", damage("bad")),
        ("data/t/damage_type/Upper.JSON", damage("upper")),
        ("data/t/damage_type/.json", damage("empty path")),
        ("data/t/damage_type/sub/dir.json", damage("nested")),
        ("data/t/damage_type/notes.txt", "not an entry".to_owned()),
        ("data/t/item/Stone.json", "{}".to_owned()),
    ];
    let files: Vec<(&str, &str)> = files.iter().map(|(p, c)| (*p, c.as_str())).collect();
    for (zip, level) in [(false, Level::Error), (true, Level::Warning)] {
        let packs = match zip {
            false => Packs::new().world("a", &files),
            true => Packs::new().zip("a", &files),
        };
        let (registries, diagnostics) = packs.load(&declarations()).unwrap();
        assert_eq!(names::<DamageType>(&registries), ["t:", "t:sub/dir"]);
        let skipped: Vec<(String, Level)> = diagnostics
            .iter()
            .filter_map(|d| match d {
                Diagnostic::Pack(PackDiagnostic::InvalidFileName { path, .. }) => {
                    Some((path.clone(), d.level()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            skipped,
            [
                ("damage_type/Bad.json".to_owned(), level),
                ("damage_type/Upper.JSON".to_owned(), level),
            ],
            "zip: {zip}"
        );
    }
}

#[test]
fn an_entry_file_is_decoded_as_text_then_as_its_value() {
    enum Expected {
        Loads(&'static str),
        Field(&'static str),
        Fails,
    }
    let cases: [(&[u8], Expected); 7] = [
        (
            b"\xEF\xBB\xBF{\"message_id\":\"bom\",\"exhaustion\":0.1}",
            Expected::Loads("bom"),
        ),
        (
            b"{\"message_id\":\"a\xFFb\",\"exhaustion\":0.1}",
            Expected::Loads("a\u{FFFD}b"),
        ),
        (br#"{"message_id":"a"}"#, Expected::Fails),
        (
            br#"{"message_id":"a","exhaustion":"bad"}"#,
            Expected::Field("exhaustion"),
        ),
        (
            br#"{"message_id":"a","exhaustion":0.1,"exhaustion":5.0}"#,
            Expected::Fails,
        ),
        (
            br#"{"message_id":"a","exhaustion":0.1} // x"#,
            Expected::Fails,
        ),
        (b"", Expected::Fails),
    ];
    for (content, expected) in cases {
        let shown = String::from_utf8_lossy(content);
        let result = Packs::new()
            .world("a", &[])
            .file("file/a", "data/t/damage_type/x.json", content)
            .load(&declarations());
        if let Expected::Loads(message_id) = expected {
            let (registries, _) = result.unwrap_or_else(|d| panic!("{shown}: {d:?}"));
            let value = value::<Damage>(&registries, "t:x");
            assert_eq!(value.message_id, message_id, "{shown}");
            continue;
        }
        let report = result.err().unwrap_or_else(|| panic!("{shown} loads"));
        let field = report
            .iter()
            .find_map(|d| match d {
                Diagnostic::Entry { entry, field, .. } if *entry == rl("t:x") => {
                    Some(field.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{shown}: no entry error"));
        if let Expected::Field(name) = expected {
            assert_eq!(field.as_deref(), Some(name), "{shown}");
        }
    }
}

#[test]
fn built_in_entries_behave_as_files_of_their_pack() {
    let declaration = || -> Vec<Declaration> {
        vec![
            Declaration::data::<Damage>()
                .built_in(
                    "file/a",
                    vec![rl("plains"), rl("desert"), rl("forest"), rl("ocean")],
                    built("built"),
                )
                .into(),
        ]
    };
    let filter = r#"{"pack":{"description":"x","min_format":122,"max_format":122},
        "filter":{"block":[{"namespace":"^minecraft$","path":"^damage_type/ocean\\.json$"}]}}"#;
    let packs = Packs::new()
        .world(
            "a",
            &[(
                "data/minecraft/damage_type/forest.json",
                &damage("own file"),
            )],
        )
        .world(
            "b",
            &[
                (
                    "data/minecraft/damage_type/plains.json",
                    &damage("higher file"),
                ),
                ("data/t/damage_type/aaa.json", &damage("aaa")),
            ],
        )
        .world_with_meta("c", filter, &[])
        .world(
            "d",
            &[("data/minecraft/damage_type/ocean.json", &damage("top file"))],
        );
    let (registries, _) = packs.load(&declaration()).unwrap();
    let registry = registries.registry::<Damage>().unwrap();
    assert_eq!(
        names::<DamageType>(&registries),
        [
            "t:aaa",
            "minecraft:desert",
            "minecraft:forest",
            "minecraft:ocean",
            "minecraft:plains"
        ],
    );
    let value = |name: &str| {
        let id = get::<DamageType>(&registries, name);
        let origin = registries.origin(id).unwrap().id().to_owned();
        (registry.get(id).unwrap().message_id.clone(), origin)
    };
    assert_eq!(value("desert"), ("built desert".into(), "file/a".into()));
    assert_eq!(value("forest"), ("own file".into(), "file/a".into()));
    assert_eq!(value("plains"), ("higher file".into(), "file/b".into()));
    assert_eq!(value("ocean"), ("top file".into(), "file/d".into()));
}

#[test]
fn a_vanilla_built_in_replaces_the_games_file_and_yields_to_a_pack() {
    let declaration = || -> Vec<Declaration> {
        vec![
            Declaration::data::<Damage>()
                .built_in("vanilla", vec![rl("forest"), rl("plains")], built("built"))
                .into(),
        ]
    };
    let packs = Packs::new()
        .vanilla(&[
            (
                "data/minecraft/damage_type/forest.json",
                &damage("game file"),
            ),
            (
                "data/minecraft/damage_type/plains.json",
                &damage("game file"),
            ),
        ])
        .world(
            "a",
            &[(
                "data/minecraft/damage_type/plains.json",
                &damage("pack file"),
            )],
        );
    let (registries, _) = packs.load(&declaration()).unwrap();
    let registry = registries.registry::<Damage>().unwrap();
    let value = |name: &str| {
        let id = get::<DamageType>(&registries, name);
        let origin = registries.origin(id).unwrap().id().to_owned();
        (registry.get(id).unwrap().message_id.clone(), origin)
    };
    assert_eq!(value("forest"), ("built forest".into(), "vanilla".into()));
    assert_eq!(value("plains"), ("pack file".into(), "file/a".into()));
}

fn built(prefix: &'static str) -> impl Fn(&ResourceLocation) -> Option<Damage> + Send + Sync {
    move |name| {
        Some(Damage {
            message_id: format!("{prefix} {}", name.path()),
            exhaustion: 0.0,
        })
    }
}

#[test]
fn each_built_in_set_decodes_its_own_names_and_the_higher_pack_wins() {
    let declaration = || -> Vec<Declaration> {
        vec![
            Declaration::data::<Damage>()
                .built_in("file/a", vec![rl("shared"), rl("high")], built("a"))
                .built_in("vanilla", vec![rl("shared"), rl("low")], built("vanilla"))
                .into(),
        ]
    };
    let cases: [(&str, Packs, &[(&str, &str, &str)]); 2] = [
        (
            "both packs",
            Packs::new().world("a", &[]),
            &[
                ("high", "a high", "file/a"),
                ("low", "vanilla low", "vanilla"),
                ("shared", "a shared", "file/a"),
            ],
        ),
        (
            "file/a absent",
            Packs::new(),
            &[
                ("low", "vanilla low", "vanilla"),
                ("shared", "vanilla shared", "vanilla"),
            ],
        ),
    ];
    for (case, packs, expected) in cases {
        let (registries, _) = packs.load(&declaration()).unwrap();
        let registry = registries.registry::<Damage>().unwrap();
        let entries: Vec<(&str, &str, &str)> = registry
            .iter()
            .map(|(id, name, value)| {
                let origin = registries.origin(id).unwrap().id();
                (name.path(), value.message_id.as_str(), origin)
            })
            .collect();
        assert_eq!(entries, expected, "{case}");
    }
}

#[test]
fn a_registry_without_a_type_is_named_by_its_files_and_tagged_by_key() {
    let declarations = [
        Declaration::fixed(rl("sound"), vec![rl("a"), rl("b")]),
        Declaration::names_only(rl("advancement"), Stage::Reloadable),
    ];
    let packs = Packs::new().vanilla(&[
        ("data/minecraft/advancement/b.json", "not json"),
        ("data/minecraft/advancement/a/c.json", "{}"),
        (
            "data/minecraft/tags/advancement/all.json",
            r#"{"values":["a/c","b"]}"#,
        ),
        (
            "data/minecraft/tags/sound/second.json",
            r#"{"values":["b"]}"#,
        ),
    ]);
    let (registries, _) = packs.load(&declarations).unwrap();

    let advancement = rl("advancement");
    assert_eq!(
        registries.names_of(&advancement).unwrap(),
        [rl("a/c"), rl("b")]
    );
    let tags: Vec<_> = registries.tags_of(&advancement).unwrap().collect();
    assert_eq!(tags, [(&rl("all"), &[0, 1][..])]);
    let sound_tags: Vec<_> = registries.tags_of(&rl("sound")).unwrap().collect();
    assert_eq!(sound_tags, [(&rl("second"), &[1][..])]);
}
