use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use mcrs_minecraft_core::ResourceLocation;

use crate::{corpus, names, registries};

pub type Files = BTreeMap<String, String>;

const HEADER: &str = "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n";
const NAMESPACE: &str = "minecraft:";
const NO_CONSTANTS: [&str; 2] = ["minecraft:recipe", "minecraft:advancement"];
const PREFIXES: [(&str, &str); 2] = [("minecraft", ""), ("brigadier", "BRIGADIER_")];
const WORDS: [(&str, &str); 3] = [("5", "FIVE"), ("11", "ELEVEN"), ("13", "THIRTEEN")];

pub struct Owner {
    pub registry: &'static str,
    pub krate: &'static str,
    pub value: ValueType,
}

/// A registry is keyed by a type its owner writes, or a static registry by the
/// enum of its entries the generator writes into the owner's keys module.
pub enum ValueType {
    Defined(&'static str),
    Enum,
}

const KEYS_CRATE: &str = "mcrs_minecraft_keys";
const CATALOG_CRATE: &str = "mcrs_minecraft_registry_catalog";

struct Target {
    krate: String,
    value: String,
    definitions: String,
    bindings: Vec<String>,
    modules: Vec<String>,
    enums: Vec<(String, String)>,
}

impl Target {
    fn new(krate: &str, value: String) -> Self {
        Target {
            krate: krate.to_owned(),
            value,
            definitions: String::new(),
            bindings: Vec::new(),
            modules: Vec::new(),
            enums: Vec::new(),
        }
    }

    fn owned(&self) -> bool {
        self.krate != KEYS_CRATE
    }

    fn source(&self, file: &str) -> String {
        if self.owned() {
            format!("crates/{}/src/keys/{file}", self.krate)
        } else {
            format!("crates/{}/src/{file}", self.krate)
        }
    }

    fn path(&self, item: &str) -> String {
        if self.owned() {
            format!("{}::keys::{item}", self.krate)
        } else {
            format!("{}::{item}", self.krate)
        }
    }
}

pub fn generate(
    registries: &registries::Report,
    datapack: &names::Datapack,
    names: &names::Names,
    owners: &[Owner],
    above_catalog: &BTreeSet<String>,
) -> Result<Files, String> {
    let named = registries
        .keys()
        .chain(names.entries.keys())
        .chain(names.tags.keys())
        .map(String::as_str)
        .chain(owners.iter().map(|owner| owner.registry));
    for registry in named {
        if !datapack.registries.contains_key(registry) {
            return Err(format!("{registry}: not a registry of datapack.json"));
        }
    }
    let mut owner_of: BTreeMap<&str, &Owner> = BTreeMap::new();
    for owner in owners {
        if owner.krate == KEYS_CRATE || owner.krate == CATALOG_CRATE {
            return Err(format!(
                "{}: {} cannot own a registry",
                owner.registry, owner.krate
            ));
        }
        if owner_of.insert(owner.registry, owner).is_some() {
            return Err(format!("{}: owned twice", owner.registry));
        }
    }

    let mut files = Files::new();
    let mut targets: BTreeMap<String, Target> = BTreeMap::new();
    targets.insert(
        KEYS_CRATE.to_owned(),
        Target::new(KEYS_CRATE, String::new()),
    );
    let mut statics = Vec::new();
    let mut claimed: BTreeMap<String, &str> = ["registry", "lib", "keys"]
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

        let report = registries.get(registry);
        let entries = names.entries.get(registry);
        if report.is_some() && entries.is_some() {
            return Err(format!(
                "{registry}: a registry of registries.json also has entries in names.json"
            ));
        }
        if flags.elements && report.is_none() && entries.is_none() {
            return Err(format!("{registry}: names.json lists no entries for it"));
        }

        let enumerated = match owner_of.get(registry.as_str()).map(|owner| &owner.value) {
            Some(ValueType::Enum) if report.is_some_and(|report| !report.entries.is_empty()) => {
                true
            }
            Some(ValueType::Enum) => {
                return Err(format!(
                    "{registry}: only a static registry with entries is keyed by an enum of them"
                ));
            }
            _ => false,
        };
        let (krate, value) = match owner_of.get(registry.as_str()) {
            Some(owner) => match owner.value {
                ValueType::Defined(value) => (owner.krate, value.to_owned()),
                ValueType::Enum => (owner.krate, format!("crate::keys::{marker}")),
            },
            None => (KEYS_CRATE, format!("crate::{marker}")),
        };
        let target = targets
            .entry(krate.to_owned())
            .or_insert_with(|| Target::new(krate, String::new()));
        target.value = value;
        let key = module.to_ascii_uppercase();
        definition_text(target, registry, &marker, &key);
        target.bindings.push(key.clone());

        let text = match (report, entries) {
            (Some(report), _) if !report.entries.is_empty() => {
                if enumerated {
                    target.enums.push((module.clone(), marker.clone()));
                    Some(enum_module(registry, &marker, report)?)
                } else {
                    Some(static_module(registry, &target.value, report)?)
                }
            }
            (None, Some(entries))
                if !entries.is_empty() && !NO_CONSTANTS.contains(&registry.as_str()) =>
            {
                Some(data_module(registry, &target.value, entries)?)
            }
            _ => None,
        };
        if let Some(report) = report {
            if above_catalog.contains(krate) {
                return Err(format!(
                    "{registry}: {krate} owns this static registry and depends on {CATALOG_CRATE}, \
                     so the catalog cannot list it"
                ));
            }
            let entries = if report.entries.is_empty() {
                "&[]".to_owned()
            } else if enumerated {
                target.path(&format!("{marker}::ENTRIES"))
            } else {
                target.path(&format!("{module}::ENTRIES"))
            };
            statics.push(format!(
                "    ({}.location(), {entries}),\n",
                target.path(&key)
            ));
        }
        if let Some(text) = text {
            files.insert(target.source(&format!("{module}.rs")), text);
            target.modules.push(module);
        }
        if let (Some(tags), Some(tag_module)) = (tags, tag_module) {
            files.insert(
                target.source(&format!("{tag_module}.rs")),
                tag_module_text(registry, &target.value, tags)?,
            );
            target.modules.push(tag_module);
        }
    }

