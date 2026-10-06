use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use mcrs_minecraft_core::ResourceLocation;

use crate::{corpus, names, registries};

pub type Files = BTreeMap<String, String>;

const HEADER: &str = "// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.\n";
const NAMESPACE: &str = "minecraft:";
const NO_CONSTANTS: [&str; 2] = ["minecraft:recipe", "minecraft:advancement"];
const PREFIXES: [(&str, &str); 2] = [("minecraft", ""), ("brigadier", "BRIGADIER_")];
const WORDS: [(&str, &str); 3] = [("5", "FIVE"), ("11", "ELEVEN"), ("13", "THIRTEEN")];

pub fn generate(
    registries: &registries::Report,
    datapack: &names::Datapack,
    names: &names::Names,
) -> Result<Files, String> {
    let named = registries
        .keys()
        .chain(names.entries.keys())
        .chain(names.tags.keys());
    for registry in named {
        if !datapack.registries.contains_key(registry) {
            return Err(format!("{registry}: not a registry of datapack.json"));
        }
    }

    let mut files = Files::new();
    let mut markers = String::new();
    let mut bindings = Vec::new();
    let mut modules = Vec::new();
    let mut static_names = Vec::new();
    let mut claimed: BTreeMap<String, &str> = ["registry", "lib"]
        .into_iter()
        .map(|name| (name.to_owned(), "src"))
        .collect();
    for (registry, flags) in &datapack.registries {
        let (module, marker) = identity(registry)?;
        let tags = names.tags.get(registry).filter(|tags| !tags.is_empty());
        let tag_module = tags.map(|_| format!("{module}_tags"));
        for claim in [Some(&module), Some(&marker), tag_module.as_ref()]
            .into_iter()
            .flatten()
        {
            if let Some(other) = claimed.insert(claim.clone(), registry) {
                return Err(format!("{other} and {registry} are both named {claim}"));
            }
        }

        let statics = registries.get(registry);
        let entries = names.entries.get(registry);
        if statics.is_some() && entries.is_some() {
            return Err(format!(
                "{registry}: a registry of registries.json also has entries in names.json"
            ));
        }
        if flags.elements && statics.is_none() && entries.is_none() {
            return Err(format!("{registry}: names.json lists no entries for it"));
        }

        marker_text(&mut markers, registry, &marker, &module);
        bindings.push(module.to_ascii_uppercase());
        if let Some(report) = statics {
            static_names.push((registry.clone(), module.clone(), report.entries.is_empty()));
        }
        let text = match (statics, entries) {
            (Some(report), _) if !report.entries.is_empty() => {
                Some(static_module(registry, &marker, report)?)
            }
            (None, Some(entries))
                if !entries.is_empty() && !NO_CONSTANTS.contains(&registry.as_str()) =>
            {
                Some(data_module(registry, &marker, entries)?)
            }
            _ => None,
        };
        if let Some(text) = text {
            files.insert(format!("src/{module}.rs"), text);
            modules.push(module);
        }
        if let (Some(tags), Some(tag_module)) = (tags, tag_module) {
            files.insert(
                format!("src/{tag_module}.rs"),
                tag_module_text(registry, &marker, tags)?,
            );
            modules.push(tag_module);
        }
    }

    files.insert(
        "src/registry.rs".to_owned(),
        registry_file(&markers, &bindings),
    );
    modules.push("registry".to_owned());
    modules.sort();
    files.insert("src/lib.rs".to_owned(), lib_file(&modules, &static_names));
    Ok(files)
}

pub fn write(root: &Path, files: &Files) -> Result<(), String> {
    let mut written = 0;
    for (path, text) in files {
        written += usize::from(corpus::write_if_changed(root, path, text.as_bytes())?);
    }

    let mut on_disk = Vec::new();
    corpus::files_below(root, &root.join("src"), &mut on_disk)?;
    let mut deleted = 0;
    for path in on_disk {
        if path.ends_with(".rs") && !files.contains_key(&path) {
            let full = root.join(&path);
            fs::remove_file(&full).map_err(|error| corpus::io(&full, error))?;
            println!("deleted {path}");
            deleted += 1;
        }
    }
    println!("keys: {written} written, {deleted} deleted");
    Ok(())
}

