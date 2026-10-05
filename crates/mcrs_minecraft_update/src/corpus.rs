use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::Path;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_client_jar::{Directory, Files};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{NameTable, RegistrySet};
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_feature::template::{PaletteState, Template};

const PREFIX: &str = "data/minecraft/";
const VERSION_FILE: &str = "version.json";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub written: Vec<String>,
    pub deleted: Vec<String>,
}

pub(crate) fn check(label: &str, path: &str) -> Result<(), String> {
    if path.contains('\\') {
        return Err(format!("{label}: the path holds a backslash"));
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("{label}: the path is not a plain relative path"));
        }
    }
    Ok(())
}

pub fn from_jar(jar: &[u8]) -> Result<(Files, Vec<u8>), String> {
    let directory = Directory::of(jar)?;
    let version = directory
        .read(jar, |name| name == VERSION_FILE)?
        .pop()
        .ok_or("the jar has no version.json")?
        .1;
    let mut files = Vec::new();
    for (name, bytes) in directory.read(jar, |name| name.starts_with(PREFIX))? {
        let path = &name[PREFIX.len()..];
        check(&name, path)?;
        files.push((path.to_owned(), bytes));
    }
    Ok((files, version))
}

/// A set that numbers every registry the stored reports name and holds no
/// values: enough to read a value that names entries of other registries, and
/// to build the ones the code describes.
pub fn stored_names(registries: &Path, names: &Path) -> Result<RegistrySet, String> {
    let bytes = fs::read(registries).map_err(|error| io(registries, error))?;
    let statics =
        from_report(&bytes).map_err(|error| format!("{}: {error}", registries.display()))?;
    let stored = crate::names::read(names)?;
    let parse = |text: &str| {
        ResourceLocation::parse(text)
            .map_err(|error| format!("{}: {text}: {error}", names.display()))
    };
    let mut tables: Vec<_> = statics.tables().cloned().collect();
    for (registry, entries) in &stored.entries {
        let tags = stored.tags.get(registry).into_iter().flatten();
        let table = NameTable::new(
            parse(registry)?,
            entries
                .iter()
                .map(|name| parse(name))
                .collect::<Result<Vec<_>, _>>()?,
            tags.map(|tag| parse(tag)).collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|error| format!("{}: {registry}: {error}", names.display()))?;
        tables.push(std::sync::Arc::new(table));
    }
    RegistrySet::from_tables(tables).map_err(|error| format!("{}: {error}", names.display()))
}

/// Splits off the jar entries the code builds itself. An entry the code builds
/// identically is dropped, so no file is kept for it. One the code builds
/// differently stays: the file then overrides the built-in at load, and its
/// path is returned so the difference is ported rather than lost.
///
/// A biome is compared as a value: the jar text is read against `names` and
/// the code's biome is built against the same set. A jar biome that does not
/// read there, a placed feature the stored names lack included, differs.
pub fn without_built_in(
    files: Files,
    names: &RegistrySet,
) -> Result<(Files, usize, Vec<String>), String> {
    let mut built_biomes = None;
    let mut identical = 0;
    let mut diverged = Vec::new();
    let mut kept = Files::new();
    for (path, bytes) in files {
        let same = match biome_name(&path) {
            Some(name) => {
                if built_biomes.is_none() {
                    built_biomes = Some(builtin::biomes(names).map_err(|failures| {
                        format!(
                            "the built biomes do not resolve against the stored names: {failures:?}"
                        )
                    })?);
                }
                built_biomes
                    .as_ref()
                    .and_then(|built| built.get(&name))
                    .map(|built| same_biome(names, built, &bytes))
            }
            None => builtin::asset(&format!("minecraft/{path}"))
                .map(|built| same_content(&path, &built, &bytes)),
        };
        match same {
            Some(true) => identical += 1,
            Some(false) => {
                diverged.push(path.clone());
                kept.push((path, bytes));
            }
            None => kept.push((path, bytes)),
        }
    }
    Ok((kept, identical, diverged))
}

fn biome_name(path: &str) -> Option<ResourceLocation> {
    let name = path
        .strip_prefix("worldgen/biome/")?
        .strip_suffix(".json")?;
    Some(ResourceLocation::minecraft(name))
}

