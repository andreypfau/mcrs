use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use mcrs_minecraft_client_jar::Directory;
use mcrs_minecraft_core::ResourceLocation;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::{corpus, release};

const PREFIX: &str = "data/minecraft/";
const EXPERIMENTAL_PACKS: &str = "datapacks/";
const TAGS: &str = "tags/";
const NAMESPACE: &str = "minecraft:";
const WORLD_PRESET: &str = "minecraft:worldgen/world_preset";
const DIMENSION: &str = "minecraft:dimension";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Names {
    pub entries: BTreeMap<String, BTreeSet<String>>,
    pub tags: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Row {
    registry: String,
    tag: bool,
    name: String,
    added: bool,
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.added { '+' } else { '-' };
        let hash = if self.tag { "#" } else { "" };
        write!(f, "{sign}\t{}\t{hash}{}", self.registry, self.name)
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Datapack {
    others: IgnoredAny,
    pub registries: BTreeMap<String, Flags>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flags {
    pub elements: bool,
    stable: bool,
    tags: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldPreset {
    dimensions: BTreeMap<String, IgnoredAny>,
}

impl Datapack {
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| error.to_string())
    }

    pub fn elements(&self) -> BTreeMap<String, bool> {
        self.registries
            .iter()
            .map(|(name, flags)| (name.clone(), flags.elements))
            .collect()
    }
}

pub fn from_jar(jar: &[u8], registries: &BTreeMap<String, bool>) -> Result<Names, String> {
    let mut directories = HashMap::new();
    let mut names = Names::default();
    for (registry, elements) in registries {
        let path = registry
            .strip_prefix(NAMESPACE)
            .ok_or_else(|| format!("{registry}: not a registry of the minecraft namespace"))?;
        directories.insert(path, registry.as_str());
        if *elements {
            names.entries.insert(registry.clone(), BTreeSet::new());
        }
    }

    let files = Directory::of(jar)?.read(jar, |name| {
        name.strip_prefix(PREFIX)
            .is_some_and(|path| path.ends_with(".json") && !path.starts_with(EXPERIMENTAL_PACKS))
    })?;
    let mut dimensions = BTreeSet::new();
    for (name, bytes) in files {
        let path = name
            .strip_prefix(PREFIX)
            .and_then(|path| path.strip_suffix(".json"))
            .expect("a picked entry is a json file of the data pack");
        corpus::check(&name, path)?;
        let (tag, path) = match path.strip_prefix(TAGS) {
            Some(path) => (true, path),
            None => (false, path),
        };
        let Some((registry, below)) = locate(&directories, path) else {
            if tag {
                return Err(format!("{name}: no registry holds this tag"));
            }
            continue;
        };
        let identifier = identifier(&name, below)?;
        if tag {
            names
                .tags
                .entry(registry.to_owned())
                .or_default()
                .insert(identifier);
        } else if let Some(entries) = names.entries.get_mut(registry) {
            entries.insert(identifier);
            if registry == WORLD_PRESET {
                let preset: WorldPreset =
                    serde_json::from_slice(&bytes).map_err(|error| format!("{name}: {error}"))?;
                for dimension in preset.dimensions.into_keys() {
                    dimensions.insert(identifier_of(&name, &dimension)?);
                }
            }
        }
    }
    if !dimensions.is_empty() {
        names
            .entries
            .entry(DIMENSION.to_owned())
            .or_default()
            .extend(dimensions);
    }
    Ok(names)
}

fn locate<'r, 'p>(
    directories: &HashMap<&str, &'r str>,
    path: &'p str,
) -> Option<(&'r str, &'p str)> {
    path.rmatch_indices('/').find_map(|(slash, _)| {
        let registry = directories.get(&path[..slash])?;
        Some((*registry, &path[slash + 1..]))
    })
}

fn identifier(file: &str, path: &str) -> Result<String, String> {
    identifier_of(file, &format!("{NAMESPACE}{path}"))
}

fn identifier_of(file: &str, text: &str) -> Result<String, String> {
    ResourceLocation::read(text)
        .map(|identifier| identifier.as_str().to_owned())
        .map_err(|error| format!("{file}: {error}"))
}

pub fn render(names: &Names) -> Result<String, String> {
    release::pretty(names)
}

pub fn read(path: &Path) -> Result<Names, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Names::default()),
        Err(error) => Err(corpus::io(path, error)),
    }
}

