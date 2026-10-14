use std::cell::Cell;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{
    Declaration, Id, LoadCtx, ReadValue, Registries, RegistryType, Stage,
};
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use crate::support::{
    Block, DamageType, Enchantment, Item, LootTable, Packs, Predicate, damage, declarations,
    report_errors, rl,
};

thread_local! {
    static SCOPE: Cell<Option<Stage>> = const { Cell::new(None) };
}

struct Scoped;
impl RegistryType for Scoped {
    type Value = Seen;
}

#[derive(Debug, PartialEq)]
struct Seen(Option<Stage>);

impl ReadValue for Seen {
    fn read<'de, D: Deserializer<'de>>(_: &LoadCtx<'_>, d: D) -> Result<Self, D::Error> {
        IgnoredAny::deserialize(d)?;
        Ok(Seen(SCOPE.get()))
    }
}

type TagView = Vec<(u32, ResourceLocation, Vec<u16>)>;
type RegistrySnapshot = (ResourceLocation, Vec<ResourceLocation>, TagView);

fn snapshot<R: RegistryType>(registries: &Registries) -> (Vec<ResourceLocation>, TagView) {
    let registry = registries.registry::<R>().unwrap();
    let tags = registries.tags::<R>().unwrap();
    let names = registry.iter().map(|(_, name, _)| name.clone()).collect();
    let tags = tags
        .iter()
        .map(|(tag, name)| {
            let members = tags.members(tag).map(Id::number).collect();
            (u32::try_from(tag.index()).unwrap(), name.clone(), members)
        })
        .collect();
    (names, tags)
}

fn final_snapshot(registries: &Registries, key: &str) -> (Vec<ResourceLocation>, TagView) {
    match key {
        "minecraft:block" => snapshot::<Block>(registries),
        "minecraft:item" => snapshot::<Item>(registries),
        "minecraft:damage_type" => snapshot::<DamageType>(registries),
        "minecraft:enchantment" => snapshot::<Enchantment>(registries),
        "minecraft:scoped" => snapshot::<Scoped>(registries),
        "minecraft:loot_table" => snapshot::<LootTable>(registries),
        "minecraft:predicate" => snapshot::<Predicate>(registries),
        other => panic!("{other} is not declared"),
    }
}

#[test]
fn each_stage_decodes_inside_the_caller_scope_with_its_final_names_and_tags() {
    let mut declarations = declarations();
    declarations.push(
        Declaration::data::<Scoped>(rl("scoped"), Stage::Dimension)
            .built_in("vanilla", vec![rl("built")], |_, _| Ok(Seen(SCOPE.get())))
            .validate(|_, _| match SCOPE.get() {
                Some(Stage::Dimension) => Ok(()),
                other => Err(format!("validated in {other:?}")),
            })
            .into(),
    );
    let packs = Packs::new().vanilla(&[
        ("data/minecraft/scoped/file.json", "{}"),
        ("data/minecraft/damage_type/fire.json", &damage("fire")),
        ("data/minecraft/damage_type/drown.json", &damage("drown")),
        (
            "data/minecraft/tags/block/logs.json",
            r#"{"values":["a","b"]}"#,
        ),
        (
            "data/minecraft/tags/damage_type/hot.json",
            r#"{"values":["fire"]}"#,
        ),
        ("data/minecraft/loot_table/x.json", "{}"),
    ]);

    let mut seen: Vec<(Stage, Vec<RegistrySnapshot>)> = Vec::new();
    let (registries, _) = packs
        .load_with(&declarations, None, &mut |view, decode| {
            let visible = view.registries().map(|registry| {
                let tags = registry
                    .tags()
                    .map(|(n, name, ids)| (n, name.clone(), ids.to_vec()));
                (
                    registry.key().clone(),
                    registry.names().to_vec(),
                    tags.collect(),
                )
            });
            seen.push((view.stage(), visible.collect()));
            SCOPE.set(Some(view.stage()));
            decode();
            SCOPE.set(None);
        })
        .unwrap();

    let stages: Vec<Stage> = seen.iter().map(|(stage, _)| *stage).collect();
    assert_eq!(stages, [Stage::World, Stage::Dimension, Stage::Reloadable]);
    let world = ["block", "damage_type", "enchantment", "item"];
    let expected_keys = [
        world.to_vec(),
        [&world[..], &["scoped"]].concat(),
        [&world[..], &["loot_table", "predicate", "scoped"]].concat(),
    ];
    for ((stage, visible), expected) in seen.iter().zip(expected_keys) {
        let mut keys: Vec<String> = visible
            .iter()
            .map(|(key, ..)| key.path().to_owned())
            .collect();
        keys.sort();
        assert_eq!(keys, expected, "{stage:?}");
        for (key, names, tags) in visible {
            let (final_names, final_tags) = final_snapshot(&registries, &key.to_string());
            assert_eq!(
                (names, tags),
                (&final_names, &final_tags),
                "{stage:?} {key}"
            );
        }
    }
    let (_, block_tags) = final_snapshot(&registries, "minecraft:block");
    assert_eq!(block_tags, [(0, rl("logs"), vec![2, 3])]);

    let scoped = registries.registry::<Scoped>().unwrap();
    let values: Vec<&Seen> = scoped.iter().map(|(_, _, value)| value).collect();
    assert_eq!(values, [&Seen(Some(Stage::Dimension)); 2]);
}