    for target in targets.values_mut() {
        target.modules.sort();
        if target.owned() {
            files.insert(target.source("mod.rs"), owner_file(target));
        } else {
            files.insert(target.source("registry.rs"), registry_file(target));
            target.modules.push("registry".to_owned());
            target.modules.sort();
            files.insert(target.source("lib.rs"), lib_file(&target.modules));
        }
    }
    let owning: Vec<&str> = targets
        .keys()
        .map(String::as_str)
        .filter(|krate| !above_catalog.contains(*krate))
        .collect();
    files.insert(
        format!("crates/{CATALOG_CRATE}/src/lib.rs"),
        catalog_file(&statics, &owning),
    );
    files.insert(
        format!("crates/{CATALOG_CRATE}/Cargo.toml"),
        catalog_manifest(&owning),
    );
    Ok(files)
}

/// The owners whose manifest names the catalog: the catalog cannot depend on
/// them, so they bind their own types beside it.
pub fn above_catalog(root: &Path, owners: &[Owner]) -> Result<BTreeSet<String>, String> {
    let mut above = BTreeSet::new();
    for owner in owners {
        let manifest = root.join("crates").join(owner.krate).join("Cargo.toml");
        let text = fs::read_to_string(&manifest).map_err(|error| corpus::io(&manifest, error))?;
        if text
            .lines()
            .any(|line| line.trim_start().starts_with(CATALOG_CRATE))
        {
            above.insert(owner.krate.to_owned());
        }
    }
    Ok(above)
}