pub fn diff(old: &Names, new: &Names) -> Vec<Row> {
    let empty = BTreeSet::new();
    let mut rows = Vec::new();
    for (tag, old, new) in [
        (false, &old.entries, &new.entries),
        (true, &old.tags, &new.tags),
    ] {
        let registries: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
        for registry in registries {
            let before = old.get(registry).unwrap_or(&empty);
            let after = new.get(registry).unwrap_or(&empty);
            let row = |name: &String, added| Row {
                registry: registry.clone(),
                tag,
                name: name.clone(),
                added,
            };
            rows.extend(before.difference(after).map(|name| row(name, false)));
            rows.extend(after.difference(before).map(|name| row(name, true)));
        }
    }
    rows.sort();
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{scratch, zipped};

    type Sets = BTreeMap<String, BTreeSet<String>>;

    const EMPTY: &[u8] = b"{}";
    const NORMAL: &[u8] = br#"{"dimensions":{"minecraft:overworld":{},"minecraft:the_nether":{}}}"#;
    const FLAT: &[u8] = br#"{"dimensions":{"minecraft:overworld":{},"minecraft:the_end":{}}}"#;

    fn sets(rows: &[(&str, &[&str])]) -> Sets {
        rows.iter()
            .map(|(registry, names)| {
                (
                    (*registry).to_owned(),
                    names.iter().map(|name| (*name).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn flags(rows: &[(&str, bool)]) -> BTreeMap<String, bool> {
        rows.iter()
            .map(|(name, elements)| ((*name).to_owned(), *elements))
            .collect()
    }

    fn names_of(entries: &[(&str, &[u8])], registries: &[(&str, bool)]) -> Result<Names, String> {
        from_jar(&zipped(entries), &flags(registries))
    }

    fn lines(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::to_string).collect()
    }

    #[test]
    fn the_names_report_lists_entries_and_tags_of_the_jar() {
        let names = names_of(
            &[
                ("data/minecraft/worldgen/biome/plains.json", EMPTY),
                ("data/minecraft/worldgen/biome/cave/x.json", EMPTY),
                ("data/minecraft/worldgen/world_preset/normal.json", NORMAL),
                ("data/minecraft/tags/block/logs.json", EMPTY),
                ("data/minecraft/tags/block/mineable/axe.json", EMPTY),
                ("data/minecraft/tags/worldgen/biome/is_forest.json", EMPTY),
            ],
            &[
                ("minecraft:block", false),
                ("minecraft:dimension", true),
                ("minecraft:worldgen/biome", true),
                ("minecraft:worldgen/world_preset", true),
            ],
        )
        .unwrap();

        assert_eq!(
            names,
            Names {
                entries: sets(&[
                    (
                        "minecraft:dimension",
                        &["minecraft:overworld", "minecraft:the_nether"]
                    ),
                    (
                        "minecraft:worldgen/biome",
                        &["minecraft:cave/x", "minecraft:plains"]
                    ),
                    ("minecraft:worldgen/world_preset", &["minecraft:normal"]),
                ]),
                tags: sets(&[
                    (
                        "minecraft:block",
                        &["minecraft:logs", "minecraft:mineable/axe"]
                    ),
                    ("minecraft:worldgen/biome", &["minecraft:is_forest"]),
                ]),
            }
        );
    }

    #[test]
    fn experimental_packs_are_left_out() {
        let names = names_of(
            &[
                (
                    "data/minecraft/datapacks/trade_rebalance/data/minecraft/loot_table/chests/x.json",
                    EMPTY,
                ),
                (
                    "data/minecraft/datapacks/trade_rebalance/data/minecraft/tags/block/../y.json",
                    EMPTY,
                ),
                ("data/minecraft/loot_table/kept.json", EMPTY),
            ],
            &[("minecraft:loot_table", true), ("minecraft:block", false)],
        )
        .unwrap();

        assert_eq!(
            names,
            Names {
                entries: sets(&[("minecraft:loot_table", &["minecraft:kept"])]),
                tags: Sets::new(),
            }
        );
    }

    #[test]
    fn dimension_names_come_from_every_world_preset() {
        let names = names_of(
            &[
                ("data/minecraft/worldgen/world_preset/normal.json", NORMAL),
                ("data/minecraft/worldgen/world_preset/flat.json", FLAT),
            ],
            &[
                ("minecraft:dimension", true),
                ("minecraft:worldgen/world_preset", true),
            ],
        )
        .unwrap();

        assert_eq!(
            names.entries["minecraft:dimension"],
            sets(&[(
                "",
                &[
                    "minecraft:overworld",
                    "minecraft:the_end",
                    "minecraft:the_nether"
                ]
            )])[""]
        );
    }

    #[test]
    fn a_directory_that_extends_a_registry_path_is_another_directory() {
        let names = names_of(
            &[
                ("data/minecraft/worldgen/biome_extra/a.json", EMPTY),
                ("data/minecraft/worldgen/biome/cave/x.json", EMPTY),
            ],
            &[("minecraft:worldgen/biome", true)],
        )
        .unwrap();

        assert_eq!(
            names.entries,
            sets(&[("minecraft:worldgen/biome", &["minecraft:cave/x"])])
        );
    }

    #[test]
    fn a_registry_without_files_is_listed_empty() {
        let names = names_of(
            &[("data/minecraft/tags/block/logs.json", EMPTY)],
            &[
                ("minecraft:block", false),
                ("minecraft:slot_source", true),
                ("minecraft:worldgen/biome", true),
            ],
        )
        .unwrap();

        assert_eq!(
            names,
            Names {
                entries: sets(&[
                    ("minecraft:slot_source", &[]),
                    ("minecraft:worldgen/biome", &[])
                ]),
                tags: sets(&[("minecraft:block", &["minecraft:logs"])]),
            }
        );
    }

    #[test]
    fn a_path_that_leaves_the_data_pack_is_refused() {
        for name in [
            "data/minecraft/worldgen/biome/../outside.json",
            "data/minecraft/worldgen/biome/./dot.json",
            "data/minecraft//double.json",
            "data/minecraft/worldgen/biome//double.json",
            "data/minecraft/worldgen/biome/back\\slash.json",
            "data/minecraft/worldgen/biome/.json",
        ] {
            let error =
                names_of(&[(name, EMPTY)], &[("minecraft:worldgen/biome", true)]).unwrap_err();

            assert!(error.contains(name), "{name}: {error}");
        }
    }

    #[test]
    fn a_file_name_that_is_not_an_identifier_is_refused() {
        for name in [
            "data/minecraft/worldgen/biome/Plains.json",
            "data/minecraft/worldgen/biome/a b.json",
            "data/minecraft/worldgen/biome/cave/X.json",
            "data/minecraft/worldgen/biome/a:b.json",
        ] {
            let error =
                names_of(&[(name, EMPTY)], &[("minecraft:worldgen/biome", true)]).unwrap_err();

            assert!(error.contains(name), "{name}: {error}");
        }
    }

    #[test]
    fn a_tag_of_no_registry_is_refused() {
        let name = "data/minecraft/tags/worldgen/biome_extra/t.json";

        let error = names_of(&[(name, EMPTY)], &[("minecraft:worldgen/biome", true)]).unwrap_err();

        assert!(error.contains(name), "{error}");
    }

    #[test]
    fn the_report_is_sorted_and_repeats_exactly() {
        let forward: [(&str, &[u8]); 4] = [
            ("data/minecraft/worldgen/biome/b.json", EMPTY),
            ("data/minecraft/worldgen/biome/a.json", EMPTY),
            ("data/minecraft/tags/block/z.json", EMPTY),
            ("data/minecraft/tags/block/y.json", EMPTY),
        ];
        let mut backward = forward;
        backward.reverse();
        let registries = [
            ("minecraft:block", false),
            ("minecraft:worldgen/biome", true),
        ];

        let first = names_of(&forward, &registries).unwrap();
        let second = names_of(&backward, &registries).unwrap();
        let text = render(&first).unwrap();

        assert_eq!(text, render(&second).unwrap());
        assert!(text.find("minecraft:a").unwrap() < text.find("minecraft:b").unwrap());
        assert!(text.find("minecraft:y").unwrap() < text.find("minecraft:z").unwrap());
        assert!(text.ends_with("}\n"));
        assert_eq!(serde_json::from_str::<Names>(&text).unwrap(), first);
        assert!(diff(&first, &second).is_empty());
    }

    #[test]
    fn the_diff_names_added_and_removed_entries_and_tags() {
        let old = Names {
            entries: sets(&[("minecraft:worldgen/biome", &["minecraft:a", "minecraft:b"])]),
            tags: sets(&[
                ("minecraft:block", &["minecraft:t1"]),
                ("minecraft:item", &["minecraft:x"]),
            ]),
        };
        let new = Names {
            entries: sets(&[
                ("minecraft:dimension", &["minecraft:d"]),
                ("minecraft:worldgen/biome", &["minecraft:b", "minecraft:c"]),
            ]),
            tags: sets(&[
                ("minecraft:block", &["minecraft:t1", "minecraft:t2"]),
                ("minecraft:worldgen/biome", &["minecraft:is_forest"]),
            ]),
        };

        assert_eq!(
            lines(&diff(&old, &new)),
            [
                "+\tminecraft:block\t#minecraft:t2",
                "+\tminecraft:dimension\tminecraft:d",
                "-\tminecraft:item\t#minecraft:x",
                "-\tminecraft:worldgen/biome\tminecraft:a",
                "+\tminecraft:worldgen/biome\tminecraft:c",
                "+\tminecraft:worldgen/biome\t#minecraft:is_forest",
            ]
        );
    }

    #[test]
    fn the_registries_of_the_datapack_report_say_whether_they_have_elements() {
        let text = r#"{"others":{"function":{"elements":true}},"registries":{
            "minecraft:a":{"elements":true,"stable":true,"tags":false},
            "minecraft:b":{"elements":false,"stable":false,"tags":true}}}"#;

        assert_eq!(
            Datapack::parse(text).unwrap().elements(),
            flags(&[("minecraft:a", true), ("minecraft:b", false)])
        );
    }

    #[test]
    fn an_absent_stored_report_reads_as_empty() {
        let dir = scratch("names-absent");

        assert_eq!(read(&dir.join("names.json")).unwrap(), Names::default());
    }
}