fn same_biome(names: &RegistrySet, built: &Biome, shipped: &[u8]) -> bool {
    names
        .scope(|| serde_json::from_slice::<Biome>(shipped))
        .is_ok_and(|shipped| &shipped == built)
}

fn same_content(path: &str, built: &[u8], shipped: &[u8]) -> bool {
    if path.ends_with(".nbt") {
        same_template(built, shipped)
    } else {
        same_json(built, shipped)
    }
}

type TemplateCells = BTreeMap<[i32; 3], (PaletteState, Option<NbtCompound>)>;

fn template_cells(template: &Template) -> Option<TemplateCells> {
    let palette = template.palette.as_ref()?;
    template
        .blocks
        .iter()
        .map(|block| {
            let mut state = palette.get(usize::try_from(block.state).ok()?)?.clone();
            state.properties = state.properties.filter(|properties| !properties.is_empty());
            Some((block.pos, (state, block.nbt.clone())))
        })
        .collect()
}

// Two saves of one structure differ in palette order and block order.
fn same_template(built: &[u8], shipped: &[u8]) -> bool {
    let parse = |bytes| from_gzip_bytes::<Template, _>(Cursor::new(bytes)).ok();
    let (Some(built), Some(shipped)) = (parse(built), parse(shipped)) else {
        return false;
    };
    if built.palettes.is_some() || shipped.palettes.is_some() {
        return built == shipped;
    }
    built.size == shipped.size
        && built.data_version == shipped.data_version
        && built.entities == shipped.entities
        && built.blocks.len() == shipped.blocks.len()
        && template_cells(&built).is_some_and(|cells| Some(cells) == template_cells(&shipped))
}

fn same_json(built: &[u8], shipped: &[u8]) -> bool {
    let parse = |bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok();
    parse(built).is_some_and(|built| Some(built) == parse(shipped))
}

pub(crate) fn io(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

pub(crate) fn write_if_changed(dir: &Path, path: &str, bytes: &[u8]) -> Result<bool, String> {
    let full = dir.join(path);
    if fs::read(&full).is_ok_and(|held| held == bytes) {
        return Ok(false);
    }
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
    }
    fs::write(&full, bytes).map_err(|error| io(&full, error))?;
    Ok(true)
}

pub(crate) fn files_below(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| io(dir, error))?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|error| io(&path, error))?
            .is_dir()
        {
            files_below(root, &path, out)?;
        } else {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            out.push(
                relative
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            );
        }
    }
    Ok(())
}