pub fn write(root: &Path, files: &Files) -> Result<(), String> {
    let mut written = 0;
    for (path, text) in files {
        written += usize::from(corpus::write_if_changed(root, path, text.as_bytes())?);
    }

    let mut on_disk = Vec::new();
    for dir in generated_dirs(root)? {
        corpus::files_below(root, &dir, &mut on_disk)?;
    }
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

pub fn generated_dirs(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let crates = root.join("crates");
    let mut dirs = vec![
        crates.join(KEYS_CRATE).join("src"),
        crates.join(CATALOG_CRATE).join("src"),
    ];
    let listing = fs::read_dir(&crates).map_err(|error| corpus::io(&crates, error))?;
    for entry in listing {
        let entry = entry.map_err(|error| corpus::io(&crates, error))?;
        let keys = entry.path().join("src").join("keys");
        let module = keys.join("mod.rs");
        if fs::read_to_string(&module).is_ok_and(|text| text.starts_with(HEADER)) {
            dirs.push(keys);
        }
    }
    dirs.retain(|dir| dir.is_dir());
    dirs.sort();
    Ok(dirs)
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

fn definition_text(target: &mut Target, registry: &str, marker: &str, key: &str) {
    let owned = target.owned();
    let out = &mut target.definitions;
    if !out.is_empty() {
        out.push('\n');
    }
    let value = if owned {
        target.value.clone()
    } else {
        out.push_str(&format!(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {marker} {{}}\n"
        ));
        marker.to_owned()
    };
    out.push_str(&format!(
        "pub const {key}: RegistryKey<{value}> = RegistryKey::new(rl!(\"{registry}\"));\n\
         impl Registered for {value} {{\n    \
             const REGISTRY: RegistryKey<Self> = {key};\n\
         }}\n"
    ));
}

fn by_protocol_id<'r>(
    registry: &str,
    report: &'r registries::Registry,
) -> Result<Vec<&'r str>, String> {
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

    Ok(by_id.into_iter().flatten().collect())
}

fn static_module(
    registry: &str,
    value: &str,
    report: &registries::Registry,
) -> Result<String, String> {
    let mut out = format!("{HEADER}\nmcrs_minecraft_registry::static_keys! {{\n    {value};\n");
    let names = by_protocol_id(registry, report)?;
    for (constant, name) in constants(registry, names.into_iter(), &["ENTRIES"])? {
        out.push_str(&format!("    {constant} = \"{name}\",\n"));
    }
    out.push_str("}\n");
    Ok(out)
}

fn enum_module(
    registry: &str,
    marker: &str,
    report: &registries::Registry,
) -> Result<String, String> {
    let mut out =
        format!("{HEADER}\nmcrs_minecraft_registry::static_registry! {{\n    pub enum {marker};\n");
    let names = by_protocol_id(registry, report)?;
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for (constant, name) in constants(registry, names.into_iter(), &[])? {
        let variant = variant(&constant);
        if let Some(other) = seen.insert(variant.clone(), name) {
            return Err(format!(
                "{registry}: {other} and {name} are both the variant {variant}"
            ));
        }
        out.push_str(&format!("    {variant} = \"{name}\",\n"));
    }
    out.push_str("}\n");
    Ok(out)
}

fn variant(constant: &str) -> String {
    constant
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_string() + &chars.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect()
}

fn data_module(registry: &str, value: &str, entries: &BTreeSet<String>) -> Result<String, String> {
    let mut out = format!("{HEADER}\nuse mcrs_minecraft_core::{{ResourceKey, rl}};\n\n");
    for (constant, name) in constants(registry, entries.iter().map(String::as_str), &[])? {
        out.push_str(&format!(
            "pub const {constant}: ResourceKey<{value}, &'static str> = ResourceKey::new(rl!(\"{name}\"));\n"
        ));
    }
    Ok(out)
}

fn tag_module_text(registry: &str, value: &str, tags: &BTreeSet<String>) -> Result<String, String> {
    let mut out = format!("{HEADER}\nuse mcrs_minecraft_core::{{TagKey, rl}};\n\n");
    for (constant, name) in constants(registry, tags.iter().map(String::as_str), &[])? {
        out.push_str(&format!(
            "pub const {constant}: TagKey<{value}, &'static str> = TagKey::new(rl!(\"{name}\"));\n"
        ));
    }
    Ok(out)
}

fn bindings_text(bindings: &[String]) -> String {
    let mut list = String::new();
    for key in bindings {
        list.push_str(&format!("        {key}.binding(),\n"));
    }
    format!(
        "pub fn bindings() -> [TypeBinding; {len}] {{\n    \
             [\n\
         {list}    \
             ]\n\
         }}\n",
        len = bindings.len()
    )
}

fn registry_file(target: &Target) -> String {
    format!(
        "{HEADER}\n\
         use mcrs_minecraft_core::{{RegistryKey, TypeBinding, rl}};\n\
         use mcrs_minecraft_registry::Registered;\n\
         \n\
         {definitions}\n\
         {bindings}",
        definitions = target.definitions,
        bindings = bindings_text(&target.bindings),
    )
}