fn identity(registry: &str) -> Result<(String, String), String> {
    let location = location(registry, registry)?;
    let path = location
        .as_str()
        .strip_prefix(NAMESPACE)
        .ok_or_else(|| format!("{registry}: not a registry of the minecraft namespace"))?;
    let module = path
        .strip_prefix("worldgen/")
        .unwrap_or(path)
        .replace('/', "_");
    let word = |word: &str| {
        let mut chars = word.chars();
        chars
            .next()
            .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
            .unwrap_or_default()
    };
    let valid = module.starts_with(|first: char| first.is_ascii_lowercase())
        && module
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        return Err(format!("{registry}: no module name can be made of it"));
    }
    let marker = module.split('_').map(word).collect();
    Ok((module, marker))
}

fn location(context: &str, name: &str) -> Result<ResourceLocation<std::sync::Arc<str>>, String> {
    let location = ResourceLocation::read(name).map_err(|error| format!("{context}: {error}"))?;
    if location.as_str() != name {
        return Err(format!(
            "{context}: {name} is not written as namespace:path"
        ));
    }
    Ok(location)
}

fn constant(registry: &str, name: &str) -> Result<String, String> {
    let location = location(registry, name)?;
    let (namespace, path) = (location.namespace(), location.path());
    let Some((_, prefix)) = PREFIXES.iter().find(|(known, _)| *known == namespace) else {
        return Err(format!(
            "{registry}: {name}: no constant for namespace {namespace}"
        ));
    };
    let word = WORDS
        .iter()
        .find(|(word, _)| namespace == "minecraft" && *word == path);
    let body = match word {
        Some((_, word)) => (*word).to_owned(),
        None => path
            .chars()
            .map(|c| match c {
                'a'..='z' => Ok(c.to_ascii_uppercase()),
                '0'..='9' | '_' => Ok(c),
                '/' | '.' | '-' => Ok('_'),
                _ => Err(format!("{registry}: {name}: no constant for {c:?}")),
            })
            .collect::<Result<String, String>>()?,
    };
    let constant = format!("{prefix}{body}");
    if !constant.starts_with(|first: char| first.is_ascii_uppercase()) {
        return Err(format!(
            "{registry}: {name}: no constant name starts like this"
        ));
    }
    Ok(constant)
}

fn constants<'n>(
    registry: &str,
    names: impl Iterator<Item = &'n str>,
    reserved: &[&str],
) -> Result<Vec<(String, &'n str)>, String> {
    let mut seen: BTreeMap<String, &str> = reserved
        .iter()
        .map(|reserved| ((*reserved).to_owned(), *reserved))
        .collect();
    let mut constants = Vec::new();
    for name in names {
        let constant = constant(registry, name)?;
        if let Some(other) = seen.insert(constant.clone(), name) {
            return Err(format!(
                "{registry}: {other} and {name} are both the constant {constant}"
            ));
        }
        constants.push((constant, name));
    }
    Ok(constants)
}