/// Makes `dir` hold exactly `files` and `version`. Deletion only ever names a file found on
/// disk below `dir`.
pub fn replace(dir: &Path, files: &[(String, Vec<u8>)], version: &[u8]) -> Result<Report, String> {
    for (path, _) in files {
        check(path, path)?;
    }
    let wanted: BTreeSet<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    let mut report = Report::default();
    for (path, bytes) in files {
        if write_if_changed(dir, path, bytes)? {
            report.written.push(path.clone());
        }
    }
    if write_if_changed(dir, VERSION_FILE, version)? {
        report.written.push(VERSION_FILE.to_owned());
    }
    report.written.sort();

    let mut on_disk = Vec::new();
    files_below(dir, dir, &mut on_disk)?;
    on_disk.sort();
    for path in on_disk {
        if path == VERSION_FILE || wanted.contains(path.as_str()) {
            continue;
        }
        let full = dir.join(&path);
        fs::remove_file(&full).map_err(|error| io(&full, error))?;
        let mut parent = full.parent();
        while let Some(emptied) = parent {
            if emptied == dir || fs::remove_dir(emptied).is_err() {
                break;
            }
            parent = emptied.parent();
        }
        report.deleted.push(path);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::testing::{scratch, zipped};

    fn apply(dir: &Path, jar: &[u8]) -> Result<Report, String> {
        let (files, version) = from_jar(jar)?;
        replace(dir, &files, &version)
    }

    fn put(dir: &Path, path: &str, bytes: &[u8]) {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn listing(dir: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let name = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned();
                    out.push((name, fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    fn strings(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn a_new_and_a_changed_file_are_written_and_an_unchanged_file_is_not() {
        let dir = scratch("written");
        put(&dir, "tags/same.json", b"same");
        put(&dir, "tags/changed.json", b"old");
        let jar = zipped(&[
            ("data/minecraft/tags/same.json", b"same"),
            ("data/minecraft/tags/changed.json", b"new"),
            ("data/minecraft/tags/added.json", b"added"),
            ("version.json", b"{}"),
        ]);

        let report = apply(&dir, &jar).unwrap();

        assert_eq!(
            report.written,
            strings(&["tags/added.json", "tags/changed.json", "version.json"])
        );
        assert_eq!(fs::read(dir.join("tags/changed.json")).unwrap(), b"new");
        assert_eq!(fs::read(dir.join("tags/added.json")).unwrap(), b"added");
        assert_eq!(fs::read(dir.join("tags/same.json")).unwrap(), b"same");
    }

    #[test]
    fn a_file_the_jar_lacks_is_deleted_and_reported() {
        let dir = scratch("deleted");
        put(&dir, "tags/gone.json", b"gone");
        put(&dir, "tags/stays.json", b"stays");
        let jar = zipped(&[
            ("data/minecraft/tags/stays.json", b"stays"),
            ("version.json", b"{}"),
        ]);

        let report = apply(&dir, &jar).unwrap();

        assert_eq!(report.deleted, strings(&["tags/gone.json"]));
        assert!(!dir.join("tags/gone.json").exists());
    }

    #[test]
    fn the_version_file_is_copied_byte_for_byte_and_never_deleted() {
        let dir = scratch("version");
        put(&dir, "version.json", b"old");
        let text: &[u8] = b"{\n    \"id\": \"x\"\n}";

        let report = apply(&dir, &zipped(&[("version.json", text)])).unwrap();

        assert_eq!(fs::read(dir.join("version.json")).unwrap(), text);
        assert!(report.deleted.is_empty());
        assert_eq!(report.written, strings(&["version.json"]));
    }

    #[test]
    fn an_entry_that_leaves_the_corpus_stops_the_step_before_any_write() {
        for name in [
            "data/minecraft/../outside.json",
            "data/minecraft/tags/../../x.json",
            "data/minecraft//double.json",
            "data/minecraft//absolute.json",
            "data/minecraft/back\\slash.json",
            "data/minecraft/./dot.json",
        ] {
            let dir = scratch("traversal");
            put(&dir, "tags/old.json", b"old");
            let before = listing(&dir);
            let jar = zipped(&[
                ("data/minecraft/tags/new.json", b"new"),
                (name, b"evil"),
                ("version.json", b"{}"),
            ]);

            let error = apply(&dir, &jar).unwrap_err();

            assert!(error.contains(name), "{name}: {error}");
            assert_eq!(listing(&dir), before, "{name}");
        }
    }

    #[test]
    fn entries_outside_the_data_pack_are_ignored() {
        let dir = scratch("outside");
        let jar = zipped(&[
            ("data/.mcassetsroot", b""),
            ("assets/minecraft/lang/en_us.json", b"{}"),
            ("net/minecraft/Main.class", b"class"),
            ("data/minecraft/tags/in.json", b"in"),
            ("version.json", b"{}"),
        ]);

        apply(&dir, &jar).unwrap();

        assert_eq!(
            listing(&dir)
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            strings(&["tags/in.json", "version.json"])
        );
    }

    #[test]
    fn a_directory_left_empty_by_a_deletion_is_removed() {
        let dir = scratch("empty");
        put(&dir, "a/b/c/gone.json", b"gone");
        put(&dir, "a/keep.json", b"keep");
        let jar = zipped(&[
            ("data/minecraft/a/keep.json", b"keep"),
            ("version.json", b"{}"),
        ]);

        apply(&dir, &jar).unwrap();

        assert!(!dir.join("a/b").exists());
        assert!(dir.join("a/keep.json").exists());
    }

    #[test]
    fn a_second_run_writes_and_deletes_nothing() {
        let dir = scratch("twice");
        put(&dir, "stale.json", b"stale");
        put(&dir, "stale/k.json", b"k");
        let jar = zipped(&[
            ("data/minecraft/tags/a.json", b"a"),
            ("version.json", b"{}"),
        ]);

        apply(&dir, &jar).unwrap();
        let after_first = listing(&dir);
        let second = apply(&dir, &jar).unwrap();

        assert!(second.written.is_empty(), "{:?}", second.written);
        assert!(second.deleted.is_empty(), "{:?}", second.deleted);
        assert_eq!(listing(&dir), after_first);
    }

    fn saved(order: [usize; 2], top: &str) -> Vec<u8> {
        let states = ["minecraft:stone", top];
        let palette = order
            .map(|index| PaletteState {
                id: mcrs_minecraft_core::ResourceLocation::parse(states[index]).unwrap(),
                properties: None,
            })
            .to_vec();
        let blocks = order
            .iter()
            .enumerate()
            .map(
                |(slot, &index)| mcrs_minecraft_worldgen_feature::template::TemplateBlock {
                    nbt: None,
                    pos: [0, index as i32, 0],
                    state: slot as i32,
                },
            )
            .collect();
        let template = Template {
            size: [1, 2, 1],
            entities: Vec::new(),
            blocks,
            palette: Some(palette),
            palettes: None,
            data_version: mcrs_minecraft_core::VERSION.world_version,
        };
        mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(&template).unwrap()
    }

    #[test]
    fn templates_compare_by_content_not_by_palette_or_block_order() {
        let path = "structure/a.nbt";
        assert!(same_content(
            path,
            &saved([0, 1], "minecraft:dirt"),
            &saved([1, 0], "minecraft:dirt")
        ));
        assert!(!same_content(
            path,
            &saved([0, 1], "minecraft:dirt"),
            &saved([0, 1], "minecraft:sand")
        ));
        assert!(!same_content(
            path,
            &saved([0, 1], "minecraft:dirt"),
            b"not a template"
        ));
    }

    #[test]
    fn an_entry_the_code_builds_identically_is_dropped_and_a_different_one_is_kept() {
        let zero = "worldgen/density_function/zero.json";
        let y = "worldgen/density_function/y.json";
        let biome = "worldgen/structure/igloo.json";
        let files = vec![
            (zero.to_owned(), b" 0.0 ".to_vec()),
            (y.to_owned(), b"1.0".to_vec()),
            (biome.to_owned(), b"{}".to_vec()),
        ];
        let (kept, identical, diverged) = without_built_in(files, &stored_set()).unwrap();
        assert_eq!(identical, 1);
        assert_eq!(diverged, strings(&[y]));
        let kept: Vec<String> = kept.into_iter().map(|(path, _)| path).collect();
        assert_eq!(kept, strings(&[y, biome]));
    }

    fn stored_set() -> RegistrySet {
        let reports = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/reports");
        stored_names(
            &reports.join("registries.json"),
            &reports.join("names.json"),
        )
        .unwrap()
    }

    const PLAINS: &str = "worldgen/biome/plains.json";

    fn jar_plains(
        names: &RegistrySet,
        change: impl FnOnce(&mut mcrs_minecraft_biome::Biome),
    ) -> Files {
        let mut plains = mcrs_minecraft_worldgen_builtin::biomes(names)
            .unwrap()
            .remove(&ResourceLocation::minecraft("plains"))
            .unwrap();
        change(&mut plains);
        let text = names.scope(|| serde_json::to_vec_pretty(&plains).unwrap());
        vec![(PLAINS.to_owned(), text)]
    }

    #[test]
    fn a_jar_biome_equal_to_the_built_one_is_left_to_the_code() {
        let names = stored_set();
        let files = jar_plains(&names, |_| {});
        let (kept, identical, diverged) = without_built_in(files, &names).unwrap();
        assert_eq!((kept.len(), identical, diverged.len()), (0, 1, 0));
    }

    #[test]
    fn a_jar_biome_that_differs_is_kept_as_a_file() {
        let names = stored_set();
        let files = jar_plains(&names, |plains| plains.temperature = 0.1);
        let (kept, identical, diverged) = without_built_in(files, &names).unwrap();
        assert_eq!(identical, 0);
        assert_eq!(diverged, strings(&[PLAINS]));
        assert_eq!(kept.len(), 1);
    }
}