fn owner_file(target: &Target) -> String {
    let mut out = format!("{HEADER}\n");
    for module in &target.modules {
        out.push_str(&format!("pub mod {module};\n"));
    }
    if !target.modules.is_empty() {
        out.push('\n');
    }
    for (module, marker) in &target.enums {
        out.push_str(&format!("pub use {module}::{marker};\n"));
    }
    if !target.enums.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!(
        "use mcrs_minecraft_core::{{RegistryKey, TypeBinding, rl}};\n\
         use mcrs_minecraft_registry::Registered;\n\
         \n\
         {definitions}\n\
         {bindings}",
        definitions = target.definitions,
        bindings = bindings_text(&target.bindings),
    ));
    out
}

fn lib_file(modules: &[String]) -> String {
    let mut out = format!("{HEADER}\n");
    for module in modules {
        out.push_str(&format!("#[rustfmt::skip]\npub mod {module};\n"));
    }
    out.push_str("\npub use registry::*;\n");
    out
}

fn catalog_file(statics: &[String], owning: &[&str]) -> String {
    let mut out = format!(
        "{HEADER}\n\
         use mcrs_minecraft_core::{{StaticResourceLocation, TypeBinding}};\n\
         \n\
         #[rustfmt::skip]\n\
         pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[\n"
    );
    for line in statics {
        out.push_str(line);
    }
    out.push_str(
        "];\n\
         \n\
         #[rustfmt::skip]\n\
         pub fn bindings() -> impl Iterator<Item = TypeBinding> {\n    \
             std::iter::empty()\n",
    );
    for krate in owning {
        let path = if *krate == KEYS_CRATE {
            format!("{krate}::bindings()")
        } else {
            format!("{krate}::keys::bindings()")
        };
        out.push_str(&format!("        .chain({path})\n"));
    }
    out.push_str("}\n");
    out
}

