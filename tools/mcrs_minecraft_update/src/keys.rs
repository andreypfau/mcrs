use mcrs_minecraft_resource::ResourceLocationBuf;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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
    Enum(&'static str),
}

const KEYS_CRATE: &str = "mcrs_minecraft_keys";
const CATALOG_CRATE: &str = "mcrs_minecraft_registry_catalog";

/// The module of an owning crate that holds its registry keys, beside the `<registry>_keys`
/// and `<registry>_tag_keys` modules of the entries.
const REGISTRY_KEYS: &str = "registry_keys";

struct Target {
    krate: String,
    definitions: String,
    modules: Vec<String>,
    statics: Vec<(u16, String)>,
}

impl Target {
    fn new(krate: &str) -> Self {
        Target {
            krate: krate.to_owned(),
            definitions: String::new(),
            modules: Vec::new(),
            statics: Vec::new(),
        }
    }

    fn owned(&self) -> bool {
        self.krate != KEYS_CRATE
    }

    fn entries_module(&self, module: &str) -> String {
        if self.owned() {
            format!("{module}_keys")
        } else {
            module.to_owned()
        }
    }

    fn tags_module(&self, module: &str) -> String {
        if self.owned() {
            format!("{module}_tag_keys")
        } else {
            format!("{module}_tags")
        }
    }

    fn source(&self, module: &str) -> String {
        format!("crates/{}/src/{module}.rs", self.krate)
    }

    fn path(&self, item: &str) -> String {
        format!("{}::{item}", self.krate)
    }