#[test]
fn a_scope_is_entered_once_per_decoded_stage_and_must_decode_it() {
    let packs = Packs::new().vanilla(&[("data/minecraft/damage_type/fire.json", &damage("fire"))]);
    let declarations = declarations();
    let previous = packs.load(&declarations).unwrap().0;
    let not_built =
        |key: &str| format!("minecraft:root/minecraft:{key}: the registry was not built");
    let cases = [
        (
            "a reload",
            Some(&previous),
            None,
            vec![Stage::Reloadable],
            vec![],
        ),
        (
            "a scope that skips the world stage",
            None,
            Some(Stage::World),
            vec![Stage::World],
            vec![not_built("damage_type"), not_built("enchantment")],
        ),
    ];
    for (name, previous, skipped, expected_stages, expected_errors) in cases {
        let mut entered = Vec::new();
        let outcome = packs.load_with(&declarations, previous, &mut |view, decode| {
            entered.push(view.stage());
            if Some(view.stage()) != skipped {
                decode();
            }
        });
        assert_eq!(entered, expected_stages, "{name}");
        let errors: Vec<String> = report_errors(&outcome)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(errors, expected_errors, "{name}");
    }
}

#[test]
fn a_reload_view_keeps_tag_numbers_and_omits_a_tag_that_failed() {
    let block_tags = |a_bad| {
        Packs::new().vanilla(&[
            ("data/minecraft/tags/block/a_bad.json", a_bad),
            (
                "data/minecraft/tags/block/logs.json",
                r#"{"values":["a","b"]}"#,
            ),
        ])
    };
    let declarations = declarations();
    let (previous, _) = block_tags(r#"{"values":["a"]}"#)
        .load(&declarations)
        .unwrap();
    let mut viewed = Vec::new();
    let (registries, _) = block_tags(r#"{"values":["t:nope"]}"#)
        .load_with(&declarations, Some(&previous), &mut |view, decode| {
            let block = view.registries().find(|r| *r.key() == rl("block")).unwrap();
            viewed.extend(
                block
                    .tags()
                    .map(|(n, name, ids)| (n, name.clone(), ids.to_vec())),
            );
            decode();
        })
        .unwrap();
    let expected = [(1, rl("logs"), vec![2, 3])];
    assert_eq!(viewed, expected);
    assert_eq!(final_snapshot(&registries, "minecraft:block").1, expected);
}