fn catalog_manifest(owning: &[&str]) -> String {
    let mut out = format!(
        "{}\n\
         [package]\n\
         name = \"{CATALOG_CRATE}\"\n\
         description = \"The static registries and the registry types of every crate that owns one\"\n\
         version.workspace = true\n\
         edition.workspace = true\n\
         \n\
         [lib]\n\
         doctest = false\n\
         \n\
         [dependencies]\n\
         mcrs_minecraft_core.workspace = true\n",
        HEADER.replacen("//", "#", 1).trim_end()
    );
    for krate in owning {
        out.push_str(&format!("{krate}.workspace = true\n"));
    }
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
    const COMMAND: &str = "cargo run -p mcrs_minecraft_update -- keys";

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

    const CATALOG_LIB: &str = "crates/mcrs_minecraft_registry_catalog/src/lib.rs";
    const CATALOG_MANIFEST: &str = "crates/mcrs_minecraft_registry_catalog/Cargo.toml";

    fn in_keys_crate(files: Files) -> Files {
        files
            .into_iter()
            .map(
                |(path, text)| match path.strip_prefix("crates/mcrs_minecraft_keys/") {
                    Some(path) => (path.to_owned(), text),
                    None => (path, text),
                },
            )
            .collect()
    }

    fn owned(statics: Statics, data: Data, owners: &[Owner]) -> Result<Files, String> {
        let (registries, datapack, names) = reports(statics, data);
        generate(&registries, &datapack, &names, owners, &BTreeSet::new()).map(in_keys_crate)
    }

    fn generated(statics: Statics, data: Data) -> Result<Files, String> {
        owned(statics, data, &[])
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
                CATALOG_MANIFEST,
                CATALOG_LIB,
                "src/biome.rs",
                "src/block.rs",
                "src/lib.rs",
                "src/registry.rs"
            ]
        );
        assert_eq!(
            files[CATALOG_LIB],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             use mcrs_minecraft_core::{StaticResourceLocation, TypeBinding};\n\
             \n\
             #[rustfmt::skip]\n\
             pub const STATIC_REGISTRIES: &[(StaticResourceLocation, &[StaticResourceLocation])] = &[\n\
             \x20   (mcrs_minecraft_keys::BLOCK.location(), mcrs_minecraft_keys::block::ENTRIES),\n\
             ];\n\
             \n\
             #[rustfmt::skip]\n\
             pub fn bindings() -> impl Iterator<Item = TypeBinding> {\n\
             \x20   std::iter::empty()\n\
             \x20       .chain(mcrs_minecraft_keys::bindings())\n\
             }\n"
        );
        assert_eq!(
            files["src/lib.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             #[rustfmt::skip]\n\
             pub mod biome;\n\
             #[rustfmt::skip]\n\
             pub mod block;\n\
             #[rustfmt::skip]\n\
             pub mod registry;\n\
             \n\
             pub use registry::*;\n"
        );
        assert_eq!(
            files["src/registry.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};\n\
             use mcrs_minecraft_registry::Registered;\n\
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
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             mcrs_minecraft_registry::static_keys! {\n\
             \x20   crate::Block;\n\
             \x20   AIR = \"minecraft:air\",\n\
             \x20   STONE = \"minecraft:stone\",\n\
             }\n"
        );
        assert_eq!(
            files["src/biome.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
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

        assert!(files[CATALOG_LIB].contains("    (mcrs_minecraft_keys::NONE.location(), &[]),\n"));
        assert!(files[CATALOG_LIB].contains(
            "    (mcrs_minecraft_keys::BLOCK.location(), mcrs_minecraft_keys::block::ENTRIES),\n"
        ));
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
        generate(&registries, &datapack, &names, &[], &BTreeSet::new()).map(in_keys_crate)
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
                CATALOG_MANIFEST,
                CATALOG_LIB,
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
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
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
            generate(&registries, &datapack, &names, &[], &BTreeSet::new()),
            &["minecraft:block"],
        );

        let (registries, mut datapack, names) = reports(statics, data);
        datapack.registries.remove("minecraft:worldgen/biome");
        assert_refused(
            generate(&registries, &datapack, &names, &[], &BTreeSet::new()),
            &["minecraft:worldgen/biome"],
        );

        let (registries, datapack, mut names) = reports(statics, data);
        names.entries.remove("minecraft:worldgen/biome");
        assert_refused(
            generate(&registries, &datapack, &names, &[], &BTreeSet::new()),
            &["minecraft:worldgen/biome"],
        );

        let (registries, datapack, mut names) = reports(statics, data);
        names
            .entries
            .insert("minecraft:block".to_owned(), BTreeSet::new());
        assert_refused(
            generate(&registries, &datapack, &names, &[], &BTreeSet::new()),
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

    fn tag_file<R>(
        registry: mcrs_minecraft_core::RegistryKey<R>,
        tag: mcrs_minecraft_core::TagKey<R, &'static str>,
    ) -> String {
        let location = tag.resource_location();
        format!(
            "{}/tags/{}/{}.json",
            location.namespace(),
            registry.path(),
            location.path()
        )
    }

    #[test]
    fn generated_tags_name_their_shipped_files() {
        use mcrs_minecraft_biome::keys::{BIOME, biome_tags};
        use mcrs_minecraft_keys::{BLOCK, ITEM, block_tags, item_tags};

        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let rows = [
            (
                tag_file(BLOCK, block_tags::MINEABLE_PICKAXE),
                "minecraft/tags/block/mineable/pickaxe.json",
            ),
            (
                tag_file(BLOCK, block_tags::LOGS),
                "minecraft/tags/block/logs.json",
            ),
            (
                tag_file(ITEM, item_tags::LOGS),
                "minecraft/tags/item/logs.json",
            ),
            (
                tag_file(BIOME, biome_tags::IS_OCEAN),
                "minecraft/tags/worldgen/biome/is_ocean.json",
            ),
        ];
        for (asset_path, expected) in rows {
            assert_eq!(asset_path, expected);
            let file = corpus.join("minecraft").join(
                asset_path
                    .strip_prefix("minecraft/")
                    .expect("a tag of the minecraft namespace"),
            );
            assert!(file.is_file(), "{} is not shipped", file.display());
        }
    }

    #[test]
    fn the_key_sources_are_what_the_generator_writes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reports = root.join("assets/mcrs/reports");
        let registries = registries::read(&reports.join("registries.json")).unwrap();
        let datapack =
            names::Datapack::parse(&fs::read_to_string(reports.join("datapack.json")).unwrap())
                .unwrap();
        let names = names::read(&reports.join("names.json")).unwrap();

        let files = generate(
            &registries,
            &datapack,
            &names,
            crate::owners::OWNERS,
            &above_catalog(&root, crate::owners::OWNERS).unwrap(),
        )
        .unwrap();
        for (path, text) in &files {
            match fs::read_to_string(root.join(path)) {
                Ok(held) => assert!(
                    held == *text,
                    "{path} differs from what the generator writes; run `{COMMAND}`"
                ),
                Err(error) => panic!("{path} cannot be read ({error}); run `{COMMAND}`"),
            }
        }
        let mut on_disk = Vec::new();
        for dir in generated_dirs(&root).unwrap() {
            corpus::files_below(&root, &dir, &mut on_disk).unwrap();
        }
        for path in on_disk {
            assert!(
                !path.ends_with(".rs") || files.contains_key(&path),
                "{path} is not written by the generator any more; run `{COMMAND}`"
            );
        }
    }

    #[test]
    fn an_owned_registry_is_keyed_in_its_owner_and_bound_by_the_catalog() {
        let files = owned(
            &[("minecraft:block", BLOCK)],
            &[("minecraft:worldgen/biome", &["minecraft:plains"])],
            &[Owner {
                registry: "minecraft:worldgen/biome",
                krate: "mcrs_minecraft_biome",
                value: ValueType::Defined("crate::Biome"),
            }],
        )
        .unwrap();

        assert!(!files["src/registry.rs"].contains("Biome"));
        assert!(!files.contains_key("src/biome.rs"));
        assert_eq!(
            files["crates/mcrs_minecraft_biome/src/keys/mod.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             pub mod biome;\n\
             \n\
             use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};\n\
             use mcrs_minecraft_registry::Registered;\n\
             \n\
             pub const BIOME: RegistryKey<crate::Biome> = RegistryKey::new(rl!(\"minecraft:worldgen/biome\"));\n\
             impl Registered for crate::Biome {\n\
             \x20   const REGISTRY: RegistryKey<Self> = BIOME;\n\
             }\n\
             \n\
             pub fn bindings() -> [TypeBinding; 1] {\n\
             \x20   [\n\
             \x20       BIOME.binding(),\n\
             \x20   ]\n\
             }\n"
        );
        assert!(
            files["crates/mcrs_minecraft_biome/src/keys/biome.rs"].contains(
                "pub const PLAINS: ResourceKey<crate::Biome, &'static str> = ResourceKey::new(rl!(\"minecraft:plains\"));"
            )
        );
        assert!(
            files[CATALOG_LIB].contains("        .chain(mcrs_minecraft_biome::keys::bindings())\n")
        );
        assert!(files[CATALOG_MANIFEST].contains("\nmcrs_minecraft_biome.workspace = true\n"));
    }

    #[test]
    fn an_owned_static_registry_is_an_enum_of_its_entries() {
        let files = owned(
            &[(
                "minecraft:fruit",
                &[
                    ("minecraft:zebra", 0),
                    ("minecraft:apple", 1),
                    ("minecraft:music_disc_5", 2),
                ],
            )],
            &[],
            &[Owner {
                registry: "minecraft:fruit",
                krate: "mcrs_minecraft_food",
                value: ValueType::Enum,
            }],
        )
        .unwrap();

        assert_eq!(
            files["crates/mcrs_minecraft_food/src/keys/fruit.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             mcrs_minecraft_registry::static_registry! {\n\
             \x20   pub enum Fruit;\n\
             \x20   Zebra = \"minecraft:zebra\",\n\
             \x20   Apple = \"minecraft:apple\",\n\
             \x20   MusicDisc5 = \"minecraft:music_disc_5\",\n\
             }\n"
        );
        let module = &files["crates/mcrs_minecraft_food/src/keys/mod.rs"];
        assert!(module.contains("pub mod fruit;\n\npub use fruit::Fruit;\n"));
        assert!(module.contains("impl Registered for crate::keys::Fruit {"));
        assert!(files[CATALOG_LIB].contains(
            "    (mcrs_minecraft_food::keys::FRUIT.location(), mcrs_minecraft_food::keys::Fruit::ENTRIES),\n"
        ));
    }

    #[test]
    fn only_a_static_registry_with_entries_is_an_enum() {
        let owner = |registry, value| Owner {
            registry,
            krate: "mcrs_minecraft_food",
            value,
        };
        let statics: Statics = &[("minecraft:none", &[]), ("minecraft:block", BLOCK)];
        let data: Data = &[("minecraft:worldgen/biome", &["minecraft:plains"])];
        for (registry, value) in [
            ("minecraft:none", ValueType::Enum),
            ("minecraft:worldgen/biome", ValueType::Enum),
        ] {
            assert_refused(owned(statics, data, &[owner(registry, value)]), &[registry]);
        }
    }

    #[test]
    fn two_entries_of_one_variant_name_stop_generation() {
        let result = owned(
            &[(
                "minecraft:fruit",
                &[("minecraft:foo_1_2", 0), ("minecraft:foo_12", 1)],
            )],
            &[],
            &[Owner {
                registry: "minecraft:fruit",
                krate: "mcrs_minecraft_food",
                value: ValueType::Enum,
            }],
        );
        assert_refused(result, &["minecraft:foo_1_2", "minecraft:foo_12", "Foo12"]);
    }

    #[test]
    fn an_owner_that_depends_on_the_catalog_is_left_out_of_it() {
        let (registries, datapack, names) = reports(
            &[("minecraft:block", BLOCK)],
            &[("minecraft:chat_type", &["minecraft:chat"])],
        );
        let owner = |registry, value| Owner {
            registry,
            krate: "mcrs_minecraft_world",
            value,
        };
        let above = BTreeSet::from(["mcrs_minecraft_world".to_owned()]);

        let files = generate(
            &registries,
            &datapack,
            &names,
            &[owner(
                "minecraft:chat_type",
                ValueType::Defined("crate::ChatType"),
            )],
            &above,
        )
        .unwrap();
        assert!(files.contains_key("crates/mcrs_minecraft_world/src/keys/mod.rs"));
        assert!(!files[CATALOG_LIB].contains("mcrs_minecraft_world"));
        assert!(!files[CATALOG_MANIFEST].contains("mcrs_minecraft_world"));

        assert_refused(
            generate(
                &registries,
                &datapack,
                &names,
                &[owner("minecraft:block", ValueType::Enum)],
                &above,
            ),
            &["minecraft:block", "mcrs_minecraft_world"],
        );
    }

    #[test]
    fn an_owner_of_an_unknown_registry_stops_generation() {
        let owner = |registry| Owner {
            registry,
            krate: "mcrs_minecraft_biome",
            value: ValueType::Defined("crate::Biome"),
        };
        let data: Data = &[("minecraft:worldgen/biome", &["minecraft:plains"])];
        assert_refused(
            owned(&[], data, &[owner("minecraft:nothing")]),
            &["minecraft:nothing"],
        );
        assert_refused(
            owned(
                &[],
                data,
                &[
                    owner("minecraft:worldgen/biome"),
                    owner("minecraft:worldgen/biome"),
                ],
            ),
            &["minecraft:worldgen/biome"],
        );
    }

    #[test]
    fn writing_replaces_changed_files_and_deletes_stale_sources() {
        let root = scratch("keys-write");
        let keys = root.join("crates/mcrs_minecraft_keys");
        let owner = root.join("crates/mcrs_minecraft_gone/src/keys");
        fs::create_dir_all(keys.join("src")).unwrap();
        fs::create_dir_all(&owner).unwrap();
        fs::write(keys.join("Cargo.toml"), "[package]\n").unwrap();
        fs::write(keys.join("src/stale.rs"), "old").unwrap();
        fs::write(keys.join("src/notes.txt"), "kept").unwrap();
        fs::write(keys.join("src/lib.rs"), "old").unwrap();
        fs::write(owner.join("mod.rs"), HEADER).unwrap();
        fs::write(owner.join("gone.rs"), HEADER).unwrap();

        let (registries, datapack, names) = reports(&[("minecraft:block", BLOCK)], &[]);
        let files = generate(&registries, &datapack, &names, &[], &BTreeSet::new()).unwrap();
        write(&root, &files).unwrap();

        for (path, text) in &files {
            assert_eq!(
                &fs::read_to_string(root.join(path)).unwrap(),
                text,
                "{path}"
            );
        }
        assert!(!keys.join("src/stale.rs").exists());
        assert!(!owner.join("mod.rs").exists());
        assert!(!owner.join("gone.rs").exists());
        assert_eq!(
            fs::read_to_string(keys.join("src/notes.txt")).unwrap(),
            "kept"
        );
        assert_eq!(
            fs::read_to_string(keys.join("Cargo.toml")).unwrap(),
            "[package]\n"
        );
    }
}