    fn key_path(&self, key: &str) -> String {
        if self.owned() {
            self.path(&format!("{REGISTRY_KEYS}::{key}"))
        } else {
            self.path(key)
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "checks the owners, then builds every registry's key module and the owning crates' files in one pass, so each error reads against its check"
)]
pub fn generate(
    registries: &registries::Report,
    datapack: &names::Datapack,
    names: &names::Names,
    owners: &[Owner],
    above_catalog: &BTreeSet<String>,
    own_lists: &BTreeSet<String>,
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
    targets.insert(KEYS_CRATE.to_owned(), Target::new(KEYS_CRATE));
    let mut statics: Vec<(u16, String)> = Vec::new();
    let mut static_owners: BTreeSet<String> = BTreeSet::new();
    let mut claimed: BTreeMap<String, &str> = ["registry", "lib", "keys"]
        .into_iter()
        .map(|name| (name.to_owned(), "src"))
        .collect();
    for (registry, flags) in &datapack.registries {
        let module = identity(registry)?;
        let tags = names.tags.get(registry).filter(|tags| !tags.is_empty());
        let tag_claims = tags.map(|_| [format!("{module}_tag"), format!("{module}_tags")]);
        for claim in std::iter::once(&module).chain(tag_claims.iter().flatten()) {
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

        let owner = owner_of.get(registry.as_str());
        let static_entries = report.is_some_and(|report| !report.entries.is_empty());
        let enumerated = match owner.map(|owner| &owner.value) {
            Some(ValueType::Enum(name)) if static_entries => Some(*name),
            Some(ValueType::Enum(_)) => {
                return Err(format!(
                    "{registry}: only a static registry with entries is keyed by an enum of them"
                ));
            }
            Some(ValueType::Defined(_)) if static_entries => {
                return Err(format!(
                    "{registry}: a static registry with entries is keyed by an enum of them"
                ));
            }
            _ => None,
        };
        let krate = owner.map_or(KEYS_CRATE, |owner| owner.krate);
        let value = owner.map(|owner| match owner.value {
            ValueType::Defined(value) => value.to_owned(),
            ValueType::Enum(name) => format!("crate::{module}_keys::{name}"),
        });
        let value = value.as_deref();
        let target = targets
            .entry(krate.to_owned())
            .or_insert_with(|| Target::new(krate));
        let key = module.to_ascii_uppercase();
        definition_text(&mut target.definitions, registry, &key, value);

        let text = match (report, entries) {
            (Some(report), _) if !report.entries.is_empty() => {
                if let Some(name) = enumerated {
                    Some(enum_module(registry, name, report)?)
                } else {
                    Some(static_names_module(registry, report)?)
                }
            }
            (None, Some(entries))
                if !entries.is_empty() && !NO_CONSTANTS.contains(&registry.as_str()) =>
            {
                Some(named_constants(registry, value, "ResourceKey", entries)?)
            }
            _ => None,
        };
        if let Some(report) = report {
            if above_catalog.contains(krate) && !own_lists.contains(krate) {
                return Err(format!(
                    "{registry}: {krate} owns this static registry and depends on {CATALOG_CRATE}, \
                     so the catalog cannot list it"
                ));
            }
            let entries_module = target.entries_module(&module);
            let entries = match enumerated {
                _ if report.entries.is_empty() => None,
                Some(name) => Some(format!("{entries_module}::{name}::ENTRIES")),
                None => Some(format!("{entries_module}::ENTRIES")),
            };
            if own_lists.contains(krate) {
                let entries =
                    entries.map_or_else(|| "&[]".to_owned(), |entries| format!("crate::{entries}"));
                target.statics.push((
                    report.protocol_id,
                    format!("    ({key}.location(), {entries}),\n"),
                ));
            } else {
                let entries =
                    entries.map_or_else(|| "&[]".to_owned(), |entries| target.path(&entries));
                let location = if target.owned() {
                    format!("{}.location()", target.key_path(&key))
                } else {
                    target.key_path(&key)
                };
                statics.push((
                    report.protocol_id,
                    format!("    ({location}, {entries}),\n"),
                ));
                static_owners.insert(krate.to_owned());
            }
        }
        if let Some(text) = text {
            let entries_module = target.entries_module(&module);
            files.insert(target.source(&entries_module), text);
            target.modules.push(entries_module);
        }
        if let Some(tags) = tags {
            let tags_module = target.tags_module(&module);
            files.insert(
                target.source(&tags_module),
                named_constants(registry, value, "TagKey", tags)?,
            );
            target.modules.push(tags_module);
        }
    }

    for target in targets.values_mut() {
        target.modules.sort();
        if target.owned() {
            files.insert(target.source(REGISTRY_KEYS), registry_keys_file(target));
        } else {
            files.insert(target.source("registry"), registry_file(target));
            target.modules.push("registry".to_owned());
            target.modules.sort();
            files.insert(target.source("lib"), lib_file(&target.modules));
        }
    }
    statics.sort_by_key(|&(protocol_id, _)| protocol_id);
    let statics: Vec<String> = statics.into_iter().map(|(_, line)| line).collect();
    files.insert(
        format!("crates/{CATALOG_CRATE}/src/lib.rs"),
        catalog_file(&statics),
    );
    files.insert(
        format!("crates/{CATALOG_CRATE}/Cargo.toml"),
        catalog_manifest(&static_owners),
    );
    for (path, text) in &mut files {
        if path.ends_with(".rs") {
            *text = rustfmt(path, text)?;
        }
    }
    Ok(files)
}

/// rustfmt checks a file given by path without its parent module, so a
/// `#[rustfmt::skip]` there cannot shield a generated source: it is written
/// formatted.
pub fn rustfmt(path: &str, text: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{path}: rustfmt: {error}"))?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| format!("{path}: rustfmt has no stdin"))
        .and_then(|mut stdin| {
            stdin
                .write_all(text.as_bytes())
                .map_err(|error| format!("{path}: rustfmt: {error}"))
        });
    let output = child
        .wait_with_output()
        .map_err(|error| format!("{path}: rustfmt: {error}"))?;
    written?;
    if !output.status.success() {
        return Err(format!(
            "{path}: rustfmt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("{path}: rustfmt: {error}"))
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
        if !holds(root, path, text) {
            written += usize::from(corpus::write_if_changed(root, path, text.as_bytes())?);
        }
    }

    let stale = stale(root, files)?;
    for path in &stale {
        let full = root.join(path);
        fs::remove_file(&full).map_err(|error| corpus::io(&full, error))?;
        println!("deleted {path}");
    }
    println!("keys: {written} written, {} deleted", stale.len());
    let undeclared = undeclared(root, files)?;
    if undeclared.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "an owning crate does not declare a key module it holds:\n{}",
            undeclared.join("\n")
        ))
    }
}

/// The `pub mod` lines an owning crate's `lib.rs` lacks for the key modules written into it.
/// The generator writes the modules but never the hand-written `lib.rs`.
fn undeclared(root: &Path, files: &Files) -> Result<Vec<String>, String> {
    let mut missing = Vec::new();
    let mut libs: BTreeMap<String, String> = BTreeMap::new();
    for path in files.keys() {
        let Some((krate, module)) = owned_module(path) else {
            continue;
        };
        let lib = format!("crates/{krate}/src/lib.rs");
        let text = match libs.get(&lib) {
            Some(text) => text,
            None => {
                let full = root.join(&lib);
                let text = fs::read_to_string(&full).map_err(|error| corpus::io(&full, error))?;
                libs.entry(lib.clone()).or_insert(text)
            }
        };
        let declaration = format!("pub mod {module};");
        if !text.lines().any(|line| line.trim() == declaration) {
            missing.push(format!("{lib}: add `{declaration}`"));
        }
    }
    Ok(missing)
}

/// The crate and module of a key source the generator writes into an owning crate.
fn owned_module(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("crates/")?;
    let (krate, file) = rest.split_once("/src/")?;
    let module = file.strip_suffix(".rs")?;
    let owned = krate != KEYS_CRATE && krate != CATALOG_CRATE;
    (owned && !module.contains('/') && module.ends_with("_keys")).then_some((krate, module))
}

/// A source written before the generator formatted its output is kept while
/// only its layout differs, so a regeneration touches only changed content.
fn holds(root: &Path, path: &str, text: &str) -> bool {
    let Ok(held) = fs::read_to_string(root.join(path)) else {
        return false;
    };
    if held == text {
        return true;
    }
    if !path.ends_with(".rs") {
        return false;
    }
    rustfmt(path, &unskipped(&held)).is_ok_and(|formatted| formatted == unskipped(text))
}

fn unskipped(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| line.trim_end() != "#[rustfmt::skip]")
        .collect()
}