fn marker_text(out: &mut String, registry: &str, marker: &str, module: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    let key = module.to_ascii_uppercase();
    out.push_str(&format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum {marker} {{}}\n\
         pub const {key}: RegistryKey<{marker}> = RegistryKey::new(rl!(\"{registry}\"));\n\
         impl Registered for {marker} {{\n    \
             const REGISTRY: RegistryKey<Self> = {key};\n\
         }}\n"
    ));
}

fn static_module(
    registry: &str,
    marker: &str,
    report: &registries::Registry,
) -> Result<String, String> {
    let mut by_id: Vec<Option<&str>> = vec![None; report.entries.len()];
    for (name, entry) in &report.entries {
        let slot = by_id.get_mut(usize::from(entry.protocol_id));
        match slot {
            Some(slot @ None) => *slot = Some(name),
            _ => {
                return Err(format!(
                    "{registry}: {name}: protocol id {} is outside 0..{} or taken",
                    entry.protocol_id,
                    report.entries.len()
                ));
            }
        }
    }

    let mut out =
        format!("{HEADER}\nmcrs_minecraft_registry::static_keys! {{\n    crate::{marker};\n");
    let names = by_id.into_iter().flatten();
    for (constant, name) in constants(registry, names, &["ENTRIES"])? {
        out.push_str(&format!("    {constant} = \"{name}\",\n"));
    }
    out.push_str("}\n");
    Ok(out)
}

fn data_module(registry: &str, marker: &str, entries: &BTreeSet<String>) -> Result<String, String> {
    let mut out = format!("{HEADER}\nuse mcrs_minecraft_core::{{ResourceKey, rl}};\n\n");
    for (constant, name) in constants(registry, entries.iter().map(String::as_str), &[])? {
        out.push_str(&format!(
            "pub const {constant}: ResourceKey<crate::{marker}, &'static str> = ResourceKey::new(rl!(\"{name}\"));\n"
        ));
    }
    Ok(out)
}

fn tag_module_text(
    registry: &str,
    marker: &str,
    tags: &BTreeSet<String>,
) -> Result<String, String> {
    let mut out = format!("{HEADER}\nuse mcrs_minecraft_core::{{TagKey, rl}};\n\n");
    for (constant, name) in constants(registry, tags.iter().map(String::as_str), &[])? {
        out.push_str(&format!(
            "pub const {constant}: TagKey<crate::{marker}, &'static str> = TagKey::new(rl!(\"{name}\"));\n"
        ));
    }
    Ok(out)
}

fn registry_file(markers: &str, bindings: &[String]) -> String {
    let mut list = String::new();
    for key in bindings {
        list.push_str(&format!("        {key}.binding(),\n"));
    }
    format!(
        "{HEADER}\n\
         use mcrs_minecraft_core::{{RegistryKey, TypeBinding, rl}};\n\
         \n\
         pub trait Registered: Sized + 'static {{\n    \
             const REGISTRY: RegistryKey<Self>;\n\
         }}\n\
         \n\
         {markers}\n\
         pub fn bindings() -> [TypeBinding; {len}] {{\n    \
             [\n\
         {list}    \
             ]\n\
         }}\n",
        len = bindings.len()
    )
}

fn lib_file(modules: &[String], statics: &[(String, String, bool)]) -> String {
    let mut out = format!("{HEADER}\n");
    for module in modules {
        out.push_str(&format!("#[rustfmt::skip]\npub mod {module};\n"));
    }
    out.push_str("\npub use registry::*;\n\nuse mcrs_minecraft_core::StaticResourceLocation;\n");
    out.push_str(
        "\n#[rustfmt::skip]\n\
         pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[\n",
    );
    for (_, module, empty) in statics {
        let entries = if *empty {
            "&[]".to_owned()
        } else {
            format!("{module}::ENTRIES")
        };
        let key = module.to_ascii_uppercase();
        out.push_str(&format!("    ({key}.location(), {entries}),\n"));
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use serde_json::{Map, Value, json};

    use super::*;
    use crate::testing::scratch;

    type Statics<'a> = &'a [(&'a str, &'a [(&'a str, u16)])];
    type Data<'a> = &'a [(&'a str, &'a [&'a str])];
    type Reports = (registries::Report, names::Datapack, names::Names);
    type Refusal<'a> = (Statics<'a>, Data<'a>, &'a [&'a str]);

    const BLOCK: &[(&str, u16)] = &[("minecraft:air", 0), ("minecraft:stone", 1)];
    const COMMAND: &str = "cargo run -p mcrs_minecraft_update -- names";

    fn reports(statics: Statics, data: Data) -> Reports {
        let mut flags = Map::new();
        let mut report = Map::new();
        for (index, (registry, entries)) in statics.iter().enumerate() {
            flags.insert(
                (*registry).to_owned(),
                json!({"elements": false, "stable": true, "tags": false}),
            );
            let entries: Map<String, Value> = entries
                .iter()
                .map(|(name, id)| ((*name).to_owned(), json!({ "protocol_id": id })))
                .collect();
            report.insert(
                (*registry).to_owned(),
                json!({"protocol_id": index, "entries": entries}),
            );
        }
        for (registry, _) in data {
            flags.insert(
                (*registry).to_owned(),
                json!({"elements": true, "stable": true, "tags": false}),
            );
        }
        let datapack = json!({"others": {}, "registries": flags});
        let names = names::Names {
            entries: data
                .iter()
                .map(|(registry, names)| {
                    (
                        (*registry).to_owned(),
                        names.iter().map(|name| (*name).to_owned()).collect(),
                    )
                })
                .collect(),
            tags: BTreeMap::new(),
        };
        (
            registries::parse(&Value::Object(report).to_string()).unwrap(),
            names::Datapack::parse(&datapack.to_string()).unwrap(),
            names,
        )
    }

    fn generated(statics: Statics, data: Data) -> Result<Files, String> {
        let (registries, datapack, names) = reports(statics, data);
        generate(&registries, &datapack, &names)
    }

    fn constants(files: &Files, path: &str) -> Vec<String> {
        files[path]
            .lines()
            .filter_map(|line| {
                line.strip_prefix("pub const ")
                    .and_then(|line| line.split_once(':'))
                    .or_else(|| line.strip_prefix("    ")?.split_once(" = \""))
            })
            .map(|(name, _)| name.to_owned())
            .collect()
    }

    fn assert_refused(result: Result<Files, String>, expected: &[&str]) {
        let error = result.unwrap_err();
        for name in expected {
            assert!(error.contains(name), "{error:?} should name {name}");
        }
    }

    #[test]
    fn a_small_report_generates_these_files() {
        let files = generated(
            &[("minecraft:block", BLOCK)],
            &[("minecraft:worldgen/biome", &["minecraft:plains"])],
        )
        .unwrap();

        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "src/biome.rs",
                "src/block.rs",
                "src/lib.rs",
                "src/registry.rs"
            ]
        );
        assert_eq!(
            files["src/lib.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.\n\
             \n\
             #[rustfmt::skip]\n\
             pub mod biome;\n\
             #[rustfmt::skip]\n\
             pub mod block;\n\
             #[rustfmt::skip]\n\
             pub mod registry;\n\
             \n\
             pub use registry::*;\n\
             \n\
             use mcrs_minecraft_core::StaticResourceLocation;\n\
             \n\
             #[rustfmt::skip]\n\
             pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[\n\
             \x20   (BLOCK.location(), block::ENTRIES),\n\
             ];\n"
        );
        assert_eq!(
            files["src/registry.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.\n\
             \n\
             use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};\n\
             \n\
             pub trait Registered: Sized + 'static {\n\
             \x20   const REGISTRY: RegistryKey<Self>;\n\
             }\n\
             \n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
             pub enum Block {}\n\
             pub const BLOCK: RegistryKey<Block> = RegistryKey::new(rl!(\"minecraft:block\"));\n\
             impl Registered for Block {\n\
             \x20   const REGISTRY: RegistryKey<Self> = BLOCK;\n\
             }\n\
             \n\
             #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
             pub enum Biome {}\n\
             pub const BIOME: RegistryKey<Biome> = RegistryKey::new(rl!(\"minecraft:worldgen/biome\"));\n\
             impl Registered for Biome {\n\
             \x20   const REGISTRY: RegistryKey<Self> = BIOME;\n\
             }\n\
             \n\
             pub fn bindings() -> [TypeBinding; 2] {\n\
             \x20   [\n\
             \x20       BLOCK.binding(),\n\
             \x20       BIOME.binding(),\n\
             \x20   ]\n\
             }\n"
        );
        assert_eq!(
            files["src/block.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.\n\
             \n\
             mcrs_minecraft_registry::static_keys! {\n\
             \x20   crate::Block;\n\
             \x20   AIR = \"minecraft:air\",\n\
             \x20   STONE = \"minecraft:stone\",\n\
             }\n"
        );
        assert_eq!(
            files["src/biome.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.\n\
             \n\
             use mcrs_minecraft_core::{ResourceKey, rl};\n\
             \n\
             pub const PLAINS: ResourceKey<crate::Biome, &'static str> = ResourceKey::new(rl!(\"minecraft:plains\"));\n"
        );
    }

    #[test]
    fn generation_repeats_exactly() {
        let statics: Statics = &[("minecraft:block", BLOCK)];
        let data: Data = &[("minecraft:worldgen/biome", &["minecraft:plains"])];

        let first = generated(statics, data).unwrap();
        assert_eq!(first, generated(statics, data).unwrap());
        assert!(!first.is_empty());
    }

    #[test]
    fn digit_names_take_their_word() {
        let files = generated(
            &[(
                "minecraft:command_argument_type",
                &[("brigadier:bool", 0), ("minecraft:entity", 1)],
            )],
            &[(
                "minecraft:jukebox_song",
                &[
                    "minecraft:11",
                    "minecraft:13",
                    "minecraft:5",
                    "minecraft:music.overworld.forest",
                    "minecraft:music_disc_5",
                ],
            )],
        )
        .unwrap();

        assert_eq!(
            constants(&files, "src/jukebox_song.rs"),
            [
                "ELEVEN",
                "THIRTEEN",
                "FIVE",
                "MUSIC_OVERWORLD_FOREST",
                "MUSIC_DISC_5"
            ]
        );
        assert_eq!(
            constants(&files, "src/command_argument_type.rs"),
            ["BRIGADIER_BOOL", "ENTITY"]
        );
        assert!(
            files["src/jukebox_song.rs"].contains(
                "pub const FIVE: ResourceKey<crate::JukeboxSong, &'static str> = ResourceKey::new(rl!(\"minecraft:5\"));"
            ),
            "{}",
            files["src/jukebox_song.rs"]
        );
    }

    #[test]
    fn a_constant_collision_stops_generation() {
        let cases: [Refusal; 2] = [
            (
                &[],
                &[(
                    "minecraft:worldgen/biome",
                    &["minecraft:a/b", "minecraft:a_b"],
                )],
                &["minecraft:a/b", "minecraft:a_b"],
            ),
            (
                &[("minecraft:block", &[("minecraft:entries", 0)])],
                &[],
                &["minecraft:entries", "ENTRIES"],
            ),
        ];
        for (statics, data, expected) in cases {
            assert_refused(generated(statics, data), expected);
        }
    }

    #[test]
    fn an_unmapped_name_stops_generation() {
        let cases: [Refusal; 4] = [
            (
                &[],
                &[("minecraft:worldgen/biome", &["minecraft:7"])],
                &["minecraft:7"],
            ),
            (
                &[],
                &[("minecraft:worldgen/biome", &["minecraft:7up"])],
                &["minecraft:7up"],
            ),
            (
                &[],
                &[("minecraft:worldgen/biome", &["other:x"])],
                &["other:x"],
            ),
            (&[("minecraft:block", &[("other:y", 0)])], &[], &["other:y"]),
        ];
        for (statics, data, expected) in cases {
            assert_refused(generated(statics, data), expected);
        }
    }

    #[test]
    fn a_static_registry_without_entries_is_listed_and_has_no_module() {
        let files = generated(&[("minecraft:none", &[]), ("minecraft:block", BLOCK)], &[]).unwrap();

        assert!(files["src/lib.rs"].contains("    (NONE.location(), &[]),\n"));
        assert!(files["src/lib.rs"].contains("    (BLOCK.location(), block::ENTRIES),\n"));
        assert!(!files.contains_key("src/none.rs"));
    }

    #[test]
    fn a_registry_without_entries_gets_a_marker_and_no_constants() {
        let files = generated(
            &[],
            &[
                ("minecraft:item_modifier", &[]),
                ("minecraft:worldgen/biome", &["minecraft:plains"]),
            ],
        )
        .unwrap();

        assert!(files["src/registry.rs"].contains("pub enum ItemModifier {}"));
        assert!(!files.contains_key("src/item_modifier.rs"));
        assert!(!files["src/lib.rs"].contains("item_modifier"));
        assert!(files.contains_key("src/biome.rs"));
    }

    fn generated_with_tags(
        statics: Statics,
        data: Data,
        tags: &[(&str, &[&str])],
    ) -> Result<Files, String> {
        let (registries, datapack, mut names) = reports(statics, data);
        names.tags = tags
            .iter()
            .map(|(registry, tags)| {
                (
                    (*registry).to_owned(),
                    tags.iter().map(|tag| (*tag).to_owned()).collect(),
                )
            })
            .collect();
        generate(&registries, &datapack, &names)
    }

    #[test]
    fn a_registry_without_tags_gets_no_tag_module() {
        let files = generated_with_tags(
            &[("minecraft:block", BLOCK)],
            &[("minecraft:worldgen/biome", &["minecraft:plains"])],
            &[
                ("minecraft:block", &["minecraft:logs"]),
                ("minecraft:worldgen/biome", &[]),
            ],
        )
        .unwrap();

        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "src/biome.rs",
                "src/block.rs",
                "src/block_tags.rs",
                "src/lib.rs",
                "src/registry.rs"
            ]
        );
        assert!(files["src/lib.rs"].contains("pub mod block_tags;"));
        assert_eq!(
            files["src/block_tags.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.\n\
             \n\
             use mcrs_minecraft_core::{TagKey, rl};\n\
             \n\
             pub const LOGS: TagKey<crate::Block, &'static str> = TagKey::new(rl!(\"minecraft:logs\"));\n"
        );
    }

    #[test]
    fn tag_constants_sort_by_name() {
        let files = generated_with_tags(
            &[],
            &[("minecraft:worldgen/biome", &["minecraft:plains"])],
            &[(
                "minecraft:worldgen/biome",
                &[
                    "minecraft:is_ocean",
                    "minecraft:has_structure/village",
                    "minecraft:allows_surface_slime_spawns",
                ],
            )],
        )
        .unwrap();

        assert_eq!(
            constants(&files, "src/biome_tags.rs"),
            [
                "ALLOWS_SURFACE_SLIME_SPAWNS",
                "HAS_STRUCTURE_VILLAGE",
                "IS_OCEAN"
            ]
        );
        assert!(files["src/biome_tags.rs"].contains(
            "pub const HAS_STRUCTURE_VILLAGE: TagKey<crate::Biome, &'static str> = TagKey::new(rl!(\"minecraft:has_structure/village\"));"
        ));
    }

    #[test]
    fn tags_that_cannot_be_named_stop_generation() {
        let biome: Data = &[("minecraft:worldgen/biome", &["minecraft:plains"])];
        assert_refused(
            generated_with_tags(&[], biome, &[("minecraft:item", &["minecraft:logs"])]),
            &["minecraft:item"],
        );
        assert_refused(
            generated_with_tags(
                &[],
                biome,
                &[(
                    "minecraft:worldgen/biome",
                    &["minecraft:a/b", "minecraft:a_b"],
                )],
            ),
            &["minecraft:a/b", "minecraft:a_b"],
        );
        assert_refused(
            generated_with_tags(
                &[],
                &[
                    ("minecraft:biome", &["minecraft:x"]),
                    ("minecraft:biome_tags", &["minecraft:x"]),
                ],
                &[("minecraft:biome", &["minecraft:logs"])],
            ),
            &["minecraft:biome", "minecraft:biome_tags", "biome_tags"],
        );
    }

    #[test]
    fn the_same_name_in_two_registries_is_two_constants() {
        let files = generated(
            &[
                ("minecraft:block", BLOCK),
                (
                    "minecraft:item",
                    &[
                        ("minecraft:air", 0),
                        ("minecraft:apple", 1),
                        ("minecraft:stone", 2),
                    ],
                ),
            ],
            &[],
        )
        .unwrap();

        assert!(files["src/block.rs"].contains(
            "    crate::Block;\n    AIR = \"minecraft:air\",\n    STONE = \"minecraft:stone\",\n"
        ));
        assert!(files["src/item.rs"].contains(
            "    crate::Item;\n    AIR = \"minecraft:air\",\n    APPLE = \"minecraft:apple\",\n    STONE = \"minecraft:stone\",\n"
        ));
    }

    #[test]
    fn static_constants_follow_protocol_ids() {
        let files = generated(
            &[(
                "minecraft:fruit",
                &[
                    ("minecraft:zebra", 0),
                    ("minecraft:apple", 1),
                    ("minecraft:mango", 2),
                ],
            )],
            &[],
        )
        .unwrap();

        assert_eq!(
            constants(&files, "src/fruit.rs"),
            ["ZEBRA", "APPLE", "MANGO"]
        );
    }

    #[test]
    fn recipe_and_advancement_get_no_constants() {
        let files = generated(
            &[],
            &[
                ("minecraft:advancement", &["minecraft:story/root"]),
                ("minecraft:recipe", &["minecraft:stick"]),
                ("minecraft:worldgen/biome", &["minecraft:plains"]),
            ],
        )
        .unwrap();

        assert!(files["src/registry.rs"].contains("pub enum Advancement {}"));
        assert!(files["src/registry.rs"].contains("pub enum Recipe {}"));
        assert!(!files.contains_key("src/advancement.rs"));
        assert!(!files.contains_key("src/recipe.rs"));
        assert!(!files["src/lib.rs"].contains("advancement"));
        assert!(!files["src/lib.rs"].contains("recipe"));
        assert!(files.contains_key("src/biome.rs"));
    }

    #[test]
    fn registries_are_named_from_their_path() {
        let rows = [
            ("minecraft:block", "block", "Block"),
            (
                "minecraft:dimension_type",
                "dimension_type",
                "DimensionType",
            ),
            (
                "minecraft:worldgen/placed_feature",
                "placed_feature",
                "PlacedFeature",
            ),
        ];
        for (registry, module, marker) in rows {
            let files = generated(&[], &[(registry, &["minecraft:x"])]).unwrap();
            assert!(
                files.contains_key(&format!("src/{module}.rs")),
                "{registry}"
            );
            let markers = &files["src/registry.rs"];
            assert!(
                markers.contains(&format!("pub enum {marker} {{}}")),
                "{markers}"
            );
            assert!(
                markers.contains(&format!("rl!(\"{registry}\")")),
                "{markers}"
            );
        }

        assert_refused(
            generated(
                &[],
                &[
                    ("minecraft:carver", &["minecraft:x"]),
                    ("minecraft:worldgen/carver", &["minecraft:x"]),
                ],
            ),
            &["minecraft:carver", "minecraft:worldgen/carver"],
        );
        assert_refused(
            generated(
                &[],
                &[
                    ("minecraft:a_b", &["minecraft:x"]),
                    ("minecraft:a__b", &["minecraft:x"]),
                ],
            ),
            &["minecraft:a_b", "minecraft:a__b", "AB"],
        );
        assert_refused(
            generated(&[], &[("other:thing", &["minecraft:x"])]),
            &["other:thing"],
        );
    }

    #[test]
    fn reports_that_disagree_stop_generation() {
        let statics: Statics = &[("minecraft:block", BLOCK)];
        let data: Data = &[("minecraft:worldgen/biome", &["minecraft:plains"])];

        let (registries, mut datapack, names) = reports(statics, data);
        datapack.registries.remove("minecraft:block");
        assert_refused(
            generate(&registries, &datapack, &names),
            &["minecraft:block"],
        );

        let (registries, mut datapack, names) = reports(statics, data);
        datapack.registries.remove("minecraft:worldgen/biome");
        assert_refused(
            generate(&registries, &datapack, &names),
            &["minecraft:worldgen/biome"],
        );

        let (registries, datapack, mut names) = reports(statics, data);
        names.entries.remove("minecraft:worldgen/biome");
        assert_refused(
            generate(&registries, &datapack, &names),
            &["minecraft:worldgen/biome"],
        );

        let (registries, datapack, mut names) = reports(statics, data);
        names
            .entries
            .insert("minecraft:block".to_owned(), BTreeSet::new());
        assert_refused(
            generate(&registries, &datapack, &names),
            &["minecraft:block"],
        );

        assert_refused(
            generated(
                &[(
                    "minecraft:block",
                    &[("minecraft:air", 0), ("minecraft:stone", 2)],
                )],
                &[],
            ),
            &["minecraft:block", "minecraft:stone"],
        );
    }

    #[test]
    fn the_keys_crate_is_what_the_generator_writes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reports = root.join("assets/mcrs/reports");
        let registries = registries::read(&reports.join("registries.json")).unwrap();
        let datapack =
            names::Datapack::parse(&fs::read_to_string(reports.join("datapack.json")).unwrap())
                .unwrap();
        let names = names::read(&reports.join("names.json")).unwrap();
        let crate_root = root.join("crates/mcrs_minecraft_keys");

        let files = generate(&registries, &datapack, &names).unwrap();
        for (path, text) in &files {
            match fs::read_to_string(crate_root.join(path)) {
                Ok(held) => assert!(
                    held == *text,
                    "{path} differs from what the generator writes; run `{COMMAND}`"
                ),
                Err(error) => panic!("{path} cannot be read ({error}); run `{COMMAND}`"),
            }
        }
        if let Ok(sources) = fs::read_dir(crate_root.join("src")) {
            for source in sources {
                let name = source.unwrap().file_name().to_string_lossy().into_owned();
                let path = format!("src/{name}");
                assert!(
                    !name.ends_with(".rs") || files.contains_key(&path),
                    "{path} is not written by the generator any more; run `{COMMAND}`"
                );
            }
        }
    }

    #[test]
    fn writing_replaces_changed_files_and_deletes_stale_sources() {
        let root = scratch("keys-write");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\n").unwrap();
        fs::write(root.join("src/stale.rs"), "old").unwrap();
        fs::write(root.join("src/notes.txt"), "kept").unwrap();
        fs::write(root.join("src/lib.rs"), "old").unwrap();

        let files = generated(&[("minecraft:block", BLOCK)], &[]).unwrap();
        write(&root, &files).unwrap();

        for (path, text) in &files {
            assert_eq!(
                &fs::read_to_string(root.join(path)).unwrap(),
                text,
                "{path}"
            );
        }
        assert!(!root.join("src/stale.rs").exists());
        assert_eq!(
            fs::read_to_string(root.join("src/notes.txt")).unwrap(),
            "kept"
        );
        assert_eq!(
            fs::read_to_string(root.join("Cargo.toml")).unwrap(),
            "[package]\n"
        );
    }
}