fn stale(root: &Path, files: &Files) -> Result<Vec<String>, String> {
    let crates = root.join("crates");
    let mut on_disk = Vec::new();
    for krate in [KEYS_CRATE, CATALOG_CRATE] {
        let dir = crates.join(krate).join("src");
        if dir.is_dir() {
            corpus::files_below(root, &dir, &mut on_disk)?;
        }
    }
    let listing = fs::read_dir(&crates).map_err(|error| corpus::io(&crates, error))?;
    for entry in listing {
        let entry = entry.map_err(|error| corpus::io(&crates, error))?;
        let src = entry.path().join("src");
        let Ok(sources) = fs::read_dir(&src) else {
            continue;
        };
        for source in sources {
            let path = source.map_err(|error| corpus::io(&src, error))?.path();
            let generated = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_keys.rs"))
                && fs::read_to_string(&path).is_ok_and(|text| text.starts_with(HEADER));
            if let (true, Ok(relative)) = (generated, path.strip_prefix(root)) {
                on_disk.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    on_disk.retain(|path| path.ends_with(".rs") && !files.contains_key(path));
    on_disk.sort();
    on_disk.dedup();
    Ok(on_disk)
}

fn identity(registry: &str) -> Result<String, String> {
    let location = location(registry, registry)?;
    let path = location
        .as_str()
        .strip_prefix(NAMESPACE)
        .ok_or_else(|| format!("{registry}: not a registry of the minecraft namespace"))?;
    let module = path
        .strip_prefix("worldgen/")
        .unwrap_or(path)
        .replace('/', "_");
    let valid = module.starts_with(|first: char| first.is_ascii_lowercase())
        && module
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        return Err(format!("{registry}: no module name can be made of it"));
    }
    Ok(module)
}

fn location(context: &str, name: &str) -> Result<ResourceLocationBuf, String> {
    let location =
        ResourceLocationBuf::read(name).map_err(|error| format!("{context}: {error}"))?;
    if location.as_str() != name {
        return Err(format!(
            "{context}: {name} is not written as namespace:path"
        ));
    }
    Ok(location)
}

pub fn constant(registry: &str, name: &str) -> Result<String, String> {
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

fn definition_text(out: &mut String, registry: &str, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!(
            "pub const {key}: RegistryKey<{value}> = RegistryKey::new(rl!(\"{registry}\"));\n\
             impl Registered for {value} {{\n    \
                 const REGISTRY: RegistryKey<Self> = {key};\n\
             }}\n"
        ));
    } else {
        out.push_str(&format!(
            "pub const {key}: ResourceLocation<'static> = rl!(\"{registry}\");\n"
        ));
    }
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

/// The entries of a static registry no crate keys by a type: one location
/// per entry and the list of them by protocol id.
fn static_names_module(registry: &str, report: &registries::Registry) -> Result<String, String> {
    let mut out = format!("{HEADER}\nuse mcrs_minecraft_resource::{{ResourceLocation, rl}};\n\n");
    let names = by_protocol_id(registry, report)?;
    let constants = constants(registry, names.into_iter(), &["ENTRIES"])?;
    for (constant, name) in &constants {
        out.push_str(&format!(
            "pub const {constant}: ResourceLocation<'static> = rl!(\"{name}\");\n"
        ));
    }
    out.push_str("\npub const ENTRIES: &[ResourceLocation<'static>] = &[\n");
    for (constant, _) in &constants {
        out.push_str(&format!("    {constant},\n"));
    }
    out.push_str("];\n");
    Ok(out)
}

fn enum_module(
    registry: &str,
    marker: &str,
    report: &registries::Registry,
) -> Result<String, String> {
    let mut out = format!(
        "{HEADER}\nmcrs_minecraft_registries::static_registry! {{\n    pub enum {marker};\n"
    );
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

pub fn variant(constant: &str) -> String {
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

fn named_constants(
    registry: &str,
    value: Option<&str>,
    key: &str,
    names: &BTreeSet<String>,
) -> Result<String, String> {
    let ty = value.map_or_else(
        || "ResourceLocation<'static>".to_owned(),
        |value| format!("{key}<'static, {value}>"),
    );
    let import = if value.is_some() {
        key
    } else {
        "ResourceLocation"
    };
    let mut out = format!("{HEADER}\nuse mcrs_minecraft_resource::{{{import}, rl}};\n\n");
    for (constant, name) in constants(registry, names.iter().map(String::as_str), &[])? {
        let init = match value {
            Some(_) => format!("{key}::new(rl!(\"{name}\"))"),
            None => format!("rl!(\"{name}\")"),
        };
        out.push_str(&format!("pub const {constant}: {ty} = {init};\n"));
    }
    Ok(out)
}

fn registry_file(target: &Target) -> String {
    format!(
        "{HEADER}\n\
         use mcrs_minecraft_resource::{{ResourceLocation, rl}};\n\
         \n\
         {definitions}",
        definitions = target.definitions,
    )
}

fn registry_keys_file(target: &Target) -> String {
    let mut out = format!("{HEADER}\n");
    let mut statics = target.statics.clone();
    statics.sort_by_key(|&(protocol_id, _)| protocol_id);
    let imports = match statics.is_empty() {
        true => "Registered, RegistryKey, rl",
        false => "Registered, RegistryKey, ResourceLocation, rl",
    };
    out.push_str(&format!(
        "use mcrs_minecraft_resource::{{{imports}}};\n\
         \n\
         {definitions}",
        definitions = target.definitions,
    ));
    if !statics.is_empty() {
        out.push_str(
            "\n/// The static registries of this crate, which the catalog does not list.\n\
             pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[\n",
        );
        for (_, line) in statics {
            out.push_str(&line);
        }
        out.push_str("];\n");
    }
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

fn catalog_file(statics: &[String]) -> String {
    let mut out = format!(
        "{HEADER}\n\
         use mcrs_minecraft_resource::ResourceLocation;\n\
         \n\
         pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[\n"
    );
    for line in statics {
        out.push_str(line);
    }
    out.push_str("];\n");
    out
}

fn catalog_manifest(static_owners: &BTreeSet<String>) -> String {
    let mut out = format!(
        "{}\n\
         [package]\n\
         name = \"{CATALOG_CRATE}\"\n\
         description = \"The entries of every static registry\"\n\
         version.workspace = true\n\
         edition.workspace = true\n\
         \n\
         [lib]\n\
         doctest = false\n\
         \n\
         [lints]\n\
         workspace = true\n\
         \n\
         [dependencies]\n\
         mcrs_minecraft_resource.workspace = true\n",
        HEADER.replacen("//", "#", 1).trim_end()
    );
    for krate in static_owners {
        out.push_str(&format!("{krate}.workspace = true\n"));
    }
    out.push_str("\n[dev-dependencies]\nserde.workspace = true\nserde_json.workspace = true\n");
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use serde_json::{Map, Value, json};

    use super::*;
    use crate::testing::scratch;

    type Statics<'a> = &'a [(&'a str, &'a [(&'a str, u16)])];
    type Data<'a> = &'a [(&'a str, &'a [&'a str])];
    type Reports = (registries::Report, names::Datapack, names::Names);
    type Refusal<'a> = (Statics<'a>, Data<'a>, &'a [&'a str]);

    const BLOCK: &[(&str, u16)] = &[("minecraft:air", 0), ("minecraft:stone", 1)];

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
        generate(
            &registries,
            &datapack,
            &names,
            owners,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .map(in_keys_crate)
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
            .filter(|name| name != "ENTRIES")
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
             use mcrs_minecraft_resource::ResourceLocation;\n\
             \n\
             pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[(\n\
             \x20   mcrs_minecraft_keys::BLOCK,\n\
             \x20   mcrs_minecraft_keys::block::ENTRIES,\n\
             )];\n"
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
             use mcrs_minecraft_resource::{ResourceLocation, rl};\n\
             \n\
             pub const BLOCK: ResourceLocation<'static> = rl!(\"minecraft:block\");\n\
             pub const BIOME: ResourceLocation<'static> = rl!(\"minecraft:worldgen/biome\");\n"
        );
        assert_eq!(
            files["src/block.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             use mcrs_minecraft_resource::{ResourceLocation, rl};\n\
             \n\
             pub const AIR: ResourceLocation<'static> = rl!(\"minecraft:air\");\n\
             pub const STONE: ResourceLocation<'static> = rl!(\"minecraft:stone\");\n\
             \n\
             pub const ENTRIES: &[ResourceLocation<'static>] = &[AIR, STONE];\n"
        );
        assert_eq!(
            files["src/biome.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             use mcrs_minecraft_resource::{ResourceLocation, rl};\n\
             \n\
             pub const PLAINS: ResourceLocation<'static> = rl!(\"minecraft:plains\");\n"
        );
    }

    #[test]
    fn the_catalog_lists_static_registries_in_the_order_of_the_registry_table() {
        let statics: Statics = &[("minecraft:sound_event", BLOCK), ("minecraft:block", BLOCK)];

        let files = generated(statics, &[]).unwrap();
        let catalog = &files[CATALOG_LIB];
        let sound_event = catalog.find("mcrs_minecraft_keys::SOUND_EVENT").unwrap();
        let block = catalog.find("mcrs_minecraft_keys::BLOCK").unwrap();
        assert!(sound_event < block, "{catalog}");
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
            files["src/jukebox_song.rs"]
                .contains("pub const FIVE: ResourceLocation<'static> = rl!(\"minecraft:5\");"),
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

        assert!(files[CATALOG_LIB].contains("    (mcrs_minecraft_keys::NONE, &[]),\n"));
        assert!(files[CATALOG_LIB].contains(
            "    (\n        mcrs_minecraft_keys::BLOCK,\n        mcrs_minecraft_keys::block::ENTRIES,\n    ),\n"
        ));
        assert!(!files.contains_key("src/none.rs"));
    }

    #[test]
    fn a_registry_without_entries_gets_its_name_and_no_constants() {
        let files = generated(
            &[],
            &[
                ("minecraft:item_modifier", &[]),
                ("minecraft:worldgen/biome", &["minecraft:plains"]),
            ],
        )
        .unwrap();

        assert!(files["src/registry.rs"].contains(
            "pub const ITEM_MODIFIER: ResourceLocation<'static> = rl!(\"minecraft:item_modifier\");"
        ));
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
        generate(
            &registries,
            &datapack,
            &names,
            &[],
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .map(in_keys_crate)
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
             use mcrs_minecraft_resource::{ResourceLocation, rl};\n\
             \n\
             pub const LOGS: ResourceLocation<'static> = rl!(\"minecraft:logs\");\n"
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
            "pub const HAS_STRUCTURE_VILLAGE: ResourceLocation<'static> = rl!(\"minecraft:has_structure/village\");"
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

        assert_eq!(constants(&files, "src/block.rs"), ["AIR", "STONE"]);
        assert_eq!(constants(&files, "src/item.rs"), ["AIR", "APPLE", "STONE"]);
        assert!(
            files["src/item.rs"]
                .contains("pub const AIR: ResourceLocation<'static> = rl!(\"minecraft:air\");")
        );
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

        assert!(
            files["src/registry.rs"].contains("pub const ADVANCEMENT: ResourceLocation<'static>")
        );
        assert!(files["src/registry.rs"].contains("pub const RECIPE: ResourceLocation<'static>"));
        assert!(!files.contains_key("src/advancement.rs"));
        assert!(!files.contains_key("src/recipe.rs"));
        assert!(!files["src/lib.rs"].contains("advancement"));
        assert!(!files["src/lib.rs"].contains("recipe"));
        assert!(files.contains_key("src/biome.rs"));
    }

    #[test]
    fn registries_are_named_from_their_path() {
        let rows = [
            ("minecraft:block", "block", "BLOCK"),
            (
                "minecraft:dimension_type",
                "dimension_type",
                "DIMENSION_TYPE",
            ),
            (
                "minecraft:worldgen/placed_feature",
                "placed_feature",
                "PLACED_FEATURE",
            ),
        ];
        for (registry, module, key) in rows {
            let files = generated(&[], &[(registry, &["minecraft:x"])]).unwrap();
            assert!(
                files.contains_key(&format!("src/{module}.rs")),
                "{registry}"
            );
            let names = &files["src/registry.rs"];
            assert!(
                names.contains(&format!(
                    "pub const {key}: ResourceLocation<'static> = rl!(\"{registry}\");"
                )),
                "{names}"
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
            generate(
                &registries,
                &datapack,
                &names,
                &[],
                &BTreeSet::new(),
                &BTreeSet::new(),
            ),
            &["minecraft:block"],
        );

        let (registries, mut datapack, names) = reports(statics, data);
        datapack.registries.remove("minecraft:worldgen/biome");
        assert_refused(
            generate(
                &registries,
                &datapack,
                &names,
                &[],
                &BTreeSet::new(),
                &BTreeSet::new(),
            ),
            &["minecraft:worldgen/biome"],
        );

        let (registries, datapack, mut names) = reports(statics, data);
        names.entries.remove("minecraft:worldgen/biome");
        assert_refused(
            generate(
                &registries,
                &datapack,
                &names,
                &[],
                &BTreeSet::new(),
                &BTreeSet::new(),
            ),
            &["minecraft:worldgen/biome"],
        );

        let (registries, datapack, mut names) = reports(statics, data);
        names
            .entries
            .insert("minecraft:block".to_owned(), BTreeSet::new());
        assert_refused(
            generate(
                &registries,
                &datapack,
                &names,
                &[],
                &BTreeSet::new(),
                &BTreeSet::new(),
            ),
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
        registry: mcrs_minecraft_resource::RegistryKey<R>,
        tag: mcrs_minecraft_resource::TagKey<'static, R>,
    ) -> String {
        let location = tag.location();
        format!(
            "{}/tags/{}/{}.json",
            location.namespace(),
            registry.path(),
            location.path()
        )
    }

    #[test]
    fn generated_tags_name_their_shipped_files() {
        use mcrs_minecraft_biome::{biome_tag_keys, registry_keys::BIOME};
        use mcrs_minecraft_block::{block_tag_keys, registry_keys::BLOCK};
        use mcrs_minecraft_item::{item_tag_keys, registry_keys::ITEM};

        let vanilla = mcrs_minecraft_fixtures::vanilla_dir();
        let rows = [
            (
                tag_file(BLOCK, block_tag_keys::MINEABLE_PICKAXE),
                "minecraft/tags/block/mineable/pickaxe.json",
            ),
            (
                tag_file(BLOCK, block_tag_keys::LOGS),
                "minecraft/tags/block/logs.json",
            ),
            (
                tag_file(ITEM, item_tag_keys::LOGS),
                "minecraft/tags/item/logs.json",
            ),
            (
                tag_file(BIOME, biome_tag_keys::IS_OCEAN),
                "minecraft/tags/worldgen/biome/is_ocean.json",
            ),
        ];
        for (asset_path, expected) in rows {
            assert_eq!(asset_path, expected);
            let file = vanilla.join(
                asset_path
                    .strip_prefix("minecraft/")
                    .expect("a tag of the minecraft namespace"),
            );
            assert!(file.is_file(), "{} is not shipped", file.display());
        }
    }

    #[test]
    fn an_owned_registry_is_keyed_in_its_owner() {
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
            files["crates/mcrs_minecraft_biome/src/registry_keys.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             use mcrs_minecraft_resource::{Registered, RegistryKey, rl};\n\
             \n\
             pub const BIOME: RegistryKey<crate::Biome> = RegistryKey::new(rl!(\"minecraft:worldgen/biome\"));\n\
             impl Registered for crate::Biome {\n\
             \x20   const REGISTRY: RegistryKey<Self> = BIOME;\n\
             }\n"
        );
        assert!(
            files["crates/mcrs_minecraft_biome/src/biome_keys.rs"].contains(
                "pub const PLAINS: ResourceKey<'static, crate::Biome> = ResourceKey::new(rl!(\"minecraft:plains\"));"
            )
        );
        assert!(!files[CATALOG_MANIFEST].contains("mcrs_minecraft_biome"));
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
                value: ValueType::Enum("Fruit"),
            }],
        )
        .unwrap();

        assert_eq!(
            files["crates/mcrs_minecraft_food/src/fruit_keys.rs"],
            "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.\n\
             \n\
             mcrs_minecraft_registries::static_registry! {\n\
             \x20   pub enum Fruit;\n\
             \x20   Zebra = \"minecraft:zebra\",\n\
             \x20   Apple = \"minecraft:apple\",\n\
             \x20   MusicDisc5 = \"minecraft:music_disc_5\",\n\
             }\n"
        );
        let module = &files["crates/mcrs_minecraft_food/src/registry_keys.rs"];
        assert!(!module.contains("pub mod "), "{module}");
        assert!(!module.contains("pub use "), "{module}");
        assert!(module.contains("impl Registered for crate::fruit_keys::Fruit {"));
        assert!(files[CATALOG_LIB].contains(
            "&[(\n    mcrs_minecraft_food::registry_keys::FRUIT.location(),\n    mcrs_minecraft_food::fruit_keys::Fruit::ENTRIES,\n)];\n"
        ));
        assert!(files[CATALOG_MANIFEST].contains("\nmcrs_minecraft_food.workspace = true\n"));
    }

    #[test]
    fn a_registry_is_an_enum_exactly_when_it_is_static_with_entries() {
        let owner = |registry, value| Owner {
            registry,
            krate: "mcrs_minecraft_food",
            value,
        };
        let statics: Statics = &[("minecraft:none", &[]), ("minecraft:block", BLOCK)];
        let data: Data = &[("minecraft:worldgen/biome", &["minecraft:plains"])];
        for (registry, value) in [
            ("minecraft:none", ValueType::Enum("Fruit")),
            ("minecraft:worldgen/biome", ValueType::Enum("Fruit")),
            ("minecraft:block", ValueType::Defined("crate::Block")),
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
                value: ValueType::Enum("Fruit"),
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
            krate: "mcrs_minecraft_game_registries",
            value,
        };
        let above = BTreeSet::from(["mcrs_minecraft_game_registries".to_owned()]);

        let files = generate(
            &registries,
            &datapack,
            &names,
            &[owner(
                "minecraft:chat_type",
                ValueType::Defined("crate::ChatType"),
            )],
            &above,
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(files.contains_key("crates/mcrs_minecraft_game_registries/src/registry_keys.rs"));
        assert!(!files[CATALOG_LIB].contains("mcrs_minecraft_game_registries"));
        assert!(!files[CATALOG_MANIFEST].contains("mcrs_minecraft_game_registries"));

        assert_refused(
            generate(
                &registries,
                &datapack,
                &names,
                &[owner("minecraft:block", ValueType::Enum("Block"))],
                &above,
                &BTreeSet::new(),
            ),
            &["minecraft:block", "mcrs_minecraft_game_registries"],
        );
    }

    #[test]
    fn an_owner_that_lists_its_own_static_registries_is_left_out_of_the_catalog() {
        let (registries, datapack, names) = reports(&[("minecraft:block", BLOCK)], &[]);
        let owner = Owner {
            registry: "minecraft:block",
            krate: "mcrs_minecraft_worldgen_block",
            value: ValueType::Enum("Block"),
        };
        let own = BTreeSet::from(["mcrs_minecraft_worldgen_block".to_owned()]);

        let files = generate(
            &registries,
            &datapack,
            &names,
            &[owner],
            &BTreeSet::new(),
            &own,
        )
        .unwrap();
        assert!(!files[CATALOG_LIB].contains("mcrs_minecraft_worldgen_block"));
        assert!(!files[CATALOG_MANIFEST].contains("mcrs_minecraft_worldgen_block"));
        let module = &files["crates/mcrs_minecraft_worldgen_block/src/registry_keys.rs"];
        assert!(
            module.contains("&[(BLOCK.location(), crate::block_keys::Block::ENTRIES)]"),
            "{module}"
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
    fn an_owner_whose_lib_misses_a_key_module_is_named() {
        let root = scratch("keys-undeclared");
        let src = root.join("crates/mcrs_minecraft_biome/src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), "pub mod registry_keys;\n").unwrap();
        let files = Files::from([
            (
                "crates/mcrs_minecraft_biome/src/registry_keys.rs".to_owned(),
                HEADER.to_owned(),
            ),
            (
                "crates/mcrs_minecraft_biome/src/biome_keys.rs".to_owned(),
                HEADER.to_owned(),
            ),
        ]);

        let error = write(&root, &files).unwrap_err();

        assert!(
            error.contains("crates/mcrs_minecraft_biome/src/lib.rs: add `pub mod biome_keys;`"),
            "{error}"
        );
        assert!(!error.contains("registry_keys"), "{error}");
    }

    #[test]
    fn writing_replaces_changed_files_and_deletes_stale_sources() {
        let root = scratch("keys-write");
        let keys = root.join("crates/mcrs_minecraft_keys");
        let owner = root.join("crates/mcrs_minecraft_gone/src");
        fs::create_dir_all(keys.join("src")).unwrap();
        fs::create_dir_all(&owner).unwrap();
        fs::write(keys.join("Cargo.toml"), "[package]\n").unwrap();
        fs::write(keys.join("src/stale.rs"), "old").unwrap();
        fs::write(keys.join("src/notes.txt"), "kept").unwrap();
        fs::write(keys.join("src/lib.rs"), "old").unwrap();
        fs::write(owner.join("registry_keys.rs"), HEADER).unwrap();
        fs::write(owner.join("gone_keys.rs"), HEADER).unwrap();
        fs::write(owner.join("hand_keys.rs"), "// hand-written\n").unwrap();
        let (registries, datapack, names) = reports(&[("minecraft:block", BLOCK)], &[]);
        let files = generate(
            &registries,
            &datapack,
            &names,
            &[],
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        let block = "crates/mcrs_minecraft_keys/src/block.rs";
        let unformatted = files[block].replace("&[AIR, STONE]", "&[\n    AIR,\n    STONE,\n]");
        let unformatted = format!("#[rustfmt::skip]\n{unformatted}");
        fs::write(root.join(block), &unformatted).unwrap();

        write(&root, &files).unwrap();

        for (path, text) in files.iter().filter(|(path, _)| *path != block) {
            assert_eq!(
                &fs::read_to_string(root.join(path)).unwrap(),
                text,
                "{path}"
            );
        }
        assert_eq!(fs::read_to_string(root.join(block)).unwrap(), unformatted);
        assert!(!keys.join("src/stale.rs").exists());
        assert!(!owner.join("registry_keys.rs").exists());
        assert!(!owner.join("gone_keys.rs").exists());
        assert!(owner.join("hand_keys.rs").exists());
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
